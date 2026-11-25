//! yt-dlp: análise de links, escolhas de qualidade baseadas nos formatos reais e
//! leitura do progresso.

use serde::Serialize;
use serde_json::Value;

use crate::ToolError;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaFormat {
    pub id: String,
    pub ext: String,
    pub has_video: bool,
    pub has_audio: bool,
    pub height: Option<u32>,
    pub fps: Option<f64>,
    pub vcodec: Option<String>,
    pub acodec: Option<String>,
    pub abr: Option<f64>,
    pub filesize: Option<u64>,
    pub note: Option<String>,
}

/// Uma opção apresentada ao usuário (derivada dos formatos que o site realmente oferece).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Choice {
    pub id: String,
    pub label: String,
    /// video | audio
    pub kind: String,
    /// Tamanho aproximado (soma de vídeo + áudio quando o site informa).
    pub approx_size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    pub id: String,
    pub title: String,
    pub uploader: Option<String>,
    pub duration: Option<f64>,
    pub thumbnail: Option<String>,
    pub webpage_url: String,
    pub extractor: String,
    pub is_playlist: bool,
    pub entries: u32,
    pub formats: Vec<MediaFormat>,
    pub choices: Vec<Choice>,
    pub has_subtitles: bool,
}

fn s(v: &Value, k: &str) -> Option<String> {
    v.get(k).and_then(Value::as_str).filter(|x| !x.is_empty()).map(str::to_string)
}

fn f(v: &Value, k: &str) -> Option<f64> {
    v.get(k).and_then(Value::as_f64)
}

fn codec(v: &Value, k: &str) -> Option<String> {
    s(v, k).filter(|c| c != "none")
}

/// Item já resolvido (não "plano") que veio sem nenhum formato: foto de um carrossel.
fn is_photo_entry(e: &Value) -> bool {
    e.get("formats").and_then(Value::as_array).is_some_and(|f| f.is_empty())
}

/// Um post com fotos e vídeos (carrossel do Instagram, por exemplo) chega como playlist cujos itens
/// já vêm resolvidos. As fotos não contam como itens; se sobrar um vídeo só, ele é mostrado como
/// vídeo comum, com as qualidades de verdade.
pub fn parse_info(v: &Value) -> MediaInfo {
    let is_playlist = s(v, "_type").as_deref() == Some("playlist");
    let videos: Vec<&Value> = v
        .get("entries")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter(|e| !e.is_null() && !is_photo_entry(e)).collect())
        .unwrap_or_default();
    if is_playlist
        && let [only] = videos.as_slice()
        && only.get("formats").is_some()
    {
        let mut info = parse_info(only);
        if let Some(u) = s(v, "webpage_url") {
            info.webpage_url = u;
        }
        return info;
    }
    let entries = videos.len() as u32;
    let formats: Vec<MediaFormat> = v
        .get("formats")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter(|x| s(x, "protocol").as_deref() != Some("mhtml")) // storyboards
                .map(|x| {
                    let vcodec = codec(x, "vcodec");
                    let acodec = codec(x, "acodec");
                    MediaFormat {
                        id: s(x, "format_id").unwrap_or_default(),
                        ext: s(x, "ext").unwrap_or_default(),
                        // Alguns extratores não informam codec: usa altura como sinal de vídeo.
                        has_video: vcodec.is_some() || (x.get("vcodec").is_none() && f(x, "height").is_some()),
                        has_audio: acodec.is_some() || (x.get("acodec").is_none() && f(x, "height").is_none()),
                        height: f(x, "height").map(|h| h as u32),
                        fps: f(x, "fps"),
                        vcodec,
                        acodec,
                        abr: f(x, "abr"),
                        filesize: f(x, "filesize").or_else(|| f(x, "filesize_approx")).map(|n| n as u64),
                        note: s(x, "format_note"),
                    }
                })
                .filter(|m| !m.id.is_empty())
                .collect()
        })
        .unwrap_or_default();
    let has_subtitles = v.get("subtitles").and_then(Value::as_object).is_some_and(|o| !o.is_empty());
    let mut info = MediaInfo {
        id: s(v, "id").unwrap_or_default(),
        title: s(v, "title").unwrap_or_else(|| "Sem título".into()),
        uploader: s(v, "uploader").or_else(|| s(v, "channel")),
        duration: f(v, "duration"),
        thumbnail: s(v, "thumbnail").filter(|t| t.starts_with("https://")),
        webpage_url: s(v, "webpage_url").unwrap_or_default(),
        extractor: s(v, "extractor_key").or_else(|| s(v, "extractor")).unwrap_or_default(),
        is_playlist,
        entries,
        formats,
        choices: vec![],
        has_subtitles,
    };
    info.choices = choices(&info);
    info
}

