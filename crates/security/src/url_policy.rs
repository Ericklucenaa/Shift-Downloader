//! Validação de URLs digitadas e redação para logs.

use url::Url;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum UrlError {
    #[error("Cole um endereço de download.")]
    Empty,
    #[error("Este endereço não é válido.")]
    Invalid,
    #[error("O protocolo \"{0}\" ainda não é suportado. Use http:// ou https://.")]
    UnsupportedScheme(String),
    #[error("O endereço não tem um servidor.")]
    NoHost,
}

/// Aceita o que o usuário cola: com ou sem esquema, com espaços nas pontas.
/// Fase 1: só HTTP e HTTPS (FTP, SFTP, magnet e torrent entram por adaptadores próprios).
pub fn parse_download_url(input: &str) -> Result<Url, UrlError> {
    let s = input.trim();
    if s.is_empty() {
        return Err(UrlError::Empty);
    }
    let with_scheme = if s.contains("://") || s.starts_with("magnet:") { s.to_string() } else { format!("https://{s}") };
    let url = Url::parse(&with_scheme).map_err(|_| UrlError::Invalid)?;
    match url.scheme() {
        "http" | "https" => {}
        other => return Err(UrlError::UnsupportedScheme(other.to_string())),
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(UrlError::NoHost);
    }
    Ok(url)
}

/// Versão para logs e eventos: sem usuário/senha e sem a query string (que costuma
/// carregar tokens de acesso e assinaturas de links temporários).
pub fn redact_url(url: &Url) -> String {
    let mut u = url.clone();
    let _ = u.set_username("");
    let _ = u.set_password(None);
    let had_query = u.query().is_some_and(|q| !q.is_empty());
    u.set_query(None);
    u.set_fragment(None);
    let mut s = u.to_string();
    if had_query {
        s.push_str("?…");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aceita_urls_coladas_sem_esquema() {
        assert_eq!(parse_download_url("  example.com/a.zip ").unwrap().as_str(), "https://example.com/a.zip");
        assert_eq!(parse_download_url("http://host:8080/x").unwrap().port(), Some(8080));
    }

    #[test]
    fn recusa_entradas_invalidas() {
        assert_eq!(parse_download_url(""), Err(UrlError::Empty));
        assert_eq!(parse_download_url("ftp://x/y"), Err(UrlError::UnsupportedScheme("ftp".into())));
        assert_eq!(parse_download_url("file:///C:/Windows/win.ini"), Err(UrlError::UnsupportedScheme("file".into())));
        assert!(parse_download_url("javascript:alert(1)").is_err());
        assert!(parse_download_url("magnet:?xt=urn:btih:abc").is_err());
        assert!(parse_download_url("http://").is_err());
    }

    #[test]
    fn redacao_remove_credenciais_e_tokens() {
        let u = Url::parse("https://user:senha@cdn.example.com/f.zip?token=abc&sig=xyz#frag").unwrap();
        let r = redact_url(&u);
        assert_eq!(r, "https://cdn.example.com/f.zip?…");
        assert!(!r.contains("senha") && !r.contains("abc"));
    }
}
