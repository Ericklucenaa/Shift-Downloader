//! Infraestrutura dos testes de aceitação: servidor HTTP local controlável e um
//! gerenciador real (aria2 de verdade) em pasta temporária.

#![allow(dead_code)]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use shift_core::{EngineState, EventSink, Manager, ManagerConfig, Stats, TaskStatus, TaskView};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

/// Conteúdo determinístico: o byte i vale (i*31 + 7) mod 251.
pub fn content(len: usize) -> Vec<u8> {
    (0..len).map(|i| ((i * 31 + 7) % 251) as u8).collect()
}

#[derive(Clone, Debug)]
pub struct Route {
    pub status: u16,
    pub body_len: usize,
    pub ranges: bool,
    /// bytes/s; 0 = sem limite
    pub throttle: usize,
    pub headers: Vec<(String, String)>,
    /// Depois de N respostas, passa a responder com este status (ex.: link que expira).
    pub expire_after: Option<(u32, u16)>,
    /// Anuncia este Content-Length sem enviar o corpo (teste de espaço em disco).
    pub fake_length: Option<u64>,
    /// Derruba a conexão uma única vez depois de enviar N bytes (queda de rede).
    pub drop_once_after: Option<usize>,
}

impl Route {
    pub fn file(len: usize) -> Self {
        Self { status: 200, body_len: len, ranges: true, throttle: 0, headers: vec![], expire_after: None, fake_length: None, drop_once_after: None }
    }
    pub fn no_ranges(mut self) -> Self {
        self.ranges = false;
        self
    }
    pub fn throttle(mut self, bps: usize) -> Self {
        self.throttle = bps;
        self
    }
    pub fn status(code: u16) -> Self {
        Self { status: code, ..Self::file(0) }
    }
    pub fn redirect(to: &str) -> Self {
        Self { status: 302, headers: vec![("Location".into(), to.into())], ..Self::file(0) }
    }
    pub fn header(mut self, k: &str, v: &str) -> Self {
        self.headers.push((k.into(), v.into()));
        self
    }
}

#[derive(Default)]
pub struct ServerLog {
    /// (caminho, início do Range pedido)
    pub requests: Vec<(String, Option<u64>)>,
}

pub struct TestServer {
    pub port: u16,
    pub log: Arc<Mutex<ServerLog>>,
}

impl TestServer {
    pub async fn start(routes: Vec<(&str, Route)>) -> Self {
        let routes: Arc<HashMap<String, (Route, AtomicU32)>> =
            Arc::new(routes.into_iter().map(|(p, r)| (p.to_string(), (r, AtomicU32::new(0)))).collect());
        let dropped = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let log = Arc::new(Mutex::new(ServerLog::default()));
        let log2 = log.clone();
        tokio::spawn(async move {
            loop {
                let Ok((sock, _)) = listener.accept().await else { break };
                let routes = routes.clone();
                let log = log2.clone();
                let dropped = dropped.clone();
                tokio::spawn(async move {
                    let _ = handle(sock, routes, log, dropped).await;
                });
            }
        });
        Self { port, log }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{}", self.port, path)
    }

    pub fn range_starts(&self, path: &str) -> Vec<u64> {
        self.log.lock().unwrap().requests.iter().filter(|(p, _)| p == path).filter_map(|(_, r)| *r).collect()
    }
}

