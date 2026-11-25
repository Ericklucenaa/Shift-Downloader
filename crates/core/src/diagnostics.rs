//! Download Diagnostics: transforma falhas em causa + ação, em vez de "falhou".
//!
//! Fontes: códigos de saída do aria2 (seção EXIT STATUS do manual) e erros do
//! cliente HTTP usado na consulta inicial.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticKind {
    Dns,
    Connection,
    Timeout,
    Tls,
    HttpAuth,
    Forbidden,
    NotFound,
    Expired,
    ServerError,
    ServerBusy,
    TooSlow,
    DiskFull,
    Permission,
    FileExists,
    Io,
    ResumeUnsupported,
    RemoteChanged,
    RedirectBlocked,
    TooManyRedirects,
    Checksum,
    InvalidUrl,
    Engine,
    UnsupportedUrl,
    Drm,
    FormatUnavailable,
    NoVideo,
    MediaFailed,
    ToolMissing,
    Unknown,
}

impl DiagnosticKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dns => "dns",
            Self::Connection => "connection",
            Self::Timeout => "timeout",
            Self::Tls => "tls",
            Self::HttpAuth => "http_auth",
            Self::Forbidden => "forbidden",
            Self::NotFound => "not_found",
            Self::Expired => "expired",
            Self::ServerError => "server_error",
            Self::ServerBusy => "server_busy",
            Self::TooSlow => "too_slow",
            Self::DiskFull => "disk_full",
            Self::Permission => "permission",
            Self::FileExists => "file_exists",
            Self::Io => "io",
            Self::ResumeUnsupported => "resume_unsupported",
            Self::RemoteChanged => "remote_changed",
            Self::RedirectBlocked => "redirect_blocked",
            Self::TooManyRedirects => "too_many_redirects",
            Self::Checksum => "checksum",
            Self::InvalidUrl => "invalid_url",
            Self::Engine => "engine",
            Self::UnsupportedUrl => "unsupported_url",
            Self::Drm => "drm",
            Self::FormatUnavailable => "format_unavailable",
            Self::NoVideo => "no_video",
            Self::MediaFailed => "media_failed",
            Self::ToolMissing => "tool_missing",
            Self::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Self {
        ALL_KINDS.into_iter().find(|k| k.as_str() == s).unwrap_or(Self::Unknown)
    }

    /// Falhas passageiras: vale tentar de novo sozinho, com espera crescente.
    pub fn retryable(self) -> bool {
        matches!(self, Self::Dns | Self::Connection | Self::Timeout | Self::ServerError | Self::ServerBusy | Self::TooSlow | Self::Engine)
    }
}

const ALL_KINDS: [DiagnosticKind; 29] = [
    DiagnosticKind::UnsupportedUrl, DiagnosticKind::Drm, DiagnosticKind::FormatUnavailable, DiagnosticKind::NoVideo,
    DiagnosticKind::MediaFailed, DiagnosticKind::ToolMissing,
    DiagnosticKind::Dns, DiagnosticKind::Connection, DiagnosticKind::Timeout, DiagnosticKind::Tls,
    DiagnosticKind::HttpAuth, DiagnosticKind::Forbidden, DiagnosticKind::NotFound, DiagnosticKind::Expired,
    DiagnosticKind::ServerError, DiagnosticKind::ServerBusy, DiagnosticKind::TooSlow, DiagnosticKind::DiskFull,
    DiagnosticKind::Permission, DiagnosticKind::FileExists, DiagnosticKind::Io, DiagnosticKind::ResumeUnsupported,
    DiagnosticKind::RemoteChanged, DiagnosticKind::RedirectBlocked, DiagnosticKind::TooManyRedirects,
    DiagnosticKind::Checksum, DiagnosticKind::InvalidUrl, DiagnosticKind::Engine, DiagnosticKind::Unknown,
];

/// O que a interface mostra. `detail` é técnico e curto (sem URL nem credenciais).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub title: String,
    pub action: String,
    pub detail: String,
    pub retryable: bool,
}

impl Diagnostic {
    pub fn new(kind: DiagnosticKind, detail: impl Into<String>) -> Self {
        let (title, action) = texts(kind);
        Self { kind, title: title.into(), action: action.into(), detail: detail.into(), retryable: kind.retryable() }
    }

    /// Reconstrói a partir do que foi salvo no banco.
    pub fn from_stored(kind: &str, detail: &str) -> Self {
        Self::new(DiagnosticKind::parse(kind), detail)
    }
}

