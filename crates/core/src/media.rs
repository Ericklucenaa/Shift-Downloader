//! Central de mídia: downloads de vídeo/áudio (yt-dlp) e conversões locais (FFmpeg).
//! Fila e limite de concorrência próprios, separados dos downloads do aria2.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use shift_media::ffmpeg::{self, ConvertOptions, FileInfo};
use shift_media::ytdlp::{self, MediaInfo};
use shift_persistence::{Db, MediaJob, now_ms};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{Notify, watch};

use crate::diagnostics::{Diagnostic, DiagnosticKind};
use crate::manager::{CoreResult, EventSink, db_err, state_err};
use crate::views::Settings;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MediaView {
    pub id: String,
    /// download | convert
    pub kind: String,
    pub title: String,
    pub source: String,
    pub status: String,
    pub progress: f64,
    /// video | audio | processing | converting | ""
    pub phase: String,
    /// bytes/s (download) ou multiplicador de tempo real (conversão)
    pub speed: Option<f64>,
    pub eta: Option<f64>,
    pub total_bytes: Option<i64>,
    pub output_path: Option<String>,
    pub output_dir: String,
    pub thumbnail: Option<String>,
    pub duration: Option<f64>,
    pub label: String,
    pub item: Option<u32>,
    pub items: Option<u32>,
    pub error: Option<Diagnostic>,
    pub created_at: i64,
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaDownloadRequest {
    pub url: String,
    pub choice: String,
    #[serde(default)]
    pub choice_label: Option<String>,
    pub title: String,
    #[serde(default)]
    pub video_id: Option<String>,
    #[serde(default)]
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub playlist: bool,
    #[serde(default)]
    pub subtitles: bool,
    #[serde(default)]
    pub directory: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertRequest {
    pub path: String,
    pub preset: String,
    #[serde(default)]
    pub options: ConvertOptions,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetView {
    pub id: String,
    pub label: String,
    pub ext: String,
    pub audio_only: bool,
    pub remux: bool,
    pub approx_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileProbe {
    pub path: String,
    pub name: String,
    pub info: FileInfo,
    pub presets: Vec<PresetView>,
    pub hw_encoder: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsInfo {
    pub available: bool,
    pub ytdlp: Option<String>,
    pub ffmpeg: Option<String>,
    pub hw_encoder: Option<String>,
}

#[derive(Clone)]
struct Tools {
    ytdlp: PathBuf,
    ffmpeg: PathBuf,
    ffprobe: PathBuf,
    ffmpeg_dir: PathBuf,
}

#[derive(Default, Clone)]
struct Live {
    phase: String,
    speed: Option<f64>,
    eta: Option<f64>,
    item: Option<u32>,
    items: Option<u32>,
}

struct Running {
    pid: Option<u32>,
    stop: Option<&'static str>,
}

struct MInner {
    db: Arc<Db>,
    sink: Arc<dyn EventSink>,
    settings: Arc<RwLock<Settings>>,
    tools: Option<Tools>,
    running: Mutex<HashMap<String, Running>>,
    live: Mutex<HashMap<String, Live>>,
    wake: Notify,
    hw: tokio::sync::OnceCell<Option<String>>,
}

#[derive(Clone)]
pub struct MediaManager {
    inner: Arc<MInner>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn tool_err(kind: &str, detail: &str) -> Diagnostic {
    Diagnostic::new(DiagnosticKind::parse(kind), crate::diagnostics::scrub(detail))
}

fn missing() -> Diagnostic {
    Diagnostic::new(DiagnosticKind::ToolMissing, "yt-dlp/FFmpeg ausentes na pasta tools")
}

fn command(program: &Path) -> Command {
    let mut c = Command::new(program);
    c.stdin(Stdio::null()).kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

/// Encerra o processo e os filhos (o yt-dlp chama o ffmpeg para juntar vídeo e áudio).
async fn kill_tree(pid: u32) {
    #[cfg(windows)]
    {
        let mut c = command(Path::new("taskkill"));
        let _ = c.args(["/PID", &pid.to_string(), "/T", "/F"]).stdout(Stdio::null()).stderr(Stdio::null()).status().await;
    }
    #[cfg(not(windows))]
    {
        let _ = command(Path::new("kill")).args(["-9", &pid.to_string()]).status().await;
    }
}

/// Executa a ferramenta e devolve (sucesso, stdout, stderr), com tempo limite.
async fn run_capture(program: &Path, args: &[String], timeout: Duration) -> Result<(bool, String, String), String> {
    let out = tokio::time::timeout(timeout, command(program).args(args).stdout(Stdio::piped()).stderr(Stdio::piped()).output())
        .await
        .map_err(|_| "tempo esgotado".to_string())?
        .map_err(|e| e.to_string())?;
    Ok((out.status.success(), String::from_utf8_lossy(&out.stdout).to_string(), String::from_utf8_lossy(&out.stderr).to_string()))
}

impl MediaManager {
    pub(crate) fn start(db: Arc<Db>, sink: Arc<dyn EventSink>, settings: Arc<RwLock<Settings>>, tools_dir: Option<PathBuf>, stop_rx: watch::Receiver<bool>) -> Self {
        let tools = tools_dir.and_then(|d| {
            let t = Tools {
                ytdlp: d.join(if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp" }),
                ffmpeg: d.join("ffmpeg").join(if cfg!(windows) { "ffmpeg.exe" } else { "ffmpeg" }),
                ffprobe: d.join("ffmpeg").join(if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" }),
                ffmpeg_dir: d.join("ffmpeg"),
            };
            (t.ytdlp.is_file() && t.ffmpeg.is_file() && t.ffprobe.is_file()).then_some(t)
        });
        let inner = Arc::new(MInner {
            db,
            sink,
            settings,
            tools,
            running: Mutex::default(),
            live: Mutex::default(),
            wake: Notify::new(),
            hw: tokio::sync::OnceCell::new(),
        });
        // Trabalhos interrompidos (app fechado ou derrubado) voltam para a fila.
        for j in inner.db.list_media().unwrap_or_default().into_iter().filter(|j| j.status == "running") {
            if j.kind == "convert" {
                remove_quiet(j.output_path.as_deref().or(planned_output(&j).as_deref()));
            }
            let _ = inner.db.set_media_status(&j.id, "queued", None);
        }
        tokio::spawn(scheduler(inner.clone(), stop_rx));
        Self { inner }
    }

    pub fn available(&self) -> bool {
        self.inner.tools.is_some()
    }

    /// Há algo rodando ou na fila (usado pela ação "ao terminar tudo").
    pub fn busy(&self) -> bool {
        !lock(&self.inner.running).is_empty() || self.inner.db.list_media().unwrap_or_default().iter().any(|j| j.status == "queued")
    }

    pub fn list(&self) -> CoreResult<Vec<MediaView>> {
        Ok(self.inner.db.list_media().map_err(db_err)?.iter().map(|j| self.inner.view_of(j)).collect())
    }

    pub async fn tools_info(&self) -> ToolsInfo {
        let Some(t) = &self.inner.tools else { return ToolsInfo { available: false, ytdlp: None, ffmpeg: None, hw_encoder: None } };
        let first = |s: String| s.lines().next().unwrap_or("").trim().to_string();
        let y = run_capture(&t.ytdlp, &["--version".into()], Duration::from_secs(20)).await.ok().map(|o| first(o.1));
        let f = run_capture(&t.ffmpeg, &["-hide_banner".into(), "-version".into()], Duration::from_secs(20))
            .await
            .ok()
            .map(|o| first(o.1).replace("ffmpeg version ", "").split(' ').next().unwrap_or("").to_string());
        ToolsInfo { available: true, ytdlp: y, ffmpeg: f, hw_encoder: self.inner.hw_encoder().await }
    }

    /// Atualiza o yt-dlp (os sites mudam com frequência). Devolve a saída da ferramenta.
    pub async fn update_ytdlp(&self) -> CoreResult<String> {
        let t = self.inner.tools.as_ref().ok_or_else(missing)?;
        let (ok, out, err) = run_capture(&t.ytdlp, &["-U".into(), "--no-colors".into()], Duration::from_secs(180))
            .await
            .map_err(|e| Diagnostic::new(DiagnosticKind::Connection, e))?;
        let text = format!("{out}{err}").trim().to_string();
        if ok { Ok(text) } else { Err(tool_err("media_failed", &text)) }
    }

    /// Analisa um link com o yt-dlp: título, duração e os formatos que o site oferece de fato.
    pub async fn analyze(&self, input: &str) -> CoreResult<MediaInfo> {
        let t = self.inner.tools.as_ref().ok_or_else(missing)?;
        let url = shift_security::parse_download_url(input).map_err(|e| Diagnostic::new(DiagnosticKind::InvalidUrl, e.to_string()))?;
        check_target(&url).await?;
        let (ok, out, err) = run_capture(&t.ytdlp, &ytdlp::analyze_args(url.as_str()), Duration::from_secs(90))
            .await
            .map_err(|e| Diagnostic::new(DiagnosticKind::Timeout, format!("yt-dlp: {e}")))?;
        if !ok {
            let e = ytdlp::classify_error(&err);
            return Err(tool_err(e.kind, &e.detail));
        }
        let v: Value = serde_json::from_str(out.trim()).map_err(|e| tool_err("media_failed", &format!("resposta do yt-dlp: {e}")))?;
        Ok(ytdlp::parse_info(&v))
    }

    pub fn add_download(&self, req: MediaDownloadRequest) -> CoreResult<MediaView> {
        self.inner.tools.as_ref().ok_or_else(missing)?;
        let url = shift_security::parse_download_url(&req.url).map_err(|e| Diagnostic::new(DiagnosticKind::InvalidUrl, e.to_string()))?;
        let settings = self.inner.settings.read().unwrap_or_else(|e| e.into_inner()).clone();
        let dir = match req.directory.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
            Some(d) => PathBuf::from(d),
            None if settings.organize_by_type => {
                let cat = if req.choice.starts_with("audio") { "audio" } else { "video" };
                PathBuf::from(&settings.download_dir).join(crate::category::folder_name(cat, &settings.language))
            }
            None => PathBuf::from(&settings.download_dir),
        };
        std::fs::create_dir_all(&dir).map_err(|e| Diagnostic::new(DiagnosticKind::Permission, e.to_string()))?;
        let now = now_ms();
        let job = MediaJob {
            id: uuid::Uuid::new_v4().to_string(),
            kind: "download".into(),
            source: url.to_string(),
            title: req.title.chars().take(300).collect(),
            output_dir: dir.to_string_lossy().to_string(),
            options: json!({
                "choice": req.choice, "label": req.choice_label.unwrap_or_else(|| req.choice.clone()),
                "playlist": req.playlist, "subtitles": req.subtitles, "videoId": req.video_id,
            })
            .to_string(),
            status: "queued".into(),
            thumbnail: req.thumbnail.filter(|t| t.starts_with("https://")),
            duration: req.duration,
            created_at: now,
            updated_at: now,
            ..Default::default()
        };
        self.inner.db.insert_media(&job).map_err(db_err)?;
        self.inner.wake.notify_one();
        let v = self.inner.view_of(&job);
        self.inner.sink.media_changed(std::slice::from_ref(&v));
        Ok(v)
    }

    /// Lê um arquivo local para a interface oferecer os presets que fazem sentido.
    pub async fn probe_file(&self, path: &str) -> CoreResult<FileProbe> {
        let t = self.inner.tools.as_ref().ok_or_else(missing)?;
        let p = PathBuf::from(path);
        if !p.is_file() {
            return Err(state_err("Arquivo não encontrado."));
        }
        let (ok, out, err) = run_capture(&t.ffprobe, &ffmpeg::probe_args(path), Duration::from_secs(30))
            .await
            .map_err(|e| tool_err("media_failed", &e))?;
        if !ok {
            return Err(tool_err("media_failed", &format!("o FFmpeg não reconheceu o arquivo: {}", err.trim())));
        }
        let info = ffmpeg::parse_probe(&serde_json::from_str(&out).unwrap_or(Value::Null));
        if info.video.is_none() && info.audio.is_none() {
            return Err(tool_err("media_failed", "o arquivo não tem vídeo nem áudio"));
        }
        let hw = self.inner.hw_encoder().await;
        let presets = ffmpeg::PRESETS
            .iter()
            .filter(|p| p.audio_only || info.video.is_some())
            .filter(|p| p.id != "mp4-hw" || hw.is_some())
            .filter(|p| !p.audio_only || info.audio.is_some())
            .map(|p| PresetView {
                id: p.id.into(),
                label: p.label.into(),
                ext: p.ext.into(),
                audio_only: p.audio_only,
                remux: p.remux,
                approx_size: ffmpeg::estimate_size(*p, &info, &ConvertOptions::default()),
            })
            .collect();
        Ok(FileProbe {
            name: p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            path: path.into(),
            info,
            presets,
            hw_encoder: hw,
        })
    }

    pub fn add_convert(&self, req: ConvertRequest) -> CoreResult<MediaView> {
        self.inner.tools.as_ref().ok_or_else(missing)?;
        let preset = ffmpeg::preset(&req.preset).ok_or_else(|| state_err("Formato de conversão desconhecido."))?;
        let input = PathBuf::from(&req.path);
        if !input.is_file() {
            return Err(state_err("Arquivo não encontrado."));
        }
        let dir = input.parent().map(Path::to_path_buf).unwrap_or_default();
        let stem = input.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "saida".into());
        let wanted = shift_security::sanitize_filename(&format!("{stem} (convertido).{}", preset.ext));
        // Reserva o nome contra arquivos existentes e outras conversões na fila.
        let planned: Vec<String> = self.inner.db.list_media().unwrap_or_default().iter().filter_map(planned_output).collect();
        let name = shift_security::resolve_conflict(&wanted, |n| {
            let p = dir.join(n);
            p.exists() || planned.iter().any(|x| Path::new(x) == p)
        });
        let output = dir.join(&name);
        let now = now_ms();
        let job = MediaJob {
            id: uuid::Uuid::new_v4().to_string(),
            kind: "convert".into(),
            source: req.path.clone(),
            title: input.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
            output_dir: dir.to_string_lossy().to_string(),
            options: json!({ "preset": preset.id, "label": preset.label, "options": req.options, "output": output.to_string_lossy() }).to_string(),
            status: "queued".into(),
            created_at: now,
            updated_at: now,
            ..Default::default()
        };
        self.inner.db.insert_media(&job).map_err(db_err)?;
        self.inner.wake.notify_one();
        let v = self.inner.view_of(&job);
        self.inner.sink.media_changed(std::slice::from_ref(&v));
        Ok(v)
    }

    /// Pausa um download de mídia (o yt-dlp continua dos arquivos .part depois).
    pub async fn pause(&self, id: &str) -> CoreResult<()> {
        let j = self.inner.require(id)?;
        if j.kind != "download" {
            return Err(state_err("Conversões não podem ser pausadas; cancele e converta de novo se precisar."));
        }
        match j.status.as_str() {
            "running" => self.inner.stop(id, "pause").await,
            "queued" => {
                self.inner.db.set_media_status(id, "paused", None).map_err(db_err)?;
                self.inner.emit(id);
            }
            _ => {}
        }
        Ok(())
    }

    pub fn resume(&self, id: &str) -> CoreResult<()> {
        let j = self.inner.require(id)?;
        if matches!(j.status.as_str(), "paused" | "failed") {
            self.inner.db.set_media_status(id, "queued", None).map_err(db_err)?;
            self.inner.emit(id);
            self.inner.wake.notify_one();
        }
        Ok(())
    }

    pub async fn cancel(&self, id: &str) -> CoreResult<()> {
        let j = self.inner.require(id)?;
        match j.status.as_str() {
            "running" => self.inner.stop(id, "cancel").await,
            "queued" | "paused" | "failed" => {
                self.inner.cleanup(&j);
                self.inner.db.set_media_status(id, "cancelled", None).map_err(db_err)?;
                self.inner.emit(id);
            }
            _ => {}
        }
        Ok(())
    }

    /// Tira da lista (arquivos concluídos ficam no disco).
    pub fn remove(&self, id: &str) -> CoreResult<()> {
        let j = self.inner.require(id)?;
        if matches!(j.status.as_str(), "running" | "queued" | "paused") {
            return Err(state_err("Cancele o trabalho antes de removê-lo da lista."));
        }
        if j.status == "failed" {
            self.inner.cleanup(&j);
        }
        self.inner.db.delete_media(id).map_err(db_err)?;
        lock(&self.inner.live).remove(id);
        self.inner.sink.media_removed(&[id.to_string()]);
        Ok(())
    }

    /// Arquivo (ou pasta, para playlists) do resultado.
    pub fn output_path(&self, id: &str) -> CoreResult<PathBuf> {
        let j = self.inner.require(id)?;
        if let Some(p) = j.output_path.as_deref().map(PathBuf::from).filter(|p| p.exists()) {
            return Ok(p);
        }
        // Trabalhos concluídos antes da correção da leitura podem não ter o caminho gravado:
        // procura o arquivo na pasta de destino pelo título e guarda para as próximas vezes.
        if j.status == "completed"
            && let Some(found) = find_output_by_title(Path::new(&j.output_dir), &j.title)
        {
            let _ = self.inner.db.set_media_output(id, &found.to_string_lossy());
            return Ok(found);
        }
        if j.output_path.is_some() {
            return Err(state_err("O arquivo não está mais na pasta."));
        }
        Err(state_err("Ainda não há arquivo de saída."))
    }

    /// Para "Abrir pasta": o arquivo, se existir; senão a pasta de destino (melhor que um erro).
    pub fn reveal_target(&self, id: &str) -> CoreResult<(PathBuf, bool)> {
        match self.output_path(id) {
            Ok(p) => Ok((p, true)),
            Err(e) => {
                let j = self.inner.require(id)?;
                let dir = PathBuf::from(&j.output_dir);
                if dir.is_dir() { Ok((dir, false)) } else { Err(e) }
            }
        }
    }

    pub(crate) async fn shutdown(&self) {
        let ids: Vec<String> = lock(&self.inner.running).keys().cloned().collect();
        for id in ids {
            self.inner.stop(&id, "shutdown").await;
        }
    }
}

/// Mesma política de rede dos downloads: nada de link-local/endereços especiais.
async fn check_target(url: &url::Url) -> CoreResult<()> {
    let host = url.host_str().unwrap_or("").trim_start_matches('[').trim_end_matches(']').to_string();
    let port = url.port_or_known_default().unwrap_or(443);
    let ips: Vec<std::net::IpAddr> = match host.parse::<std::net::IpAddr>() {
        Ok(ip) => vec![ip],
        Err(_) => tokio::net::lookup_host((host.as_str(), port))
            .await
            .map_err(|e| Diagnostic::new(DiagnosticKind::Dns, format!("{host}: {e}")))?
            .map(|a| a.ip())
            .collect(),
    };
    if let shift_security::RedirectVerdict::Block(r) = shift_security::check_initial_target(&ips) {
        return Err(Diagnostic::new(DiagnosticKind::RedirectBlocked, r));
    }
    Ok(())
}

fn opts(j: &MediaJob) -> Value {
    serde_json::from_str(&j.options).unwrap_or(Value::Null)
}

fn planned_output(j: &MediaJob) -> Option<String> {
    (j.kind == "convert").then(|| opts(j).get("output").and_then(Value::as_str).map(str::to_string)).flatten()
}

fn remove_quiet(p: Option<&str>) {
    if let Some(p) = p {
        let _ = std::fs::remove_file(p);
    }
}

impl MInner {
    fn require(&self, id: &str) -> CoreResult<MediaJob> {
        self.db.get_media(id).map_err(db_err)?.ok_or_else(|| state_err("Trabalho de mídia não encontrado."))
    }

    fn view_of(&self, j: &MediaJob) -> MediaView {
        let o = opts(j);
        let l = lock(&self.live).get(&j.id).cloned().unwrap_or_default();
        let running = j.status == "running";
        MediaView {
            id: j.id.clone(),
            kind: j.kind.clone(),
            title: j.title.clone(),
            source: if j.kind == "download" {
                url::Url::parse(&j.source).map(|u| shift_security::redact_url(&u)).unwrap_or_default()
            } else {
                j.source.clone()
            },
            status: j.status.clone(),
            progress: j.progress,
            phase: if running { l.phase } else { String::new() },
            speed: if running { l.speed } else { None },
            eta: if running { l.eta } else { None },
            total_bytes: j.total_bytes,
            output_path: j.output_path.clone(),
            output_dir: j.output_dir.clone(),
            thumbnail: j.thumbnail.clone(),
            duration: j.duration,
            label: o.get("label").and_then(Value::as_str).unwrap_or("").to_string(),
            item: l.item,
            items: l.items,
            error: j.error_kind.as_ref().map(|k| Diagnostic::from_stored(k, j.error_detail.as_deref().unwrap_or(""))),
            created_at: j.created_at,
            finished_at: j.finished_at,
        }
    }

    fn emit(&self, id: &str) {
        if let Ok(j) = self.require(id) {
            self.sink.media_changed(&[self.view_of(&j)]);
        }
    }

    async fn hw_encoder(&self) -> Option<String> {
        let t = self.tools.clone()?;
        self.hw
            .get_or_init(|| async move {
                for enc in ffmpeg::HW_ENCODERS {
                    let args: Vec<String> = ["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "color=c=black:s=256x256:d=0.2", "-frames:v", "3", "-c:v", enc, "-f", "null", "-"]
                        .into_iter()
                        .map(str::to_string)
                        .collect();
                    if let Ok((true, _, _)) = run_capture(&t.ffmpeg, &args, Duration::from_secs(15)).await {
                        return Some(enc.to_string());
                    }
                }
                None
            })
            .await
            .clone()
    }

    async fn stop(&self, id: &str, why: &'static str) {
        let pid = {
            let mut r = lock(&self.running);
            match r.get_mut(id) {
                Some(x) => {
                    x.stop = Some(why);
                    x.pid
                }
                None => None,
            }
        };
        if let Some(pid) = pid {
            kill_tree(pid).await;
        }
        // Espera o executor do trabalho registrar o novo estado.
        for _ in 0..50 {
            if !lock(&self.running).contains_key(id) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Apaga sobras de um trabalho cancelado ou que falhou de vez.
    fn cleanup(&self, j: &MediaJob) {
        if j.kind == "convert" {
            if j.status != "completed" {
                remove_quiet(planned_output(j).as_deref());
            }
            return;
        }
        let Some(vid) = opts(j).get("videoId").and_then(Value::as_str).filter(|v| !v.is_empty()).map(|v| format!("[{v}]")) else { return };
        let dir = PathBuf::from(&j.output_dir);
        let Ok(entries) = std::fs::read_dir(&dir) else { return };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let partial = name.ends_with(".part") || name.ends_with(".ytdl") || name.contains(".part-Frag") || name.ends_with(".temp") || name.contains(".f") && name.contains(&vid);
            if name.contains(&vid) && (partial || j.status != "completed") && e.path().is_file() {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
}

async fn scheduler(inner: Arc<MInner>, mut stop_rx: watch::Receiver<bool>) {
    loop {
        tokio::select! {
            _ = inner.wake.notified() => {}
            _ = tokio::time::sleep(Duration::from_secs(2)) => {}
            _ = stop_rx.changed() => break,
        }
        if *stop_rx.borrow() || inner.tools.is_none() {
            continue;
        }
        let max = inner.settings.read().unwrap_or_else(|e| e.into_inner()).max_media_jobs.clamp(1, 4) as usize;
        loop {
            if lock(&inner.running).len() >= max {
                break;
            }
            let mut queued: Vec<MediaJob> = inner.db.list_media().unwrap_or_default().into_iter().filter(|j| j.status == "queued").collect();
            queued.sort_by_key(|j| j.created_at);
            let Some(job) = queued.into_iter().find(|j| !lock(&inner.running).contains_key(&j.id)) else { break };
            lock(&inner.running).insert(job.id.clone(), Running { pid: None, stop: None });
            let _ = inner.db.set_media_status(&job.id, "running", None);
            inner.emit(&job.id);
            tokio::spawn(run_job(inner.clone(), job));
        }
    }
}

async fn run_job(inner: Arc<MInner>, job: MediaJob) {
    let id = job.id.clone();
    let result = match job.kind.as_str() {
        "download" => run_download(&inner, &job).await,
        _ => run_convert(&inner, &job).await,
    };
    let stop = lock(&inner.running).remove(&id).and_then(|r| r.stop);
    lock(&inner.live).remove(&id);
    let fresh = inner.require(&id).unwrap_or(job.clone());
    match (stop, result) {
        (Some("pause"), _) => {
            let _ = inner.db.set_media_status(&id, "paused", None);
        }
        (Some("shutdown"), _) => {
            if job.kind == "convert" {
                remove_quiet(planned_output(&job).as_deref());
            }
            let _ = inner.db.set_media_status(&id, "queued", None);
        }
        (Some(_), _) => {
            let mut j = fresh.clone();
            j.status = "cancelled".into();
            inner.cleanup(&j);
            let _ = inner.db.set_media_status(&id, "cancelled", None);
        }
        (None, Ok(output)) => {
            if let Some(o) = &output {
                let _ = inner.db.set_media_output(&id, o);
            }
            let _ = inner.db.set_media_progress(&id, 1.0, None);
            let _ = inner.db.set_media_status(&id, "completed", None);
        }
        (None, Err(diag)) => {
            if job.kind == "convert" {
                // Original intacto; a saída parcial não serve para nada.
                remove_quiet(planned_output(&job).as_deref());
            }
            let _ = inner.db.set_media_status(&id, "failed", Some((diag.kind.as_str(), &diag.detail)));
        }
    }
    if let Ok(j) = inner.require(&id) {
        let v = inner.view_of(&j);
        inner.sink.media_changed(std::slice::from_ref(&v));
        if matches!(j.status.as_str(), "completed" | "failed") {
            inner.sink.media_finished(&v);
        }
    }
    inner.wake.notify_one();
}

/// Lê stdout linha a linha e stderr inteiro (para o diagnóstico).
async fn spawn_and_follow(inner: &Arc<MInner>, id: &str, program: &Path, args: &[String], mut on_line: impl FnMut(&str)) -> Result<(bool, Vec<String>), Diagnostic> {
    let mut child = command(program)
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| tool_err("tool_missing", &e.to_string()))?;
    if let Some(r) = lock(&inner.running).get_mut(id) {
        r.pid = child.id();
    }
    let stderr = child.stderr.take();
    let err_task = tokio::spawn(async move {
        let mut lines = Vec::new();
        if let Some(e) = stderr {
            let mut rd = BufReader::new(e).lines();
            while let Ok(Some(l)) = rd.next_line().await {
                if lines.len() > 200 {
                    lines.remove(0);
                }
                lines.push(l);
            }
        }
        lines
    });
    if let Some(out) = child.stdout.take() {
        // Lê bytes e converte com tolerância: `lines()` parava na primeira linha que não fosse UTF-8 válido
        // e a linha final com o caminho do arquivo (SHIFTFILE) nunca chegava. Era por isso que vídeos com
        // "·" ou emoji no título terminavam sem arquivo de saída e "Abrir"/"Abrir pasta" falhavam.
        // O yt-dlp agora recebe --encoding utf-8; a leitura tolerante fica como garantia.
        let mut rd = BufReader::new(out);
        let mut buf = Vec::with_capacity(512);
        loop {
            buf.clear();
            match rd.read_until(b'\n', &mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(_) => on_line(&decode_line(&buf)),
            }
        }
    }
    let status = child.wait().await.map_err(|e| tool_err("media_failed", &e.to_string()))?;
    let err_lines = err_task.await.unwrap_or_default();
    Ok((status.success(), err_lines))
}

async fn run_download(inner: &Arc<MInner>, job: &MediaJob) -> Result<Option<String>, Diagnostic> {
    let t = inner.tools.clone().ok_or_else(missing)?;
    let o = opts(job);
    let choice = o.get("choice").and_then(Value::as_str).unwrap_or("best").to_string();
    let playlist = o.get("playlist").and_then(Value::as_bool).unwrap_or(false);
    let args = ytdlp::download_args(&ytdlp::DownloadArgs {
        url: &job.source,
        choice: &choice,
        out_dir: &job.output_dir,
        ffmpeg_dir: &t.ffmpeg_dir.to_string_lossy(),
        playlist,
        subtitles: o.get("subtitles").and_then(Value::as_bool).unwrap_or(false),
    });
    std::fs::create_dir_all(&job.output_dir).map_err(|e| tool_err("permission", &e.to_string()))?;
    // Vídeo + áudio separados contam como duas etapas.
    let streams = if choice.starts_with("audio") || choice.starts_with("f:") && !choice.contains('+') { 1.0 } else { 2.0 };
    let mut formats: Vec<String> = Vec::new();
    let mut files: Vec<String> = Vec::new();
    let mut last_persist = Instant::now() - Duration::from_secs(10);
    let id = job.id.clone();
    let inner2 = inner.clone();
    let (ok, err) = spawn_and_follow(inner, &id, &t.ytdlp, &args, |line| match ytdlp::parse_line(line) {
        Some(ytdlp::Event::Progress { downloaded, total, speed, eta, item, items, format_id, .. }) => {
            if let Some(f) = format_id.filter(|f| !formats.contains(f)) {
                formats.push(f);
            }
            let stream_idx = formats.len().saturating_sub(1) as f64;
            let frac = match (downloaded, total) {
                (Some(d), Some(t)) if t > 0 => (d as f64 / t as f64).min(1.0),
                _ => 0.0,
            };
            let per_item = ((stream_idx.min(streams - 1.0) + frac) / streams).min(1.0) * 0.97;
            let overall = match (item, items) {
                (Some(i), Some(n)) if n > 0 => ((i.saturating_sub(1)) as f64 + per_item) / n as f64,
                _ => per_item,
            };
            {
                let mut live = lock(&inner2.live);
                let l = live.entry(id.clone()).or_default();
                l.phase = if (streams > 1.0 && stream_idx >= 1.0) || choice.starts_with("audio") { "audio".into() } else { "video".into() };
                l.speed = speed;
                l.eta = eta;
                l.item = item;
                l.items = items;
            }
            if last_persist.elapsed() >= Duration::from_millis(700) {
                last_persist = Instant::now();
                let _ = inner2.db.set_media_progress(&id, overall, total.map(|t| t as i64));
                inner2.emit(&id);
            }
        }
        Some(ytdlp::Event::PostProcess { .. }) => {
            lock(&inner2.live).entry(id.clone()).or_default().phase = "processing".into();
            let _ = inner2.db.set_media_progress(&id, 0.98, None);
            inner2.emit(&id);
        }
        Some(ytdlp::Event::File(p)) => files.push(p),
        None => {}
    })
    .await?;
    if !ok {
        let e = ytdlp::classify_error(&err.join("\n"));
        return Err(tool_err(e.kind, &e.detail));
    }
    let output = if playlist {
        files.first().and_then(|f| Path::new(f).parent()).map(|p| p.to_string_lossy().to_string())
    } else {
        files.last().cloned()
    };
    if let Some(o) = &output
        && let Ok(m) = std::fs::metadata(o)
        && m.is_file()
    {
        let _ = inner.db.set_media_progress(&job.id, 1.0, Some(m.len() as i64));
    }
    Ok(output)
}

async fn run_convert(inner: &Arc<MInner>, job: &MediaJob) -> Result<Option<String>, Diagnostic> {
    let t = inner.tools.clone().ok_or_else(missing)?;
    let o = opts(job);
    let preset = o.get("preset").and_then(Value::as_str).and_then(ffmpeg::preset).ok_or_else(|| tool_err("media_failed", "preset inválido"))?;
    let copts: ConvertOptions = o.get("options").cloned().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    let output = planned_output(job).ok_or_else(|| tool_err("media_failed", "saída não definida"))?;
    if !Path::new(&job.source).is_file() {
        return Err(state_err("O arquivo original não está mais na pasta."));
    }
    // Duração para o percentual.
    let (_, probe_out, _) = run_capture(&t.ffprobe, &ffmpeg::probe_args(&job.source), Duration::from_secs(30)).await.unwrap_or((false, String::new(), String::new()));
    let info = ffmpeg::parse_probe(&serde_json::from_str(&probe_out).unwrap_or(Value::Null));
    let full = info.duration.unwrap_or(0.0);
    let span = (copts.end.unwrap_or(full).min(full) - copts.start.unwrap_or(0.0)).max(0.001);
    let hw = if preset.id == "mp4-hw" { inner.hw_encoder().await } else { None };
    let args = ffmpeg::convert_args(&job.source, &output, preset, &copts, hw.as_deref());
    lock(&inner.live).entry(job.id.clone()).or_default().phase = "converting".into();
    let id = job.id.clone();
    let inner2 = inner.clone();
    let mut last = Instant::now() - Duration::from_secs(10);
    let mut cur = 0.0;
    let (ok, err) = spawn_and_follow(inner, &id, &t.ffmpeg, &args, |line| match ffmpeg::parse_progress(line) {
        Some(ffmpeg::ProgressLine::Time(s)) => {
            cur = (s / span).clamp(0.0, 0.99);
        }
        Some(ffmpeg::ProgressLine::Speed(x)) => {
            let mut live = lock(&inner2.live);
            let l = live.entry(id.clone()).or_default();
            l.speed = Some(x);
            l.eta = (x > 0.0).then(|| (span * (1.0 - cur)) / x);
        }
        Some(ffmpeg::ProgressLine::Done(_)) | Some(ffmpeg::ProgressLine::Size(_)) | None => {
            if last.elapsed() >= Duration::from_millis(700) {
                last = Instant::now();
                let _ = inner2.db.set_media_progress(&id, cur, None);
                inner2.emit(&id);
            }
        }
    })
    .await?;
    if !ok {
        let e = ffmpeg::classify_error(&shift_media::tail(&err, 6));
        return Err(tool_err(e.kind, &e.detail));
    }
    let size = std::fs::metadata(&output).map(|m| m.len()).ok();
    if size.unwrap_or(0) == 0 {
        return Err(tool_err("media_failed", "o FFmpeg terminou sem gerar o arquivo"));
    }
    // Confere se o resultado é mídia de verdade (ex.: corte fora da duração gera só o cabeçalho).
    let (_, out_probe, _) = run_capture(&t.ffprobe, &ffmpeg::probe_args(&output), Duration::from_secs(30)).await.unwrap_or((false, String::new(), String::new()));
    let out_info = ffmpeg::parse_probe(&serde_json::from_str(&out_probe).unwrap_or(Value::Null));
    if out_info.duration.is_none_or(|d| d < 0.05) || (out_info.video.is_none() && out_info.audio.is_none()) {
        return Err(tool_err("media_failed", "o arquivo gerado não tem conteúdo reproduzível (confira o trecho de corte)"));
    }
    let _ = inner.db.set_media_progress(&job.id, 1.0, size.map(|s| s as i64));
    Ok(Some(output))
}


/// Converte uma linha da saída de uma ferramenta: UTF-8 tolerante, sem quebra de linha no fim.
fn decode_line(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim_end_matches(['\n', '\r']).to_string()
}

/// Só letras e números ASCII, em minúsculas. O yt-dlp troca caracteres proibidos no Windows ("|", ":"…) e,
/// antes da correção de codificação, títulos gravados no banco vinham com "�" no lugar de "·", "é" etc.
/// Ignorar tudo que não é ASCII faz "Cruzou � Gol" (banco) bater com "Cruzou é Gol" (arquivo).
fn simplify(s: &str) -> String {
    s.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect()
}

/// Arquivo da pasta cujo nome começa como o título (o modelo de nome é "%(title).150B [%(id)s].%(ext)s").
/// Ignora partes temporárias do yt-dlp e, havendo mais de um, fica com o mais recente.
fn find_output_by_title(dir: &Path, title: &str) -> Option<PathBuf> {
    let key: String = simplify(title).chars().take(40).collect();
    if key.chars().count() < 4 {
        return None;
    }
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().map(|n| n.to_string_lossy().to_lowercase()) else { continue };
        if !path.is_file() || name.ends_with(".part") || name.ends_with(".ytdl") || name.contains(".part-frag") || name.ends_with(".temp") {
            continue;
        }
        if !simplify(&name).starts_with(&key) {
            continue;
        }
        let modified = entry.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
        if best.as_ref().is_none_or(|(t, _)| modified > *t) {
            best = Some((modified, path));
        }
    }
    best.map(|(_, p)| p)
}

#[cfg(test)]
mod output_tests {
    use super::*;

    #[test]
    fn invalid_utf8_line_is_decoded_instead_of_stopping() {
        // "·" em cp1252 é 0xB7: inválido em UTF-8. Antes, a leitura parava aqui.
        let line = decode_line(b"SHIFTFILE C:\\D\\2.1K views \xb7 770 [abc].mp4\r\n");
        assert!(line.starts_with("SHIFTFILE C:"));
        assert!(line.ends_with("[abc].mp4"));
    }

    #[test]
    fn finds_finished_file_by_title_ignoring_forbidden_characters() {
        let dir = std::env::temp_dir().join(format!("shift-find-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let wanted = dir.join("2.1K views · 770 reactions ｜ The Office ｜ Cruzou é Gol [1681432380].mp4");
        std::fs::write(&wanted, b"x").unwrap();
        std::fs::write(dir.join("2.1K views · 770 reactions ｜ The Office ｜ Cruzou é Gol [1681432380].mp4.part"), b"x").unwrap();
        std::fs::write(dir.join("Outro video [zzz].mp4"), b"x").unwrap();
        let found = find_output_by_title(&dir, "2.1K views · 770 reactions | The Office | Cruzou é Gol");
        assert_eq!(found.as_deref(), Some(wanted.as_path()));
        // Título como ficou no banco antes da correção (caracteres trocados por U+FFFD).
        let mangled = "2.1K views \u{FFFD} 770 reactions | The Office | Cruzou \u{FFFD} Gol";
        assert_eq!(find_output_by_title(&dir, mangled).as_deref(), Some(wanted.as_path()));
        assert!(find_output_by_title(&dir, "Nada a ver com isso").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