async fn handle(
    mut sock: TcpStream,
    routes: Arc<HashMap<String, (Route, AtomicU32)>>,
    log: Arc<Mutex<ServerLog>>,
    dropped: Arc<std::sync::atomic::AtomicBool>,
) -> std::io::Result<()> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 2048];
    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = sock.read(&mut chunk).await?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
    }
    let req = String::from_utf8_lossy(&buf).to_string();
    let path = req.split_whitespace().nth(1).unwrap_or("/").split('?').next().unwrap_or("/").to_string();
    let range_start = req.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        if !k.eq_ignore_ascii_case("range") {
            return None;
        }
        v.trim().strip_prefix("bytes=")?.split('-').next()?.parse::<u64>().ok()
    });
    let range_end = req.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        if !k.eq_ignore_ascii_case("range") {
            return None;
        }
        v.trim().strip_prefix("bytes=")?.split('-').nth(1)?.parse::<u64>().ok()
    });
    log.lock().unwrap().requests.push((path.clone(), range_start));

    let Some((route, hits)) = routes.get(&path) else {
        sock.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await?;
        return Ok(());
    };
    let n = hits.fetch_add(1, Ordering::SeqCst);
    let mut status = route.status;
    if let Some((after, code)) = route.expire_after
        && n >= after
    {
        status = code;
    }
    let mut head = String::new();
    for (k, v) in &route.headers {
        head.push_str(&format!("{k}: {v}\r\n"));
    }
    if status != 200 {
        let reason = match status { 302 => "Found", 403 => "Forbidden", 404 => "Not Found", 410 => "Gone", _ => "Status" };
        sock.write_all(format!("HTTP/1.1 {status} {reason}\r\n{head}Content-Length: 0\r\nConnection: close\r\n\r\n").as_bytes()).await?;
        return Ok(());
    }
    if let Some(fake) = route.fake_length {
        sock.write_all(format!("HTTP/1.1 200 OK\r\n{head}Content-Length: {fake}\r\nConnection: close\r\n\r\n").as_bytes()).await?;
        return Ok(());
    }
    let body = content(route.body_len);
    let total = body.len() as u64;
    let (code, from, to) = match (route.ranges, range_start) {
        (true, Some(s)) if s < total => (206, s, range_end.map_or(total - 1, |e| e.min(total - 1))),
        _ => (200, 0, total.saturating_sub(1)),
    };
    let slice = if total == 0 { &body[..] } else { &body[from as usize..=to as usize] };
    let mut h = format!("HTTP/1.1 {} {}\r\n{head}Content-Length: {}\r\nConnection: close\r\n", code, if code == 206 { "Partial Content" } else { "OK" }, slice.len());
    if route.ranges {
        h.push_str("Accept-Ranges: bytes\r\n");
    }
    if code == 206 {
        h.push_str(&format!("Content-Range: bytes {from}-{to}/{total}\r\n"));
    }
    h.push_str("\r\n");
    sock.write_all(h.as_bytes()).await?;
    if let Some(n) = route.drop_once_after
        && slice.len() > n
        && !dropped.swap(true, Ordering::SeqCst)
    {
        sock.write_all(&slice[..n]).await?;
        sock.flush().await?;
        return Ok(()); // fecha a conexão no meio
    }
    if route.throttle == 0 {
        sock.write_all(slice).await?;
    } else {
        let piece = (route.throttle / 20).max(1);
        for part in slice.chunks(piece) {
            sock.write_all(part).await?;
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
    sock.flush().await?;
    Ok(())
}

pub struct NullSink;
impl EventSink for NullSink {
    fn tasks_changed(&self, _: &[TaskView]) {}
    fn tasks_removed(&self, _: &[String]) {}
    fn engine_changed(&self, _: &EngineState) {}
    fn stats(&self, _: &Stats) {}
}

pub fn aria2_binary() -> PathBuf {
    if let Ok(p) = std::env::var("SHIFT_ARIA2") {
        return p.into();
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/desktop/src-tauri/binaries/aria2c-x86_64-pc-windows-msvc.exe")
}

pub fn tools_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/desktop/src-tauri/tools")
}

pub struct Env {
    pub dir: tempfile::TempDir,
}

impl Env {
    pub fn new() -> Self {
        Self { dir: tempfile::tempdir().unwrap() }
    }
    pub fn downloads(&self) -> PathBuf {
        self.dir.path().join("Downloads")
    }
    pub fn config(&self) -> ManagerConfig {
        ManagerConfig {
            db_path: self.dir.path().join("data/shift.db"),
            aria2_binary: aria2_binary(),
            session_file: self.dir.path().join("data/aria2.session"),
            default_download_dir: self.downloads(),
            user_agent: "SHIFT-tests".into(),
            tools_dir: Some(tools_dir()),
        }
    }
    pub async fn manager(&self) -> Manager {
        std::fs::create_dir_all(self.downloads()).unwrap();
        let m = Manager::start(self.config(), Arc::new(NullSink)).await.unwrap();
        wait_engine(&m).await;
        m
    }
}

pub async fn wait_engine(m: &Manager) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if matches!(m.snapshot().unwrap().engine, EngineState::Ready { .. }) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("motor não ficou pronto: {:?}", m.snapshot().unwrap().engine);
}

pub fn task(m: &Manager, id: &str) -> TaskView {
    m.snapshot().unwrap().tasks.into_iter().find(|t| t.id == id).expect("tarefa existe")
}

pub async fn wait_for(m: &Manager, id: &str, secs: u64, what: &str, f: impl Fn(&TaskView) -> bool) -> TaskView {
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        let t = task(m, id);
        if f(&t) {
            return t;
        }
        if Instant::now() > deadline {
            panic!("esperando {what}; estado atual: {:?} {}/{:?} erro={:?}", t.status, t.completed_bytes, t.total_bytes, t.error);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

pub async fn wait_status(m: &Manager, id: &str, s: TaskStatus, secs: u64) -> TaskView {
    wait_for(m, id, secs, &format!("{s:?}"), |t| t.status == s).await
}