const HEIGHTS: [u32; 7] = [2160, 1440, 1080, 720, 480, 360, 240];

fn best_audio_size(fs: &[MediaFormat]) -> Option<u64> {
    fs.iter().filter(|f| f.has_audio && !f.has_video).filter_map(|f| f.filesize).max()
}

pub fn choices(info: &MediaInfo) -> Vec<Choice> {
    let fs = &info.formats;
    let mut out = Vec::new();
    let has_video = info.is_playlist || fs.iter().any(|f| f.has_video) || fs.is_empty();
    let audio = best_audio_size(fs);
    if has_video {
        let best_h = fs.iter().filter(|f| f.has_video).filter_map(|f| f.height).max();
        let best_size = best_h.and_then(|h| fs.iter().filter(|f| f.has_video && f.height == Some(h)).filter_map(|f| f.filesize).max()).map(|v| v + audio.unwrap_or(0));
        out.push(Choice {
            id: "best".into(),
            label: match best_h {
                Some(h) => format!("Melhor qualidade ({h}p)"),
                None => "Melhor qualidade".into(),
            },
            kind: "video".into(),
            approx_size: best_size,
        });
        // Cada formato cai na resolução padrão mais próxima acima dele (1072p → 1080p).
        let bucket = |x: u32| HEIGHTS.iter().rev().copied().find(|h| *h >= x).unwrap_or(x);
        let best_bucket = best_h.map(bucket);
        for h in HEIGHTS {
            let at = fs.iter().filter(|f| f.has_video && f.height.is_some_and(|x| bucket(x) == h));
            let size = at.clone().filter_map(|f| f.filesize).max();
            if at.count() > 0 && Some(h) != best_bucket {
                out.push(Choice { id: format!("v{h}"), label: format!("{h}p"), kind: "video".into(), approx_size: size.map(|v| v + audio.unwrap_or(0)) });
            }
        }
    }
    // Áudio pode vir de um fluxo só de áudio ou ser extraído de um vídeo com som (ou de formato sem codec informado).
    if info.is_playlist || fs.is_empty() || fs.iter().any(|f| f.has_audio || f.acodec.is_none() && f.vcodec.is_none() || f.acodec.is_some()) {
        out.push(Choice { id: "audio-m4a".into(), label: "Só áudio (M4A)".into(), kind: "audio".into(), approx_size: audio });
        out.push(Choice { id: "audio-mp3".into(), label: "Só áudio (MP3)".into(), kind: "audio".into(), approx_size: audio });
    }
    out
}

pub struct DownloadArgs<'a> {
    pub url: &'a str,
    pub choice: &'a str,
    pub out_dir: &'a str,
    pub ffmpeg_dir: &'a str,
    pub playlist: bool,
    pub subtitles: bool,
}

pub const PROGRESS_PREFIX: &str = "SHIFTPROG ";
pub const POSTPROCESS_PREFIX: &str = "SHIFTPP ";
pub const FILE_PREFIX: &str = "SHIFTFILE ";

