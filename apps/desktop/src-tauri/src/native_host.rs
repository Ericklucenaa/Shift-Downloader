//! Modo "native messaging host": o navegador inicia o Shift Downloader com a origem da extensão
//! e troca mensagens JSON por stdin/stdout (4 bytes de tamanho + JSON).
//! Este modo não abre janela; ele repassa pedidos para o app (ponte local com token)
//! e, se o app estiver fechado, abre o app primeiro.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use crate::bridge::BridgeInfo;

const MAX_MESSAGE: usize = 1024 * 1024;

fn data_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join("app.shift.downloads"))
}

fn read_message(input: &mut impl Read) -> Option<Value> {
    let mut len = [0u8; 4];
    input.read_exact(&mut len).ok()?;
    let len = u32::from_le_bytes(len) as usize;
    if len > MAX_MESSAGE {
        return None;
    }
    let mut buf = vec![0u8; len];
    input.read_exact(&mut buf).ok()?;
    serde_json::from_slice(&buf).ok()
}

fn write_message(out: &mut impl Write, v: &Value) {
    let data = serde_json::to_vec(v).unwrap_or_default();
    let _ = out.write_all(&(data.len() as u32).to_le_bytes());
    let _ = out.write_all(&data);
    let _ = out.flush();
}

fn bridge() -> Option<BridgeInfo> {
    let raw = std::fs::read(data_dir()?.join("bridge.json")).ok()?;
    serde_json::from_slice(&raw).ok()
}

/// HTTP/1.1 mínimo até a ponte local (sem dependências de rede no host).
fn call(info: &BridgeInfo, method: &str, path: &str, body: Option<&Value>) -> Option<Value> {
    let mut s = TcpStream::connect_timeout(&([127, 0, 0, 1], info.port).into(), Duration::from_secs(2)).ok()?;
    s.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let body = body.map(|b| b.to_string()).unwrap_or_default();
    let req = format!(
        "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        info.token,
        body.len()
    );
    s.write_all(req.as_bytes()).ok()?;
    let mut resp = String::new();
    s.read_to_string(&mut resp).ok()?;
    let (head, json) = resp.split_once("\r\n\r\n")?;
    if !head.starts_with("HTTP/1.1 200") {
        return None;
    }
    serde_json::from_str(json).ok()
}

fn running() -> Option<BridgeInfo> {
    let info = bridge()?;
    call(&info, "GET", "/v1/ping", None).map(|_| info)
}

/// Abre o Shift Downloader (processo separado, sem herdar stdin/stdout do navegador) e espera a ponte.
fn launch_app() -> Option<BridgeInfo> {
    let exe = std::env::current_exe().ok()?;
    let mut cmd = std::process::Command::new(exe);
    cmd.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    cmd.spawn().ok()?;
    let deadline = Instant::now() + Duration::from_secs(25);
    while Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(300));
        if let Some(i) = running() {
            return Some(i);
        }
    }
    None
}

pub fn run() -> i32 {
    let mut stdin = std::io::stdin().lock();
    let mut stdout = std::io::stdout().lock();
    while let Some(msg) = read_message(&mut stdin) {
        let reply = match msg.get("type").and_then(Value::as_str) {
            Some("ping") => match running() {
                Some(_) => json!({ "ok": true, "running": true, "version": env!("CARGO_PKG_VERSION") }),
                None => json!({ "ok": true, "running": false, "version": env!("CARGO_PKG_VERSION") }),
            },
            Some("add") => {
                let url = msg.get("url").and_then(Value::as_str).unwrap_or("");
                if !(url.starts_with("http://") || url.starts_with("https://") || url.starts_with("magnet:")) {
                    json!({ "ok": false, "error": "link não suportado" })
                } else {
                    let info = running().or_else(launch_app);
                    match info {
                        Some(i) => {
                            let body = json!({
                                "url": url,
                                "kind": msg.get("kind"),
                                "fileName": msg.get("fileName"),
                                "referrer": msg.get("referrer"),
                            });
                            match call(&i, "POST", "/v1/add", Some(&body)) {
                                Some(_) => json!({ "ok": true }),
                                None => json!({ "ok": false, "error": "o Shift Downloader recusou o pedido" }),
                            }
                        }
                        None => json!({ "ok": false, "error": "não consegui abrir o Shift Downloader" }),
                    }
                }
            }
            _ => json!({ "ok": false, "error": "mensagem desconhecida" }),
        };
        write_message(&mut stdout, &reply);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocolo_de_mensagens_ida_e_volta() {
        let mut buf = Vec::new();
        write_message(&mut buf, &json!({"type": "ping"}));
        assert_eq!(u32::from_le_bytes(buf[..4].try_into().unwrap()) as usize, buf.len() - 4);
        let back = read_message(&mut std::io::Cursor::new(buf)).unwrap();
        assert_eq!(back["type"], "ping");
        // Tamanho absurdo é recusado sem alocar.
        let mut huge = (u32::MAX).to_le_bytes().to_vec();
        huge.extend_from_slice(b"{}");
        assert!(read_message(&mut std::io::Cursor::new(huge)).is_none());
    }
}