fn texts(kind: DiagnosticKind) -> (&'static str, &'static str) {
    use DiagnosticKind::*;
    match kind {
        Dns => ("Não foi possível encontrar o servidor", "Confira se o endereço está certo e se a internet está funcionando."),
        Connection => ("A conexão com o servidor caiu", "Verifique sua internet. O SHIFT tenta de novo sozinho e continua de onde parou."),
        Timeout => ("O servidor demorou demais para responder", "O SHIFT tenta de novo sozinho. Se continuar, o servidor pode estar sobrecarregado."),
        Tls => ("A conexão segura não pôde ser verificada", "O certificado do site é inválido ou a data do computador está errada. Não continue se não confiar no site."),
        HttpAuth => ("O servidor pediu login", "Este arquivo exige autenticação. Baixe pelo navegador com sua conta ou use um link público."),
        Forbidden => ("O servidor recusou o acesso (403)", "Links de download costumam expirar. Gere um link novo na página de origem e tente de novo."),
        NotFound => ("O arquivo não existe mais nesse endereço (404)", "O arquivo foi movido ou o link expirou. Copie o link de novo na página de origem."),
        Expired => ("O link expirou (410)", "Gere um link novo na página de origem e adicione o download de novo."),
        ServerError => ("Erro no servidor", "Problema do lado do site. O SHIFT tenta de novo sozinho em alguns instantes."),
        ServerBusy => ("Servidor ocupado", "O site limitou os pedidos por enquanto. O SHIFT espera e tenta de novo."),
        TooSlow => ("A velocidade caiu abaixo do mínimo", "O SHIFT tenta de novo. Se continuar, reduza as conexões nas configurações."),
        DiskFull => ("Espaço insuficiente no disco", "Libere espaço ou escolha outra pasta. Os arquivos que já existem não foram alterados."),
        Permission => ("Sem permissão para gravar na pasta", "Escolha outra pasta de destino ou verifique as permissões dela."),
        FileExists => ("Já existe um arquivo com esse nome", "Escolha outro nome. O SHIFT nunca sobrescreve arquivos existentes."),
        Io => ("Erro ao gravar o arquivo", "Verifique o disco de destino (desconectado, cheio ou com problema) e tente de novo."),
        ResumeUnsupported => ("O servidor não permite continuar de onde parou", "O download precisa recomeçar do início."),
        RemoteChanged => ("O arquivo mudou no servidor", "A parte já baixada era de outra versão e foi descartada. O download recomeça do início."),
        RedirectBlocked => ("Redirecionamento bloqueado", "O site tentou levar o download para um endereço interno da sua rede. Isso foi impedido por segurança."),
        TooManyRedirects => ("Redirecionamentos demais", "O link fica em ciclo de redirecionamento. Copie o link final do arquivo pelo navegador."),
        Checksum => ("O arquivo baixado não confere", "A verificação de integridade falhou. Baixe de novo."),
        InvalidUrl => ("Endereço inválido", "Cole o endereço completo do arquivo, começando com http:// ou https://."),
        Engine => ("O motor de downloads parou", "O SHIFT está reiniciando o motor. O download continua quando ele voltar."),
        UnsupportedUrl => ("Este site não tem suporte para vídeo/áudio", "Se for um link direto de arquivo, cole na barra de downloads em vez da área de mídia."),
        Drm => ("Conteúdo protegido por DRM", "O SHIFT não contorna proteção de conteúdo. Use o aplicativo ou site oficial."),
        FormatUnavailable => ("O formato escolhido não está disponível", "Analise o link de novo e escolha outra qualidade."),
        NoVideo => ("Este link não tem vídeo", "O post só tem fotos. O SHIFT baixa vídeo e áudio; salve as fotos pelo navegador."),
        MediaFailed => ("O processamento da mídia falhou", "O arquivo original não foi alterado. Veja o detalhe técnico e tente outro formato."),
        ToolMissing => ("Ferramenta de mídia não encontrada", "Reinstale o SHIFT: o yt-dlp ou o FFmpeg não está na pasta do programa."),
        Unknown => ("O download falhou", "Tente de novo. Os detalhes técnicos abaixo ajudam a entender o problema."),
    }
}

/// Status HTTP numa mensagem do aria2 ("...status=403" ou "...Not Found").
fn http_status_in(message: &str) -> Option<u16> {
    let m = message.to_ascii_lowercase();
    if let Some(i) = m.find("status=") {
        return m[i + 7..].chars().take_while(char::is_ascii_digit).collect::<String>().parse().ok();
    }
    None
}

pub fn from_http_status(status: u16) -> DiagnosticKind {
    use DiagnosticKind::*;
    match status {
        401 | 407 => HttpAuth,
        403 => Forbidden,
        404 => NotFound,
        410 => Expired,
        429 | 503 => ServerBusy,
        500..=599 => ServerError,
        _ => Unknown,
    }
}

