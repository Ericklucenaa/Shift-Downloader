//! Modelo persistido e a máquina de estados das tarefas.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    /// Na fila (esperando vaga no motor ou nova tentativa agendada).
    #[default]
    Queued,
    /// Consultando o servidor: tamanho, nome, suporte a retomada.
    Probing,
    Downloading,
    Paused,
    /// Conferindo o arquivo concluído.
    Verifying,
    /// Pós-processamento (conversão, extração) — fases seguintes.
    Processing,
    Completed,
    Failed,
    Cancelled,
}

impl TaskStatus {
    pub const ALL: [TaskStatus; 9] = [
        Self::Queued, Self::Probing, Self::Downloading, Self::Paused, Self::Verifying,
        Self::Processing, Self::Completed, Self::Failed, Self::Cancelled,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Probing => "probing",
            Self::Downloading => "downloading",
            Self::Paused => "paused",
            Self::Verifying => "verifying",
            Self::Processing => "processing",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.as_str() == s)
    }

    /// Estados finais: o motor não tem mais nada a fazer com a tarefa.
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Cancelled)
    }

    /// Estados em que a tarefa ocupa (ou deve ocupar) o motor.
    pub fn is_engine_bound(self) -> bool {
        matches!(self, Self::Queued | Self::Downloading | Self::Paused)
    }

    /// Transições permitidas. Qualquer outra é erro de programação ou corrida e é recusada.
    pub fn can_transition(self, to: Self) -> bool {
        use TaskStatus::*;
        if self == to {
            return false;
        }
        match self {
            Probing => matches!(to, Queued | Paused | Failed | Cancelled),
            Queued => matches!(to, Downloading | Paused | Failed | Cancelled | Completed | Verifying),
            Downloading => matches!(to, Queued | Paused | Verifying | Completed | Failed | Cancelled),
            Paused => matches!(to, Queued | Downloading | Failed | Cancelled | Completed | Verifying),
            Verifying => matches!(to, Completed | Failed | Processing),
            Processing => matches!(to, Completed | Failed),
            // Falha pode ser tentada de novo (volta à fila) ou cancelada.
            Failed => matches!(to, Queued | Cancelled),
            Completed | Cancelled => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct DownloadRecord {
    pub id: String,
    pub source_url: String,
    pub effective_url: String,
    pub protocol: String,
    pub file_name: String,
    pub directory: String,
    pub total_bytes: Option<i64>,
    pub completed_bytes: i64,
    pub status: TaskStatus,
    pub priority: i64,
    pub engine: String,
    pub engine_ref: Option<String>,
    pub resumable: Option<bool>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub content_type: Option<String>,
    pub connections: i64,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub retry_count: i64,
    pub next_retry_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    /// http | torrent | magnet
    pub kind: String,
    pub category: String,
    pub sha256: Option<String>,
    pub expected_sha256: Option<String>,
    pub speed_limit: i64,
    /// user | schedule (por que está pausado)
    pub paused_reason: Option<String>,
    pub torrent_meta: Option<String>,
    pub uploaded_bytes: i64,
}

/// Trabalho de mídia: download via yt-dlp ou conversão via FFmpeg.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct MediaJob {
    pub id: String,
    /// download | convert
    pub kind: String,
    pub source: String,
    pub title: String,
    pub output_dir: String,
    pub output_path: Option<String>,
    /// JSON com o formato escolhido ou o preset de conversão.
    pub options: String,
    /// queued | running | paused | completed | failed | cancelled
    pub status: String,
    pub progress: f64,
    pub total_bytes: Option<i64>,
    pub thumbnail: Option<String>,
    pub duration: Option<f64>,
    pub error_kind: Option<String>,
    pub error_detail: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventRecord {
    pub id: i64,
    pub download_id: String,
    pub at: i64,
    pub kind: String,
    pub from_status: Option<String>,
    pub to_status: Option<String>,
    pub detail: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use TaskStatus::*;

    #[test]
    fn nomes_ida_e_volta() {
        for s in TaskStatus::ALL {
            assert_eq!(TaskStatus::parse(s.as_str()), Some(s));
        }
        assert_eq!(TaskStatus::parse("x"), None);
    }

    #[test]
    fn transicoes_validas_e_invalidas() {
        assert!(Queued.can_transition(Downloading));
        assert!(Downloading.can_transition(Paused));
        assert!(Paused.can_transition(Downloading));
        assert!(Downloading.can_transition(Failed));
        assert!(Failed.can_transition(Queued));
        assert!(Verifying.can_transition(Completed));
        assert!(!Completed.can_transition(Downloading));
        assert!(!Cancelled.can_transition(Queued));
        assert!(!Failed.can_transition(Completed));
        assert!(!Downloading.can_transition(Downloading));
        assert!(!Verifying.can_transition(Paused));
    }
}
