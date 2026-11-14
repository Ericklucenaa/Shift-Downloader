//! Supervisão do processo aria2: inicia sem janela e sem shell, espera o RPC
//! responder, reinicia com espera crescente se o processo cair e encerra com
//! `saveSession` + `shutdown` quando o aplicativo fecha.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::client::Aria2Client;

#[derive(Debug, Clone)]
pub struct Aria2Config {
    pub binary: PathBuf,
    /// Arquivo de sessão do aria2 (lido na partida e salvo periodicamente).
    pub session_file: PathBuf,
    pub max_concurrent: u32,
    /// Conexões por download (split e max-connection-per-server).
    pub connections: u32,
    /// Limite global em bytes/s; 0 = sem limite.
    pub download_limit: u64,
    pub user_agent: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineEvent {
    Starting,
    Ready { version: String, restarts: u32 },
    /// O processo terminou sozinho; o supervisor vai tentar de novo.
    Crashed { code: Option<i32>, detail: String },
    /// Desistiu depois de falhas seguidas (binário ausente, porta, antivírus…).
    GaveUp(String),
    Stopped,
}

#[derive(Debug, thiserror::Error)]
pub enum SpawnError {
    #[error("não encontrei o aria2 em {0}")]
    Missing(String),
    #[error("não consegui iniciar o aria2: {0}")]
    Io(String),
    #[error("o aria2 encerrou ao iniciar: {0}")]
    EarlyExit(String),
    #[error("o aria2 não respondeu ao RPC em 10 s")]
    Timeout,
}

pub struct Aria2Process {
    client_rx: watch::Receiver<Option<Aria2Client>>,
    stop_tx: watch::Sender<bool>,
    task: Mutex<Option<JoinHandle<()>>>,
}

const MAX_FAST_FAILURES: u32 = 5;

impl Aria2Process {
    /// Inicia o supervisor. Eventos de estado do motor saem por `events`.
    pub fn spawn(config: Aria2Config, events: mpsc::UnboundedSender<EngineEvent>) -> Self {
        let (client_tx, client_rx) = watch::channel(None);
        let (stop_tx, stop_rx) = watch::channel(false);
        let task = tokio::spawn(supervise(config, client_tx, stop_rx, events));
        Self { client_rx, stop_tx, task: Mutex::new(Some(task)) }
    }

    /// Cliente do processo atual. Muda a cada reinício (porta e segredo novos).
    pub fn watch_client(&self) -> watch::Receiver<Option<Aria2Client>> {
        self.client_rx.clone()
    }

    pub fn client(&self) -> Option<Aria2Client> {
        self.client_rx.borrow().clone()
    }

