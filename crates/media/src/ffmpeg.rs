//! FFmpeg: presets de conversão, leitura do ffprobe e do progresso.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ToolError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: &'static str,
    pub label: &'static str,
    pub ext: &'static str,
    pub audio_only: bool,
    /// Copia os fluxos sem recodificar (rápido, sem perda).
    pub remux: bool,
}

pub const PRESETS: &[Preset] = &[
    Preset { id: "mp4", label: "MP4 (H.264 + AAC)", ext: "mp4", audio_only: false, remux: false },
    Preset { id: "mp4-hw", label: "MP4 (H.264, aceleração de hardware)", ext: "mp4", audio_only: false, remux: false },
    Preset { id: "webm", label: "WebM (VP9 + Opus)", ext: "webm", audio_only: false, remux: false },
    Preset { id: "mkv-remux", label: "MKV (sem recodificar)", ext: "mkv", audio_only: false, remux: true },
    Preset { id: "mp4-remux", label: "MP4 (sem recodificar)", ext: "mp4", audio_only: false, remux: true },
    Preset { id: "mp3", label: "MP3", ext: "mp3", audio_only: true, remux: false },
    Preset { id: "m4a", label: "M4A (AAC)", ext: "m4a", audio_only: true, remux: false },
    Preset { id: "flac", label: "FLAC (sem perda)", ext: "flac", audio_only: true, remux: false },
    Preset { id: "wav", label: "WAV", ext: "wav", audio_only: true, remux: false },
    Preset { id: "opus", label: "Opus", ext: "opus", audio_only: true, remux: false },
];

