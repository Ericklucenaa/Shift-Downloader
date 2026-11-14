//! Mídia: yt-dlp (análise e download de vídeo/áudio de sites compatíveis) e
//! FFmpeg (conversão local). Este crate só monta argumentos e interpreta saídas;
//! quem executa os processos é o núcleo (`shift-core::media`).
//!
//! Limites deliberados: nada de contornar DRM, paywall ou login de terceiros.

pub mod ffmpeg;
pub mod ytdlp;

/// Erro classificado a partir da saída de uma ferramenta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolError {
    /// Mesmo vocabulário do `DiagnosticKind` do núcleo (ex.: "unsupported_url").
    pub kind: &'static str,
    pub detail: String,
}

/// Últimas linhas relevantes de stderr, sem caminhos de usuário longos demais.
pub fn tail(lines: &[String], n: usize) -> String {
    lines.iter().rev().filter(|l| !l.trim().is_empty()).take(n).cloned().collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join(" | ")
}