/// Erro de download relatado pelo aria2 (`errorCode`/`errorMessage` do tellStatus).
pub fn from_aria2(code: &str, message: &str) -> Diagnostic {
    use DiagnosticKind::*;
    let lower = message.to_ascii_lowercase();
    let tls_hint = ["ssl", "tls", "certificate", "handshake", "schannel"].iter().any(|k| lower.contains(k));
    let kind = match code.parse::<u32>().unwrap_or(1) {
        2 => Timeout,
        3 | 4 => match http_status_in(message) {
            Some(410) => Expired,
            _ => NotFound,
        },
        5 => TooSlow,
        6 if tls_hint => Tls,
        6 => Connection,
        8 => ResumeUnsupported,
        9 => DiskFull,
        11..=13 => FileExists,
        15 | 17 => Io,
        16 | 18 => Permission,
        19 => Dns,
        22 => http_status_in(message).map(from_http_status).unwrap_or(ServerError),
        23 => TooManyRedirects,
        24 => HttpAuth,
        29 => ServerBusy,
        32 => Checksum,
        1 if tls_hint => Tls,
        1 => http_status_in(message).map(from_http_status).unwrap_or(Unknown),
        _ => Unknown,
    };
    let detail = if message.is_empty() { format!("aria2 código {code}") } else { format!("aria2 código {code}: {}", scrub(message)) };
    Diagnostic::new(kind, detail)
}

/// Erros do reqwest na consulta inicial.
pub fn from_reqwest(err: &reqwest::Error) -> Diagnostic {
    use DiagnosticKind::*;
    let chain = error_chain(err);
    let lower = chain.to_ascii_lowercase();
    let kind = if err.is_timeout() {
        Timeout
    } else if ["certificate", "ssl", "tls", "handshake", "schannel"].iter().any(|k| lower.contains(k)) {
        Tls
    } else if lower.contains("dns") || lower.contains("resolve") || lower.contains("no such host") {
        Dns
    } else if err.is_connect() || err.is_request() {
        Connection
    } else if err.is_redirect() {
        TooManyRedirects
    } else {
        Unknown
    };
    Diagnostic::new(kind, scrub(&chain))
}

fn error_chain(err: &dyn std::error::Error) -> String {
    let mut parts = vec![err.to_string()];
    let mut cur = err.source();
    while let Some(e) = cur {
        let s = e.to_string();
        if !parts.contains(&s) {
            parts.push(s);
        }
        cur = e.source();
    }
    parts.join(": ")
}

/// Mensagens de erro às vezes trazem a URL inteira (com tokens). Remove.
pub fn scrub(message: &str) -> String {
    message
        .split_whitespace()
        .map(|w| {
            let t = w.trim_matches(|c| c == '(' || c == ')' || c == ',' || c == '"' || c == '\'');
            if t.starts_with("http://") || t.starts_with("https://") {
                match url::Url::parse(t) {
                    Ok(u) => w.replace(t, &shift_security::redact_url(&u)),
                    Err(_) => "<url>".to_string(),
                }
            } else {
                w.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use DiagnosticKind::*;

    #[test]
    fn mapeia_codigos_do_aria2() {
        assert_eq!(from_aria2("19", "Name resolution failed").kind, Dns);
        assert_eq!(from_aria2("6", "Network problem has occurred. cause:Connection refused").kind, Connection);
        assert_eq!(from_aria2("6", "SSL/TLS handshake failure").kind, Tls);
        assert_eq!(from_aria2("3", "Resource not found").kind, NotFound);
        assert_eq!(from_aria2("22", "Unexpected status. status=410").kind, Expired);
        assert_eq!(from_aria2("22", "Unexpected status. status=403").kind, Forbidden);
        assert_eq!(from_aria2("22", "status=502").kind, ServerError);
        assert_eq!(from_aria2("9", "").kind, DiskFull);
        assert_eq!(from_aria2("24", "").kind, HttpAuth);
        assert_eq!(from_aria2("99", "?").kind, Unknown);
    }

    #[test]
    fn falhas_passageiras_sao_retentaveis() {
        assert!(from_aria2("6", "").retryable);
        assert!(from_aria2("2", "").retryable);
        assert!(!from_aria2("9", "").retryable, "disco cheio não se resolve tentando de novo");
        assert!(!from_aria2("22", "status=410").retryable);
    }

    #[test]
    fn detalhe_nao_vaza_tokens() {
        let d = from_aria2("3", "Not found: https://user:pw@cdn.x.com/f.zip?token=SEGREDO");
        assert!(!d.detail.contains("SEGREDO") && !d.detail.contains("pw"), "{}", d.detail);
        assert!(d.detail.contains("cdn.x.com/f.zip?…"));
    }

    #[test]
    fn tipo_ida_e_volta() {
        for k in ALL_KINDS {
            assert_eq!(DiagnosticKind::parse(k.as_str()), k);
        }
    }
}