    /// Salva a sessão, pede para o aria2 encerrar e espera o supervisor terminar.
    pub async fn stop(&self) {
        let _ = self.stop_tx.send(true);
        let task = self.task.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(t) = task {
            let _ = tokio::time::timeout(Duration::from_secs(8), t).await;
        }
    }
}

async fn supervise(
    cfg: Aria2Config,
    client_tx: watch::Sender<Option<Aria2Client>>,
    mut stop_rx: watch::Receiver<bool>,
    events: mpsc::UnboundedSender<EngineEvent>,
) {
    let mut restarts = 0u32;
    let mut fast_failures = 0u32;
    loop {
        if *stop_rx.borrow() {
            break;
        }
        let _ = events.send(EngineEvent::Starting);
        let started = Instant::now();
        match launch(&cfg).await {
            Ok(Launched { mut child, client, version, stderr }) => {
                let _ = client_tx.send(Some(client.clone()));
                let _ = events.send(EngineEvent::Ready { version, restarts });
                tokio::select! {
                    status = child.wait() => {
                        let _ = client_tx.send(None);
                        let detail = tail(&stderr);
                        let code = status.ok().and_then(|s| s.code());
                        tracing::warn!(?code, %detail, "aria2 encerrou inesperadamente");
                        let _ = events.send(EngineEvent::Crashed { code, detail });
                        restarts += 1;
                        if started.elapsed() < Duration::from_secs(15) { fast_failures += 1 } else { fast_failures = 0 }
                    }
                    _ = stop_rx.changed() => {
                        let _ = client_tx.send(None);
                        let _ = client.save_session().await;
                        let _ = client.shutdown().await;
                        if tokio::time::timeout(Duration::from_secs(5), child.wait()).await.is_err() {
                            let _ = child.kill().await;
                        }
                        break;
                    }
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "falha ao iniciar o aria2");
                let _ = events.send(EngineEvent::Crashed { code: None, detail: e.to_string() });
                fast_failures += 1;
                if matches!(e, SpawnError::Missing(_)) {
                    let _ = events.send(EngineEvent::GaveUp(e.to_string()));
                    let _ = stop_rx.wait_for(|s| *s).await;
                    break;
                }
            }
        }
        if fast_failures >= MAX_FAST_FAILURES {
            let _ = events.send(EngineEvent::GaveUp(
                "o motor de downloads falhou várias vezes seguidas ao iniciar".into(),
            ));
            let _ = stop_rx.wait_for(|s| *s).await;
            break;
        }
        // Espera crescente: 1, 2, 4, 8, 16 s.
        let backoff = Duration::from_secs(1 << fast_failures.min(4));
        tokio::select! {
            _ = tokio::time::sleep(backoff) => {}
            _ = stop_rx.changed() => break,
        }
    }
    let _ = events.send(EngineEvent::Stopped);
}

struct Launched {
    child: Child,
    client: Aria2Client,
    version: String,
    stderr: Arc<Mutex<VecDeque<String>>>,
}

fn tail(buf: &Arc<Mutex<VecDeque<String>>>) -> String {
    buf.lock().map(|b| b.iter().cloned().collect::<Vec<_>>().join(" | ")).unwrap_or_default()
}

fn free_port() -> std::io::Result<u16> {
    Ok(std::net::TcpListener::bind("127.0.0.1:0")?.local_addr()?.port())
}

/// Argumentos estruturados (nunca uma linha de comando montada como texto).
pub(crate) fn build_args(cfg: &Aria2Config, port: u16, secret: &str, parent_pid: u32) -> Vec<String> {
    let session = cfg.session_file.to_string_lossy().to_string();
    let conns = cfg.connections.clamp(1, 16);
    vec![
        "--enable-rpc=true".into(),
        "--rpc-listen-all=false".into(),
        format!("--rpc-listen-port={port}"),
        format!("--rpc-secret={secret}"),
        format!("--input-file={session}"),
        format!("--save-session={session}"),
        "--save-session-interval=5".into(),
        "--force-save=false".into(),
        "--continue=true".into(),
        "--auto-file-renaming=false".into(),
        "--allow-overwrite=false".into(),
        "--file-allocation=none".into(),
        format!("--max-concurrent-downloads={}", cfg.max_concurrent.clamp(1, 20)),
        format!("--split={conns}"),
        format!("--max-connection-per-server={conns}"),
        // Blocos de 1 MiB: arquivos médios também usam todas as conexões (com 4 MiB,
        // um arquivo de 20 MB usava no máximo 5).
        "--min-split-size=1M".into(),
        format!("--max-overall-download-limit={}", cfg.download_limit),
        "--max-tries=3".into(),
        "--retry-wait=2".into(),
        "--connect-timeout=15".into(),
        "--timeout=30".into(),
        "--remote-time=true".into(),
        "--check-certificate=true".into(),
        "--uri-selector=adaptive".into(),
        "--disk-cache=16M".into(),
        // BitTorrent: guarda os metadados de magnets para retomar sem baixar de novo.
        "--bt-save-metadata=true".into(),
        "--follow-torrent=true".into(),
        // O SHIFT guarda a própria cópia dos .torrent; não deixa cópias na pasta do usuário.
        "--rpc-save-upload-metadata=false".into(),
        // DHT pronta desde a partida (como o qBittorrent): pontos de entrada conhecidos e a
        // tabela salva entre sessões. Sem isso, magnets sem rastreador não achavam ninguém.
        "--enable-dht=true".into(),
        "--enable-dht6=true".into(),
        format!("--dht-file-path={}", cfg.session_file.with_file_name("dht.dat").display()),
        format!("--dht-file-path6={}", cfg.session_file.with_file_name("dht6.dat").display()),
        "--dht-entry-point=router.bittorrent.com:6881".into(),
        "--dht-entry-point6=router.bittorrent.com:6881".into(),
        "--enable-peer-exchange=true".into(),
        "--bt-enable-lpd=true".into(),
        "--bt-max-peers=100".into(),
        "--seed-ratio=1.0".into(),
        format!("--user-agent={}", cfg.user_agent),
        // Se o SHIFT morrer sem fechar o aria2, ele se encerra sozinho.
        format!("--stop-with-process={parent_pid}"),
        "--console-log-level=error".into(),
        "--summary-interval=0".into(),
        "--show-console-readout=false".into(),
        "--quiet=false".into(),
    ]
}

async fn launch(cfg: &Aria2Config) -> Result<Launched, SpawnError> {
    if !cfg.binary.is_file() {
        return Err(SpawnError::Missing(cfg.binary.display().to_string()));
    }
    if let Some(dir) = cfg.session_file.parent() {
        std::fs::create_dir_all(dir).map_err(|e| SpawnError::Io(e.to_string()))?;
    }
    if !cfg.session_file.exists() {
        std::fs::write(&cfg.session_file, b"").map_err(|e| SpawnError::Io(e.to_string()))?;
    }
    let port = free_port().map_err(|e| SpawnError::Io(e.to_string()))?;
    let secret = uuid::Uuid::new_v4().simple().to_string() + &uuid::Uuid::new_v4().simple().to_string();

    let mut cmd = Command::new(&cfg.binary);
    cmd.args(build_args(cfg, port, &secret, std::process::id()))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = cmd.spawn().map_err(|e| SpawnError::Io(e.to_string()))?;

    // Guarda as últimas linhas de erro para o diagnóstico (e esvazia o pipe).
    let stderr_buf = Arc::new(Mutex::new(VecDeque::with_capacity(20)));
    if let Some(err) = child.stderr.take() {
        let buf = stderr_buf.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(err).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let line = line.trim().to_string();
                if line.is_empty() {
                    continue;
                }
                if let Ok(mut b) = buf.lock() {
                    if b.len() == 20 {
                        b.pop_front();
                    }
                    b.push_back(line);
                }
            }
        });
    }

    let client = Aria2Client::new(port, &secret);
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Ok(Some(status)) = child.try_wait() {
            // dá tempo do leitor de stderr juntar a mensagem
            tokio::time::sleep(Duration::from_millis(100)).await;
            return Err(SpawnError::EarlyExit(format!("código {:?}: {}", status.code(), tail(&stderr_buf))));
        }
        if let Ok(version) = client.version().await {
            return Ok(Launched { child, client, version, stderr: stderr_buf });
        }
        if Instant::now() > deadline {
            let _ = child.kill().await;
            return Err(SpawnError::Timeout);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rpc_so_local_com_segredo_e_amarrado_ao_processo_pai() {
        let cfg = Aria2Config {
            binary: "aria2c".into(),
            session_file: "s.session".into(),
            max_concurrent: 3,
            connections: 99,
            download_limit: 0,
            user_agent: "SHIFT/0.1".into(),
        };
        let a = build_args(&cfg, 6801, "abc", 42);
        assert!(a.contains(&"--rpc-listen-all=false".to_string()));
        assert!(a.contains(&"--rpc-secret=abc".to_string()));
        assert!(a.contains(&"--stop-with-process=42".to_string()));
        assert!(a.contains(&"--allow-overwrite=false".to_string()));
        assert!(a.contains(&"--split=16".to_string()), "conexões limitadas a 16");
    }
}
