//! Consulta inicial de um link: segue redirecionamentos validando cada salto
//! (política de rede interna), descobre nome, tamanho, suporte a retomada
//! (HTTP Range, RFC 9110 §14) e identificadores de versão (ETag/Last-Modified).

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use percent_encoding::percent_decode_str;
use reqwest::header::{self, HeaderMap};
use reqwest::{StatusCode, redirect};
use shift_security::{IpClass, RedirectVerdict, check_initial_target, check_redirect, classify_ip, sanitize_filename};
use url::Url;

use crate::diagnostics::{Diagnostic, DiagnosticKind, from_http_status, from_reqwest};

const MAX_REDIRECTS: u32 = 10;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeResult {
    pub source_url: Url,
    pub effective_url: Url,
    pub file_name: String,
    pub total_bytes: Option<u64>,
    /// Some(true): respondeu 206 a um Range. Some(false): ignorou o Range.
    pub resumable: Option<bool>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub content_type: Option<String>,
    pub redirects: u32,
}

pub async fn probe(url: &Url, user_agent: &str) -> Result<ProbeResult, Diagnostic> {
    let mut current = url.clone();
    let mut innermost: Option<IpClass> = None;
    let mut redirects = 0u32;
    let mut tried_without_range = false;

    loop {
        let host = current.host_str().ok_or_else(|| Diagnostic::new(DiagnosticKind::InvalidUrl, "sem servidor"))?.to_string();
        let port = current.port_or_known_default().unwrap_or(443);
        let addrs = resolve(&host, port).await?;
        let ips: Vec<IpAddr> = addrs.iter().map(SocketAddr::ip).collect();
        let verdict = match innermost {
            None => check_initial_target(&ips),
            Some(from) => check_redirect(from, &ips),
        };
        if let RedirectVerdict::Block(reason) = verdict {
            return Err(Diagnostic::new(DiagnosticKind::RedirectBlocked, reason));
        }
        // A classe "mais interna" já visitada (ordem: Public < Private < Loopback…).
        let hop_class = ips.iter().map(|ip| classify_ip(*ip)).max().unwrap_or(IpClass::Public);
        innermost = Some(innermost.map_or(hop_class, |c| c.max(hop_class)));

        // Conexão presa aos IPs já validados: evita que um segundo DNS (rebinding) troque o destino.
        let client = reqwest::Client::builder()
            .redirect(redirect::Policy::none())
            .resolve_to_addrs(&host, &addrs)
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(30))
            .user_agent(user_agent)
            .build()
            .map_err(|e| Diagnostic::new(DiagnosticKind::Unknown, e.to_string()))?;

        let mut req = client.get(current.clone()).header(header::ACCEPT_ENCODING, "identity");
        if !tried_without_range {
            req = req.header(header::RANGE, "bytes=0-0");
        }
        let res = req.send().await.map_err(|e| from_reqwest(&e))?;
        let status = res.status();

        if status.is_redirection() {
            let loc = res
                .headers()
                .get(header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or_else(|| Diagnostic::new(DiagnosticKind::ServerError, format!("redirecionamento {status} sem destino")))?;
            let next = current.join(loc).map_err(|_| Diagnostic::new(DiagnosticKind::InvalidUrl, "redirecionamento inválido"))?;
            if !matches!(next.scheme(), "http" | "https") {
                return Err(Diagnostic::new(
                    DiagnosticKind::RedirectBlocked,
                    format!("o site tentou redirecionar para o protocolo \"{}\"", next.scheme()),
                ));
            }
            redirects += 1;
            if redirects > MAX_REDIRECTS {
                return Err(Diagnostic::new(DiagnosticKind::TooManyRedirects, format!("mais de {MAX_REDIRECTS} saltos")));
            }
            current = next;
            continue;
        }

        if status == StatusCode::RANGE_NOT_SATISFIABLE && !tried_without_range {
            // Arquivo vazio ou servidor exigente: pergunta de novo sem Range.
            tried_without_range = true;
            continue;
        }

        if !status.is_success() {
            let kind = from_http_status(status.as_u16());
            let kind = if kind == DiagnosticKind::Unknown { DiagnosticKind::ServerError } else { kind };
            return Err(Diagnostic::new(kind, format!("HTTP {}", status.as_u16())));
        }

        let h = res.headers();
        let (total, resumable) = if status == StatusCode::PARTIAL_CONTENT {
            (content_range_total(h), Some(true))
        } else if tried_without_range {
            (content_length(h), accepts_ranges(h).then_some(true))
        } else {
            // 200 a um pedido com Range: o servidor ignora Range, não dá para retomar nem dividir.
            (content_length(h), Some(false))
        };
        let file_name = choose_name(h, &current, url);
        return Ok(ProbeResult {
            source_url: url.clone(),
            effective_url: current,
            file_name,
            total_bytes: total,
            resumable,
            etag: header_str(h, header::ETAG),
            last_modified: header_str(h, header::LAST_MODIFIED),
            content_type: header_str(h, header::CONTENT_TYPE).map(|c| c.split(';').next().unwrap_or("").trim().to_string()),
            redirects,
        });
    }
}

