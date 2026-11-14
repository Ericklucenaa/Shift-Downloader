//! Ponte local para a extensão do navegador.
//!
//! Servidor HTTP mínimo em 127.0.0.1 (porta aleatória) que só aceita pedidos com o
//! token escrito em `bridge.json` na pasta de dados do usuário. Quem lê esse arquivo
//! é o "native messaging host" (o próprio SHIFT chamado pelo navegador); páginas da
//! web não conseguem lê-lo, então não conseguem mandar downloads para o SHIFT.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeInfo {
    pub port: u16,
    pub token: String,
    pub pid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalAdd {
    pub url: String,
    /// file | media
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub file_name: Option<String>,
    #[serde(default)]
    pub referrer: Option<String>,
}

pub fn info_path(data_dir: &Path) -> PathBuf {
    data_dir.join("bridge.json")
}

pub async fn start(app: AppHandle, data_dir: PathBuf) -> std::io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    let token = format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple());
    let info = BridgeInfo { port, token: token.clone(), pid: std::process::id() };
    std::fs::write(info_path(&data_dir), serde_json::to_vec(&info)?)?;
    tauri::async_runtime::spawn(async move {
        loop {
            let Ok((sock, _)) = listener.accept().await else { break };
            let app = app.clone();
            let token = token.clone();
            tauri::async_runtime::spawn(async move {
                let _ = handle(sock, app, &token).await;
            });
        }
    });
    Ok(())
}

async fn handle(mut sock: tokio::net::TcpStream, app: AppHandle, token: &str) -> std::io::Result<()> {
    let mut buf = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    let header_end = loop {
        let n = sock.read(&mut chunk).await?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i + 4;
        }
        if buf.len() > 16 * 1024 {
            return respond(&mut sock, 431, "{}").await;
        }
    };
    let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = head.lines();
    let first = lines.next().unwrap_or("");
    let headers: Vec<(String, String)> = lines
        .filter_map(|l| l.split_once(':').map(|(k, v)| (k.trim().to_ascii_lowercase(), v.trim().to_string())))
        .collect();
    let header = |k: &str| headers.iter().find(|(h, _)| h == k).map(|(_, v)| v.as_str());
    // Pedidos de páginas web trazem Origin; a ponte só atende o host nativo.
    if header("origin").is_some() || header("authorization") != Some(&format!("Bearer {token}")) {
        return respond(&mut sock, 403, r#"{"ok":false}"#).await;
    }
    let len: usize = header("content-length").and_then(|v| v.parse().ok()).unwrap_or(0).min(64 * 1024);
    while buf.len() < header_end + len {
        let n = sock.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let body = &buf[header_end..(header_end + len).min(buf.len())];
    match first.split_whitespace().take(2).collect::<Vec<_>>().as_slice() {
        ["GET", "/v1/ping"] => respond(&mut sock, 200, &format!(r#"{{"ok":true,"app":"shift","version":"{}"}}"#, env!("CARGO_PKG_VERSION"))).await,
        ["POST", "/v1/add"] => {
            let Ok(req) = serde_json::from_slice::<ExternalAdd>(body) else { return respond(&mut sock, 400, r#"{"ok":false}"#).await };
            if req.url.len() > 8192 || !(req.url.starts_with("http://") || req.url.starts_with("https://") || req.url.starts_with("magnet:")) {
                return respond(&mut sock, 400, r#"{"ok":false,"error":"link não suportado"}"#).await;
            }
            focus_main(&app);
            let _ = app.emit("shift://external-add", &req);
            respond(&mut sock, 200, r#"{"ok":true}"#).await
        }
        _ => respond(&mut sock, 404, r#"{"ok":false}"#).await,
    }
}

async fn respond(sock: &mut tokio::net::TcpStream, code: u16, body: &str) -> std::io::Result<()> {
    let msg = format!("HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
    sock.write_all(msg.as_bytes()).await?;
    sock.flush().await
}

pub fn focus_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}