pub fn preset(id: &str) -> Option<Preset> {
    PRESETS.iter().copied().find(|p| p.id == id)
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertOptions {
    /// Altura máxima (redimensiona mantendo a proporção).
    pub height: Option<u32>,
    /// Qualidade constante (menor = melhor). Padrão por preset.
    pub crf: Option<u32>,
    /// Bitrate de áudio em kbps.
    pub audio_kbps: Option<u32>,
    /// Corte em segundos.
    pub start: Option<f64>,
    pub end: Option<f64>,
}

/// Encoders de hardware conhecidos, em ordem de preferência.
pub const HW_ENCODERS: [&str; 3] = ["h264_nvenc", "h264_qsv", "h264_amf"];

/// Argumentos do ffmpeg. `hw` = encoder de hardware disponível (para "mp4-hw").
pub fn convert_args(input: &str, output: &str, p: Preset, o: &ConvertOptions, hw: Option<&str>) -> Vec<String> {
    let mut v: Vec<String> = ["-hide_banner", "-nostdin", "-n", "-loglevel", "error", "-progress", "pipe:1", "-nostats", "-i", input]
        .into_iter()
        .map(str::to_string)
        .collect();
    let mut push = |xs: &[&str]| v.extend(xs.iter().map(|x| x.to_string()));
    if let Some(s) = o.start.filter(|s| *s > 0.0) {
        push(&["-ss", &format!("{s:.3}")]);
    }
    if let Some(e) = o.end.filter(|e| *e > 0.0) {
        push(&["-to", &format!("{e:.3}")]);
    }
    let crf = |default: u32| o.crf.unwrap_or(default).clamp(0, 51).to_string();
    let abr = |default: u32| format!("{}k", o.audio_kbps.unwrap_or(default).clamp(32, 512));
    let scale = o.height.filter(|_| !p.remux && !p.audio_only).map(|h| format!("scale=-2:'min({h},ih)'"));
    if let Some(s) = &scale {
        push(&["-vf", s]);
    }
    match p.id {
        "mp4" => push(&["-map", "0:v:0?", "-map", "0:a:0?", "-c:v", "libx264", "-preset", "medium", "-crf", &crf(23), "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", &abr(160), "-movflags", "+faststart"]),
        "mp4-hw" => {
            let q = crf(23);
            push(&["-map", "0:v:0?", "-map", "0:a:0?"]);
            match hw.unwrap_or("h264_nvenc") {
                "h264_qsv" => push(&["-c:v", "h264_qsv", "-global_quality", &q]),
                "h264_amf" => push(&["-c:v", "h264_amf", "-rc", "cqp", "-qp_i", &q, "-qp_p", &q]),
                _ => push(&["-c:v", "h264_nvenc", "-preset", "p5", "-rc", "vbr", "-cq", &q]),
            }
            push(&["-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", &abr(160), "-movflags", "+faststart"]);
        }
        "webm" => push(&["-map", "0:v:0?", "-map", "0:a:0?", "-c:v", "libvpx-vp9", "-crf", &crf(32), "-b:v", "0", "-row-mt", "1", "-deadline", "good", "-cpu-used", "4", "-c:a", "libopus", "-b:a", &abr(128)]),
        "mkv-remux" => push(&["-map", "0", "-c", "copy"]),
        "mp4-remux" => push(&["-map", "0:v?", "-map", "0:a?", "-c", "copy", "-movflags", "+faststart"]),
        "mp3" => push(&["-vn", "-c:a", "libmp3lame", "-b:a", &abr(192)]),
        "m4a" => push(&["-vn", "-c:a", "aac", "-b:a", &abr(192)]),
        "flac" => push(&["-vn", "-c:a", "flac"]),
        "wav" => push(&["-vn", "-c:a", "pcm_s16le"]),
        "opus" => push(&["-vn", "-c:a", "libopus", "-b:a", &abr(128)]),
        _ => {}
    }
    v.push(output.to_string());
    v
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub duration: Option<f64>,
    pub size: Option<u64>,
    pub format: Option<String>,
    pub video: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub audio: Option<String>,
}

pub fn probe_args(path: &str) -> Vec<String> {
    ["-v", "error", "-print_format", "json", "-show_format", "-show_streams", "--", path].into_iter().map(str::to_string).collect()
}

pub fn parse_probe(v: &Value) -> FileInfo {
    let fmt = v.get("format");
    let streams = v.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let video = streams.iter().find(|s| s.get("codec_type").and_then(Value::as_str) == Some("video") && s.pointer("/disposition/attached_pic").and_then(Value::as_i64) != Some(1));
    let audio = streams.iter().find(|s| s.get("codec_type").and_then(Value::as_str) == Some("audio"));
    let num = |v: Option<&Value>| v.and_then(|x| x.as_str().and_then(|s| s.parse::<f64>().ok()).or_else(|| x.as_f64()));
    FileInfo {
        duration: num(fmt.and_then(|f| f.get("duration"))),
        size: num(fmt.and_then(|f| f.get("size"))).map(|n| n as u64),
        format: fmt.and_then(|f| f.get("format_long_name")).and_then(Value::as_str).map(str::to_string),
        video: video.and_then(|s| s.get("codec_name")).and_then(Value::as_str).map(str::to_string),
        width: video.and_then(|s| s.get("width")).and_then(Value::as_u64).map(|n| n as u32),
        height: video.and_then(|s| s.get("height")).and_then(Value::as_u64).map(|n| n as u32),
        audio: audio.and_then(|s| s.get("codec_name")).and_then(Value::as_str).map(str::to_string),
    }
}

/// Estimativa aproximada do tamanho final. `None` quando depende demais do conteúdo
/// (vídeo com qualidade constante): nesse caso a interface diz que não dá para prever.
pub fn estimate_size(p: Preset, info: &FileInfo, o: &ConvertOptions) -> Option<u64> {
    let full = info.duration?;
    let dur = (o.end.unwrap_or(full).min(full) - o.start.unwrap_or(0.0)).max(0.0);
    let ratio = if full > 0.0 { dur / full } else { 1.0 };
    let kbps = |k: u32| Some((k as f64 * 1000.0 / 8.0 * dur) as u64);
    match p.id {
        "mp3" => kbps(o.audio_kbps.unwrap_or(192)),
        "m4a" => kbps(o.audio_kbps.unwrap_or(192)),
        "opus" => kbps(o.audio_kbps.unwrap_or(128)),
        "wav" => kbps(1411),
        "flac" => kbps(850),
        "mkv-remux" | "mp4-remux" => info.size.map(|s| (s as f64 * ratio) as u64),
        _ => None,
    }
}

/// `-progress pipe:1` escreve "out_time_us=…" (ou out_time_ms, que também é em µs).
pub fn parse_progress(line: &str) -> Option<ProgressLine> {
    let (k, v) = line.trim().split_once('=')?;
    match k {
        "out_time_us" | "out_time_ms" => v.parse::<i64>().ok().filter(|n| *n >= 0).map(|us| ProgressLine::Time(us as f64 / 1_000_000.0)),
        "speed" => v.trim_end_matches('x').trim().parse().ok().map(ProgressLine::Speed),
        "total_size" => v.parse().ok().map(ProgressLine::Size),
        "progress" => Some(ProgressLine::Done(v == "end")),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ProgressLine {
    Time(f64),
    /// Multiplicador de tempo real (2.0 = duas vezes mais rápido que reproduzir).
    Speed(f64),
    Size(u64),
    Done(bool),
}

pub fn classify_error(stderr: &str) -> ToolError {
    let l = stderr.to_ascii_lowercase();
    let kind = if l.contains("no space left") {
        "disk_full"
    } else if l.contains("permission denied") || l.contains("access is denied") {
        "permission"
    } else if l.contains("already exists") {
        "file_exists"
    } else {
        "media_failed"
    };
    ToolError { kind, detail: stderr.chars().take(400).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn argumentos_por_preset() {
        let o = ConvertOptions { height: Some(720), start: Some(5.0), end: Some(10.0), ..Default::default() };
        let a = convert_args("in.mkv", "out.mp4", preset("mp4").unwrap(), &o, None);
        assert_eq!(a.last().unwrap(), "out.mp4");
        assert!(a.windows(2).any(|w| w == ["-crf", "23"]));
        assert!(a.windows(2).any(|w| w == ["-ss", "5.000"]) && a.windows(2).any(|w| w == ["-to", "10.000"]));
        assert!(a.iter().any(|x| x.contains("min(720,ih)")));
        assert!(a.contains(&"-n".to_string()), "nunca sobrescreve");
        let r = convert_args("in", "out.mkv", preset("mkv-remux").unwrap(), &o, None);
        assert!(r.windows(2).any(|w| w == ["-c", "copy"]) && !r.iter().any(|x| x.contains("scale")));
        let hw = convert_args("in", "o.mp4", preset("mp4-hw").unwrap(), &ConvertOptions::default(), Some("h264_qsv"));
        assert!(hw.windows(2).any(|w| w == ["-c:v", "h264_qsv"]));
        let mp3 = convert_args("in", "o.mp3", preset("mp3").unwrap(), &ConvertOptions { audio_kbps: Some(320), ..Default::default() }, None);
        assert!(mp3.contains(&"-vn".to_string()) && mp3.contains(&"320k".to_string()));
    }

    #[test]
    fn ffprobe_e_estimativas() {
        let v = json!({"format": {"duration": "120.0", "size": "60000000", "format_long_name": "Matroska"},
                       "streams": [{"codec_type": "video", "codec_name": "h264", "width": 1920, "height": 1080},
                                   {"codec_type": "audio", "codec_name": "opus"}]});
        let i = parse_probe(&v);
        assert_eq!((i.duration, i.height, i.audio.as_deref()), (Some(120.0), Some(1080), Some("opus")));
        let mp3 = estimate_size(preset("mp3").unwrap(), &i, &ConvertOptions::default()).unwrap();
        assert_eq!(mp3, 192_000 / 8 * 120);
        let half = estimate_size(preset("mkv-remux").unwrap(), &i, &ConvertOptions { end: Some(60.0), ..Default::default() }).unwrap();
        assert_eq!(half, 30_000_000);
        assert_eq!(estimate_size(preset("mp4").unwrap(), &i, &ConvertOptions::default()), None, "vídeo CRF: não dá para prever");
    }

    #[test]
    fn progresso() {
        assert_eq!(parse_progress("out_time_us=1500000"), Some(ProgressLine::Time(1.5)));
        assert_eq!(parse_progress("speed=2.5x"), Some(ProgressLine::Speed(2.5)));
        assert_eq!(parse_progress("progress=end"), Some(ProgressLine::Done(true)));
        assert_eq!(parse_progress("out_time_us=N/A"), None);
        assert_eq!(classify_error("x: No space left on device").kind, "disk_full");
    }
}