async fn resolve(host: &str, port: u16) -> Result<Vec<SocketAddr>, Diagnostic> {
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(ip) = bare.parse::<IpAddr>() {
        return Ok(vec![SocketAddr::new(ip, port)]);
    }
    let addrs: Vec<SocketAddr> = tokio::time::timeout(Duration::from_secs(10), tokio::net::lookup_host((bare, port)))
        .await
        .map_err(|_| Diagnostic::new(DiagnosticKind::Dns, format!("tempo esgotado ao resolver {bare}")))?
        .map_err(|e| Diagnostic::new(DiagnosticKind::Dns, format!("{bare}: {e}")))?
        .collect();
    if addrs.is_empty() {
        return Err(Diagnostic::new(DiagnosticKind::Dns, format!("{bare} não tem endereço")));
    }
    Ok(addrs)
}

fn header_str(h: &HeaderMap, name: header::HeaderName) -> Option<String> {
    h.get(name).and_then(|v| v.to_str().ok()).map(str::trim).filter(|s| !s.is_empty()).map(str::to_string)
}

fn content_length(h: &HeaderMap) -> Option<u64> {
    header_str(h, header::CONTENT_LENGTH)?.parse().ok()
}

fn accepts_ranges(h: &HeaderMap) -> bool {
    header_str(h, header::ACCEPT_RANGES).is_some_and(|v| v.eq_ignore_ascii_case("bytes"))
}

/// "bytes 0-0/12345" → 12345 ("*" = desconhecido).
fn content_range_total(h: &HeaderMap) -> Option<u64> {
    header_str(h, header::CONTENT_RANGE)?.rsplit('/').next()?.trim().parse().ok()
}

/// Nome do arquivo: Content-Disposition > último trecho da URL final (se tiver extensão)
/// > último trecho da URL original > nome do servidor.
fn choose_name(h: &HeaderMap, effective: &Url, original: &Url) -> String {
    if let Some(cd) = header_str(h, header::CONTENT_DISPOSITION)
        && let Some(name) = parse_content_disposition(&cd)
    {
        return sanitize_filename(&name);
    }
    let seg = |u: &Url| -> Option<String> {
        let last = u.path_segments()?.rev().find(|s| !s.is_empty())?;
        Some(percent_decode_str(last).decode_utf8_lossy().to_string())
    };
    let eff = seg(effective);
    if let Some(n) = eff.as_ref().filter(|n| n.contains('.')) {
        return sanitize_filename(n);
    }
    if let Some(n) = seg(original).filter(|n| n.contains('.')) {
        return sanitize_filename(&n);
    }
    sanitize_filename(&eff.unwrap_or_else(|| original.host_str().unwrap_or("download").to_string()))
}

/// RFC 6266: prefere `filename*=UTF-8''nome%20codificado` a `filename="nome"`.
pub fn parse_content_disposition(value: &str) -> Option<String> {
    let mut plain = None;
    for part in value.split(';').map(str::trim) {
        let Some((k, v)) = part.split_once('=') else { continue };
        let k = k.trim().to_ascii_lowercase();
        let v = v.trim();
        if k == "filename*" {
            let encoded = v.splitn(3, '\'').nth(2).unwrap_or(v);
            let decoded = percent_decode_str(encoded.trim_matches('"')).decode_utf8_lossy().to_string();
            if !decoded.is_empty() {
                return Some(decoded);
            }
        } else if k == "filename" {
            let s = v.trim_matches('"').replace("\\\"", "\"");
            if !s.is_empty() {
                plain = Some(s);
            }
        }
    }
    plain
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_disposition_prefere_utf8() {
        assert_eq!(parse_content_disposition("attachment; filename=\"a b.zip\""), Some("a b.zip".into()));
        assert_eq!(
            parse_content_disposition("attachment; filename=\"fallback.zip\"; filename*=UTF-8''relat%C3%B3rio%20final.pdf"),
            Some("relatório final.pdf".into())
        );
        assert_eq!(parse_content_disposition("inline"), None);
    }

    #[test]
    fn nome_vem_da_url_quando_nao_ha_cabecalho() {
        let h = HeaderMap::new();
        let orig = Url::parse("https://site.com/baixar/meu%20arquivo.iso?x=1").unwrap();
        assert_eq!(choose_name(&h, &orig, &orig), "meu arquivo.iso");
        let cdn = Url::parse("https://cdn.site.com/obj/8f2a9c").unwrap();
        assert_eq!(choose_name(&h, &cdn, &orig), "meu arquivo.iso", "URL final sem extensão: usa a original");
        let host_only = Url::parse("https://site.com/").unwrap();
        assert_eq!(choose_name(&h, &host_only, &host_only), "site.com");
    }

    #[test]
    fn nome_malicioso_no_cabecalho_e_neutralizado() {
        let mut h = HeaderMap::new();
        h.insert(header::CONTENT_DISPOSITION, "attachment; filename=\"..\\..\\Windows\\evil.dll\"".parse().unwrap());
        let u = Url::parse("https://x.com/a").unwrap();
        assert_eq!(choose_name(&h, &u, &u), "evil.dll");
    }
}
