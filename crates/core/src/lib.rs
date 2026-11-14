//! Núcleo do SHIFT: gerenciador de tarefas, mídia, consulta de links e diagnósticos.
//! A camada Tauri só traduz comandos e eventos; toda regra fica aqui.

pub mod assistant;
pub mod category;
pub mod diagnostics;
pub mod hashing;
mod manager;
pub mod media;
pub mod probe;
pub mod schedule;
pub mod smartspeed;
pub mod speedometer;
pub mod torrent;
pub mod views;

pub use diagnostics::{Diagnostic, DiagnosticKind};
pub use manager::{AddRequest, BatchResult, CoreResult, EventSink, MAX_AUTO_RETRIES, Manager, ManagerConfig, TorrentMeta, TorrentRequest};
pub use media::{ConvertRequest, FileProbe, MediaDownloadRequest, MediaManager, MediaView, ToolsInfo};
pub use shift_media::ytdlp::MediaInfo;
pub use shift_persistence::TaskStatus;
pub use torrent::TorrentInfo;
pub use views::{EngineState, EventView, Insights, ProbeView, Settings, Snapshot, Stats, TaskView};
