//! Tipos enviados para a interface (IPC). Espelhados em `apps/desktop/src/lib/types.ts`.

use serde::{Deserialize, Serialize};
use shift_persistence::{DownloadRecord, EventRecord, TaskStatus};
use shift_security::redact_url;

use crate::diagnostics::Diagnostic;
use crate::schedule::Schedule;

/// Números ao vivo do motor que não ficam no banco.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LiveNumbers {
    pub speed: u64,
    pub connections: u32,
    pub upload_speed: u64,
    pub seeders: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskView {
    pub id: String,
    /// http | torrent | magnet
    pub kind: String,
    pub file_name: String,
    pub directory: String,
    pub host: String,
    /// URL para exibição: sem credenciais e sem query string.
    pub display_url: String,
    pub status: TaskStatus,
    pub total_bytes: Option<i64>,
    pub completed_bytes: i64,
    /// bytes/s medidos pelo motor (0 quando parado).
    pub speed: u64,
    pub connections: u32,
    pub eta_seconds: Option<u64>,
    pub resumable: Option<bool>,
    pub content_type: Option<String>,
    pub error: Option<Diagnostic>,
    pub retry_count: i64,
    pub next_retry_at: Option<i64>,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub category: String,
    pub tags: Vec<String>,
    pub sha256: Option<String>,
    pub expected_sha256: Option<String>,
    pub speed_limit: i64,
    pub priority: i64,
    pub paused_reason: Option<String>,
    pub upload_speed: u64,
    pub uploaded_bytes: i64,
    pub seeders: u32,
    /// Outro download concluído tem o mesmo SHA-256.
    pub duplicate: bool,
}