/// Argumentos do download (sem shell, um item por argumento).
pub fn download_args(a: &DownloadArgs<'_>) -> Vec<String> {
    let mut v: Vec<String> = Vec::new();
    let mut push = |x: &str| v.push(x.to_string());
    let (format, audio) = match a.choice {
        "best" => ("bv*+ba/b".to_string(), None),
        "audio-m4a" => ("ba[ext=m4a]/ba/b".to_string(), Some("m4a")),
        "audio-mp3" => ("ba/b".to_string(), Some("mp3")),
        c if c.starts_with('v') && c[1..].parse::<u32>().is_ok() => {
            let h = &c[1..];
            (format!("bv*[height<={h}]+ba/b[height<={h}]"), None)
        }
        c if c.starts_with("f:") => (c[2..].to_string(), None),
        _ => ("bv*+ba/b".to_string(), None),
    };
    push("-f");
    push(&format);
    if let Some(fmt) = audio {
        push("-x");
        push("--audio-format");
        push(fmt);
        push("--audio-quality");
        push("0");
    } else {
        push("--merge-output-format");
        push("mp4/mkv");
    }
    if a.playlist {
        push("--yes-playlist");
        push("-o");
        push("%(playlist_title).80B [%(playlist_id)s]/%(playlist_index)03d - %(title).120B [%(id)s].%(ext)s");
    } else {
        push("--no-playlist");
        push("-o");
        push("%(title).150B [%(id)s].%(ext)s");
    }
    if a.subtitles && audio.is_none() {
        for x in ["--write-subs", "--sub-langs", "pt.*,en.*,es.*", "--embed-subs"] {
            push(x);
        }
    }
    for x in ["-P", a.out_dir, "--ffmpeg-location", a.ffmpeg_dir] {
        push(x);
    }
    for x in [
        // Sem isso, no Windows o yt-dlp escreve no código de página do console (cp1252): "·", acentos e
        // emoji no título chegavam como bytes inválidos e a linha com o caminho final se perdia.
        "--encoding",
        "utf-8",
        "--newline",
        "--no-colors",
        "--windows-filenames",
        "--no-mtime",
        "--embed-metadata",
        // Fotos de um carrossel não têm formato de vídeo: são puladas em vez de derrubar o post inteiro.
        "--ignore-no-formats-error",
        "--progress",
        "--progress-template",
        "download:SHIFTPROG %(progress.status)s|%(progress.downloaded_bytes)s|%(progress.total_bytes)s|%(progress.total_bytes_estimate)s|%(progress.speed)s|%(progress.eta)s|%(info.playlist_index)s|%(info.n_entries)s|%(info.format_id)s",
        "--progress-template",
        "postprocess:SHIFTPP %(progress.postprocessor)s|%(progress.status)s",
        "--print",
        "after_move:SHIFTFILE %(filepath)s",
        "--",
        a.url,
    ] {
        push(x);
    }
    v
}

/// Argumentos da análise (JSON único; playlists sem baixar cada item).
pub fn analyze_args(url: &str) -> Vec<String> {
    ["--encoding", "utf-8", "-J", "--flat-playlist", "--ignore-no-formats-error", "--no-warnings", "--no-colors", "--", url].into_iter().map(str::to_string).collect()
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Progress {
        downloaded: Option<u64>,
        total: Option<u64>,
        speed: Option<f64>,
        eta: Option<f64>,
        item: Option<u32>,
        items: Option<u32>,
        format_id: Option<String>,
        finished: bool,
    },
    PostProcess { name: String },
    File(String),
}

fn num<T: std::str::FromStr>(s: &str) -> Option<T> {
    let s = s.trim();
    if s == "NA" || s == "None" || s.is_empty() { None } else { s.parse().ok() }
}

pub fn parse_line(line: &str) -> Option<Event> {
    let line = line.trim_end();
    if let Some(rest) = line.strip_prefix(PROGRESS_PREFIX) {
        let p: Vec<&str> = rest.split('|').collect();
        if p.len() < 9 {
            return None;
        }
        return Some(Event::Progress {
            finished: p[0] == "finished",
            downloaded: num::<f64>(p[1]).map(|x| x as u64),
            total: num::<f64>(p[2]).or_else(|| num::<f64>(p[3])).map(|x| x as u64),
            speed: num(p[4]),
            eta: num(p[5]),
            item: num(p[6]),
            items: num(p[7]),
            format_id: Some(p[8].to_string()).filter(|x| x != "NA"),
        });
    }
    if let Some(rest) = line.strip_prefix(POSTPROCESS_PREFIX) {
        return Some(Event::PostProcess { name: rest.split('|').next().unwrap_or("").to_string() });
    }
    line.strip_prefix(FILE_PREFIX).map(|p| Event::File(p.trim().to_string()))
}

/// Todas as linhas de erro são de itens sem vídeo (fotos de um carrossel)? Nesse caso o yt-dlp sai
/// com código 1 mesmo tendo baixado os vídeos do post.
pub fn only_photo_errors(stderr: &str) -> bool {
    let mut errors = stderr.lines().filter(|l| l.contains("ERROR")).peekable();
    errors.peek().is_some() && errors.all(|l| l.to_ascii_lowercase().contains("no video formats found"))
}