impl TaskView {
    pub fn from_record(r: &DownloadRecord, live: LiveNumbers) -> Self {
        let url = url::Url::parse(&r.source_url).ok();
        let active = r.status == TaskStatus::Downloading;
        let speed = if active { live.speed } else { 0 };
        let eta = match (r.total_bytes, speed) {
            (Some(t), s) if s > 0 && t > r.completed_bytes => Some(((t - r.completed_bytes) as u64).div_ceil(s)),
            _ => None,
        };
        let is_magnet = r.source_url.starts_with("magnet:");
        Self {
            id: r.id.clone(),
            kind: r.kind.clone(),
            file_name: r.file_name.clone(),
            directory: r.directory.clone(),
            host: if r.kind == "http" || r.kind.is_empty() {
                url.as_ref().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default()
            } else {
                "BitTorrent".into()
            },
            display_url: if is_magnet {
                // magnet:?xt=urn:btih:<hash> (sem rastreadores)
                r.source_url.split('&').next().unwrap_or("").chars().take(70).collect()
            } else if r.kind == "torrent" {
                std::path::Path::new(&r.source_url).file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
            } else {
                url.as_ref().map(redact_url).unwrap_or_default()
            },
            status: r.status,
            total_bytes: r.total_bytes,
            completed_bytes: r.completed_bytes,
            speed,
            connections: if active { live.connections } else { 0 },
            eta_seconds: eta,
            resumable: r.resumable,
            content_type: r.content_type.clone(),
            error: match (&r.error_kind, &r.error_detail) {
                (Some(k), d) => Some(Diagnostic::from_stored(k, d.as_deref().unwrap_or(""))),
                _ => None,
            },
            retry_count: r.retry_count,
            next_retry_at: r.next_retry_at,
            created_at: r.created_at,
            started_at: r.started_at,
            finished_at: r.finished_at,
            category: r.category.clone(),
            tags: vec![],
            sha256: r.sha256.clone(),
            expected_sha256: r.expected_sha256.clone(),
            speed_limit: r.speed_limit,
            priority: r.priority,
            paused_reason: r.paused_reason.clone(),
            upload_speed: if active { live.upload_speed } else { 0 },
            uploaded_bytes: r.uploaded_bytes,
            seeders: if active { live.seeders } else { 0 },
            duplicate: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "state")]
pub enum EngineState {
    Starting,
    Ready { version: String, restarts: u32 },
    Restarting { detail: String },
    Failed { detail: String },
    Stopped,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stats {
    pub download_speed: u64,
    pub upload_speed: u64,
    pub active: u32,
    pub queued: u32,
    pub paused: u32,
    pub completed: u32,
    pub failed: u32,
    /// Velocidade total, uma amostra por segundo (mais antiga primeiro), até 5 min.
    pub speed_history: Vec<u64>,
    /// A janela do agendamento está fechada agora (downloads aguardando o horário).
    pub schedule_blocked: bool,
}

fn yes() -> bool {
    true
}
fn balanced() -> String {
    "balanced".into()
}
fn nothing() -> String {
    "nothing".into()
}
fn one() -> f64 {
    1.0
}
fn two() -> u32 {
    2
}
/// Botões planos, como no Shift Control (o visual "liquid" continua como opção).
fn classic() -> String {
    "classic".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub download_dir: String,
    pub max_concurrent: u32,
    pub connections: u32,
    /// bytes/s; 0 = sem limite.
    pub speed_limit: u64,
    /// "system" | "light" | "dark"
    pub theme: String,
    /// "pt-BR" | "en" | "es"
    pub language: String,
    /// Salvar em subpastas por tipo (Vídeos, Documentos…).
    #[serde(default)]
    pub organize_by_type: bool,
    #[serde(default = "yes")]
    pub notify: bool,
    /// economy | balanced | max
    #[serde(default = "balanced")]
    pub speed_mode: String,
    /// Continuar compartilhando torrents depois de baixar, até esta razão (0 = não compartilhar).
    #[serde(default = "one")]
    pub seed_ratio: f64,
    #[serde(default)]
    pub schedule: Schedule,
    /// nothing | sleep | hibernate | shutdown
    #[serde(default = "nothing")]
    pub after_all: String,
    /// Link vindo do navegador: perguntar (abre a janela de adicionar) ou baixar direto.
    #[serde(default = "yes")]
    pub browser_confirm: bool,
    #[serde(default = "two")]
    pub max_media_jobs: u32,
    /// classic | liquid (aparência dos botões principais)
    #[serde(default = "classic")]
    pub button_style: String,
    /// A janela de boas-vindas da primeira abertura já foi respondida.
    #[serde(default)]
    pub onboarded: bool,
    /// Fechar a janela deixa o SHIFT na área de notificação (perto do relógio).
    #[serde(default = "yes")]
    pub close_to_tray: bool,
    /// Somar rastreadores públicos a magnets e torrents (acha participantes muito mais rápido).
    #[serde(default = "yes")]
    pub extra_trackers: bool,
}

impl Settings {
    pub fn defaults(download_dir: String) -> Self {
        Self {
            download_dir,
            max_concurrent: 3,
            connections: 8,
            speed_limit: 0,
            theme: "system".into(),
            language: "pt-BR".into(),
            organize_by_type: false,
            notify: true,
            speed_mode: balanced(),
            seed_ratio: 1.0,
            schedule: Schedule::default(),
            after_all: nothing(),
            browser_confirm: true,
            max_media_jobs: 2,
            button_style: classic(),
            onboarded: false,
            close_to_tray: true,
            extra_trackers: true,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProbeView {
    pub probe_id: String,
    pub host: String,
    pub display_url: String,
    pub file_name: String,
    pub directory: String,
    pub total_bytes: Option<u64>,
    pub resumable: Option<bool>,
    pub content_type: Option<String>,
    pub redirects: u32,
    pub free_space: Option<u64>,
    /// O nome sugerido foi alterado por já existir um arquivo igual.
    pub renamed: bool,
    pub category: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventView {
    pub at: i64,
    pub kind: String,
    pub from_status: Option<String>,
    pub to_status: Option<String>,
    pub detail: Option<String>,
}

impl From<EventRecord> for EventView {
    fn from(e: EventRecord) -> Self {
        Self { at: e.at, kind: e.kind, from_status: e.from_status, to_status: e.to_status, detail: e.detail }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub tasks: Vec<TaskView>,
    pub engine: EngineState,
    pub stats: Stats,
    pub settings: Settings,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HostStat {
    pub host: String,
    pub downloads: u32,
    pub bytes: i64,
    /// Média de bytes/s dos downloads concluídos (tempo entre início e fim).
    pub avg_speed: u64,
    pub failures: u32,
    /// Limite de conexões aplicado pelo SmartSpeed (None = sem limite próprio).
    pub connection_limit: Option<u32>,
    pub active_connections: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub id: String,
    /// info | warn
    pub level: String,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Insights {
    pub hosts: Vec<HostStat>,
    pub suggestions: Vec<Suggestion>,
}