/// Classifica a mensagem de erro do yt-dlp (linhas "ERROR: …").
pub fn classify_error(stderr: &str) -> ToolError {
    let err = stderr.lines().rfind(|l| l.contains("ERROR")).unwrap_or(stderr).trim().to_string();
    let l = err.to_ascii_lowercase();
    let kind = if l.contains("unsupported url") || l.contains("no video formats found") && l.contains("generic") {
        "unsupported_url"
    } else if l.contains("no video formats found") {
        "no_video"
    } else if l.contains("drm") {
        "drm"
    } else if l.contains("requested format is not available") || l.contains("format is not available") {
        "format_unavailable"
    } else if l.contains("sign in") || l.contains("login") || l.contains("private video") || l.contains("members-only") || l.contains("cookies") {
        "http_auth"
    } else if l.contains("http error 403") {
        "forbidden"
    } else if l.contains("http error 404") || l.contains("video unavailable") || l.contains("not available") {
        "not_found"
    } else if l.contains("http error 429") || l.contains("too many requests") {
        "server_busy"
    } else if l.contains("getaddrinfo") || l.contains("name resolution") || l.contains("failed to resolve") {
        "dns"
    } else if l.contains("timed out") {
        "timeout"
    } else if l.contains("unable to download") || l.contains("connection") {
        "connection"
    } else if l.contains("no space left") {
        "disk_full"
    } else {
        "media_failed"
    };
    let detail: String = err.chars().take(400).collect();
    ToolError { kind, detail }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample() -> Value {
        json!({
            "id": "abc123", "title": "Palestra", "uploader": "Canal", "duration": 300.5,
            "thumbnail": "https://img/x.jpg", "webpage_url": "https://site/v/abc123", "extractor_key": "Generic",
            "subtitles": {"pt": []},
            "formats": [
                {"format_id": "sb0", "ext": "mhtml", "protocol": "mhtml", "vcodec": "none", "acodec": "none"},
                {"format_id": "140", "ext": "m4a", "vcodec": "none", "acodec": "mp4a.40.2", "abr": 129.0, "filesize": 4000000},
                {"format_id": "137", "ext": "mp4", "vcodec": "avc1", "acodec": "none", "height": 1080, "filesize": 90000000},
                {"format_id": "136", "ext": "mp4", "vcodec": "avc1", "acodec": "none", "height": 720, "filesize": 40000000},
                {"format_id": "18", "ext": "mp4", "vcodec": "avc1", "acodec": "mp4a", "height": 360, "filesize_approx": 15000000}
            ]
        })
    }

    #[test]
    fn analisa_formatos_reais_e_monta_escolhas() {
        let i = parse_info(&sample());
        assert_eq!(i.formats.len(), 4, "storyboard mhtml ignorado");
        assert!(i.has_subtitles);
        let ids: Vec<&str> = i.choices.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["best", "v720", "v360", "audio-m4a", "audio-mp3"]);
        assert_eq!(i.choices[0].label, "Melhor qualidade (1080p)");
        assert_eq!(i.choices[0].approx_size, Some(94_000_000));
        assert_eq!(i.choices[1].approx_size, Some(44_000_000));
    }

    #[test]
    fn playlist_plana() {
        let i = parse_info(&json!({"_type": "playlist", "id": "PL1", "title": "Curso", "entries": [{"id": "a"}, {"id": "b"}]}));
        assert!(i.is_playlist);
        assert_eq!(i.entries, 2);
        assert!(i.choices.iter().any(|c| c.id == "best") && i.choices.iter().any(|c| c.id == "audio-mp3"));
    }

    fn carrossel(itens: Vec<Value>) -> Value {
        json!({"_type": "playlist", "id": "DgK", "title": "Post by cazetv", "webpage_url": "https://www.instagram.com/p/DgK/", "entries": itens})
    }

    #[test]
    fn carrossel_com_foto_e_um_video_vira_video_comum() {
        let foto = json!({"id": "foto1", "title": "Foto", "formats": []});
        let video = json!({"id": "vid1", "title": "Video by cazetv", "webpage_url": "https://www.instagram.com/p/vid1/",
            "formats": [{"format_id": "1", "ext": "mp4", "vcodec": "avc1", "acodec": "none", "height": 1280},
                        {"format_id": "2", "ext": "m4a", "vcodec": "none", "acodec": "mp4a"}]});
        let i = parse_info(&carrossel(vec![foto.clone(), video.clone()]));
        assert!(!i.is_playlist, "um vídeo só não é playlist");
        assert_eq!(i.title, "Video by cazetv");
        assert_eq!(i.webpage_url, "https://www.instagram.com/p/DgK/", "baixa pelo link do post");
        assert_eq!(i.choices[0].label, "Melhor qualidade (1280p)");
        let i = parse_info(&carrossel(vec![foto.clone(), video.clone(), video]));
        assert!(i.is_playlist && i.entries == 2, "fotos não contam como itens");
        assert_eq!(parse_info(&carrossel(vec![foto])).entries, 0);
    }

    #[test]
    fn erro_so_de_fotos_puladas() {
        let fotos = "WARNING: No video formats found!\nERROR: [Instagram] DgK_9bVjgau: No video formats found!; please report this issue";
        assert!(only_photo_errors(fotos));
        assert!(!only_photo_errors(&format!("{fotos}\nERROR: unable to download video data: HTTP Error 403: Forbidden")));
        assert!(!only_photo_errors("WARNING: nada"));
        assert_eq!(classify_error(fotos).kind, "no_video");
        assert!(analyze_args("u").contains(&"--ignore-no-formats-error".to_string()));
    }

    #[test]
    fn argumentos_sem_shell_e_com_url_depois_de_dois_tracos() {
        let a = download_args(&DownloadArgs { url: "-oops", choice: "v720", out_dir: "C:\\D", ffmpeg_dir: "C:\\F", playlist: false, subtitles: true });
        let i = a.iter().position(|x| x == "--").unwrap();
        assert_eq!(a[i + 1], "-oops", "URL que começa com hífen não vira opção");
        assert!(a.contains(&"bv*[height<=720]+ba/b[height<=720]".to_string()));
        assert!(a.contains(&"--embed-subs".to_string()));
        let mp3 = download_args(&DownloadArgs { url: "u", choice: "audio-mp3", out_dir: "d", ffmpeg_dir: "f", playlist: true, subtitles: true });
        // Saída sempre em UTF-8: sem isso, títulos com "·" ou acentos quebravam a leitura do caminho final.
        for args in [&mp3, &analyze_args("u")] {
            let i = args.iter().position(|x| x == "--encoding").expect("--encoding");
            assert_eq!(args[i + 1], "utf-8");
        }
        assert!(mp3.contains(&"mp3".to_string()) && !mp3.contains(&"--embed-subs".to_string()));
        assert!(mp3.contains(&"--yes-playlist".to_string()));
    }

    #[test]
    fn le_progresso_e_arquivo_final() {
        let e = parse_line("SHIFTPROG downloading|1048576|NA|10485760|524288.0|18|NA|NA|137").unwrap();
        assert_eq!(
            e,
            Event::Progress { downloaded: Some(1048576), total: Some(10485760), speed: Some(524288.0), eta: Some(18.0), item: None, items: None, format_id: Some("137".into()), finished: false }
        );
        assert_eq!(parse_line("SHIFTPP Merger|started"), Some(Event::PostProcess { name: "Merger".into() }));
        assert_eq!(parse_line("SHIFTFILE C:\\D\\Palestra [abc].mp4"), Some(Event::File("C:\\D\\Palestra [abc].mp4".into())));
        assert_eq!(parse_line("[youtube] lixo"), None);
    }

    #[test]
    fn classifica_erros() {
        assert_eq!(classify_error("ERROR: Unsupported URL: https://x").kind, "unsupported_url");
        assert_eq!(classify_error("ERROR: [x] abc: This video is DRM protected").kind, "drm");
        assert_eq!(classify_error("ERROR: [y] Private video. Sign in if you've been granted access").kind, "http_auth");
        assert_eq!(classify_error("ERROR: unable to download video data: HTTP Error 403: Forbidden").kind, "forbidden");
        assert_eq!(classify_error("ERROR: Requested format is not available").kind, "format_unavailable");
        assert_eq!(classify_error("algo estranho").kind, "media_failed");
    }
}
