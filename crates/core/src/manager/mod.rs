//! Gerenciador de tarefas: fonte da verdade do estado (SQLite) sincronizada com o
//! motor (aria2). Toda operação que mexe no motor passa por `op_lock`, então o
//! ciclo de acompanhamento e os comandos do usuário nunca se atropelam.

mod add;
mod ops;
mod sync;

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock, Weak};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use shift_download_engine::{Aria2Client, Aria2Config, Aria2Process, EngineEvent};
use shift_persistence::{Db, DownloadRecord, TaskStatus};
use tokio::sync::{mpsc, watch};

use crate::diagnostics::{Diagnostic, DiagnosticKind};
use crate::media::{MediaManager, MediaView};
use crate::probe::ProbeResult;
use crate::views::{EngineState, Insights, LiveNumbers, Settings, Snapshot, Stats, TaskView};

pub use add::{AddRequest, BatchResult, TorrentRequest};

pub type CoreResult<T> = Result<T, Diagnostic>;

/// Novas tentativas automáticas para falhas passageiras: 5, 10, 20, 40, 80 s.
pub const MAX_AUTO_RETRIES: i64 = 5;
const RETRY_BASE_MS: i64 = 5_000;
const POLL_EVERY: Duration = Duration::from_millis(500);
const PERSIST_EVERY: Duration = Duration::from_secs(2);
const HISTORY_LEN: usize = 300;
/// Folga mínima no disco além do tamanho do arquivo.
const DISK_MARGIN: u64 = 16 * 1024 * 1024;
const PROBE_TTL: Duration = Duration::from_secs(600);

/// Para onde vão as mudanças (a camada Tauri emite eventos para a interface).
pub trait EventSink: Send + Sync + 'static {
    fn tasks_changed(&self, tasks: &[TaskView]);
    fn tasks_removed(&self, ids: &[String]);
    fn engine_changed(&self, state: &EngineState);
    fn stats(&self, stats: &Stats);
    /// Download terminou (concluído ou falha definitiva): notificação do sistema.
    fn task_finished(&self, _task: &TaskView) {}
    /// Tudo terminou e há uma ação configurada (suspender, desligar…).
    fn after_all(&self, _action: &str) {}
    fn media_changed(&self, _jobs: &[MediaView]) {}
    fn media_removed(&self, _ids: &[String]) {}
    fn media_finished(&self, _job: &MediaView) {}
}

#[derive(Debug, Clone)]
pub struct ManagerConfig {
    pub db_path: PathBuf,
    pub aria2_binary: PathBuf,
    pub session_file: PathBuf,
    pub default_download_dir: PathBuf,
    pub user_agent: String,
    /// Pasta com yt-dlp.exe e ffmpeg/ (None = mídia indisponível).
    pub tools_dir: Option<PathBuf>,
}

#[derive(Default)]
struct Live {
    numbers: LiveNumbers,
    /// Bytes ao vivo do motor (o banco só é gravado a cada PERSIST_EVERY).
    bytes: Option<(i64, Option<i64>)>,
    last_persist: Option<Instant>,
    last_view: Option<TaskView>,
    /// Velocidade pelos bytes que chegaram (o downloadSpeed do aria2 demora a reagir).
    speedo: crate::speedometer::Speedometer,
}

/// Etiquetas e duplicados em memória (mudam pouco; evita consultar a cada 500 ms).
#[derive(Default)]
struct MetaCache {
    tags: HashMap<String, Vec<String>>,
    duplicates: HashSet<String>,
}

struct Inner {
    db: Arc<Db>,
    sink: Arc<dyn EventSink>,
    engine: Aria2Process,
    user_agent: String,
    data_dir: PathBuf,
    settings: Arc<RwLock<Settings>>,
    engine_state: Mutex<EngineState>,
    live: Mutex<HashMap<String, Live>>,
    history: Mutex<VecDeque<u64>>,
    probes: Mutex<HashMap<String, (Instant, ProbeResult)>>,
    meta: Mutex<MetaCache>,
    host_limits: Mutex<BTreeMap<String, u32>>,
    op_lock: tokio::sync::Mutex<()>,
    hash_lock: Arc<tokio::sync::Semaphore>,
    last_speed: AtomicU64,
    last_upload: AtomicU64,
    had_activity: AtomicBool,
    after_all_fired: AtomicBool,
    schedule_blocked: AtomicBool,
    stop_tx: watch::Sender<bool>,
    me: OnceLock<Weak<Inner>>,
    media: OnceLock<MediaManager>,
}

#[derive(Clone)]
pub struct Manager {
    inner: Arc<Inner>,
}

pub(crate) fn db_err(e: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::new(DiagnosticKind::Unknown, format!("banco de dados: {e}"))
}

pub(crate) fn rpc_diag(e: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::new(DiagnosticKind::Engine, e.to_string())
}

pub(crate) fn state_err(msg: &str) -> Diagnostic {
    let mut d = Diagnostic::new(DiagnosticKind::Unknown, msg);
    d.title = msg.to_string();
    d.action = String::new();
    d
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TorrentMeta {
    pub info_hash: String,
    pub files: Vec<crate::torrent::TorrentFile>,
    pub selected: Vec<usize>,
    /// Pasta-invólucro criada pelo SHIFT para evitar conflito de nome.
    #[serde(default)]
    pub wrapper: Option<String>,
}

impl Manager {
    /// Abre o banco, sobe o aria2 supervisionado e começa a acompanhar as tarefas.
    /// Precisa ser chamado dentro de um runtime Tokio.
    pub async fn start(cfg: ManagerConfig, sink: Arc<dyn EventSink>) -> CoreResult<Self> {
        let data_dir = cfg.db_path.parent().map(Path::to_path_buf).unwrap_or_default();
        std::fs::create_dir_all(&data_dir).map_err(|e| Diagnostic::new(DiagnosticKind::Permission, e.to_string()))?;
        let db = Arc::new(Db::open(&cfg.db_path).map_err(db_err)?);
        let defaults = Settings::defaults(cfg.default_download_dir.to_string_lossy().to_string());
        let settings: Settings = db.setting("settings").map_err(db_err)?.unwrap_or(defaults);
        let host_limits: BTreeMap<String, u32> = db.setting("host_limits").ok().flatten().unwrap_or_default();

        let (ev_tx, ev_rx) = mpsc::unbounded_channel();
        let engine = Aria2Process::spawn(
            Aria2Config {
                binary: cfg.aria2_binary.clone(),
                session_file: cfg.session_file.clone(),
                max_concurrent: settings.max_concurrent,
                connections: settings.connections,
                download_limit: settings.speed_limit,
                user_agent: cfg.user_agent.clone(),
            },
            ev_tx,
        );
        let (stop_tx, stop_rx) = watch::channel(false);
        let settings = Arc::new(RwLock::new(settings));
        let inner = Arc::new(Inner {
            db: db.clone(),
            sink: sink.clone(),
            engine,
            user_agent: cfg.user_agent,
            data_dir: data_dir.clone(),
            settings: settings.clone(),
            engine_state: Mutex::new(EngineState::Starting),
            live: Mutex::default(),
            history: Mutex::new(VecDeque::with_capacity(HISTORY_LEN)),
            probes: Mutex::default(),
            meta: Mutex::default(),
            host_limits: Mutex::new(host_limits),
            op_lock: tokio::sync::Mutex::new(()),
            hash_lock: Arc::new(tokio::sync::Semaphore::new(1)),
            last_speed: AtomicU64::new(0),
            last_upload: AtomicU64::new(0),
            had_activity: AtomicBool::new(false),
            after_all_fired: AtomicBool::new(false),
            schedule_blocked: AtomicBool::new(false),
            stop_tx,
            me: OnceLock::new(),
            media: OnceLock::new(),
        });
        let _ = inner.me.set(Arc::downgrade(&inner));
        let _ = inner.media.set(MediaManager::start(db, sink, settings, cfg.tools_dir, stop_rx.clone()));
        inner.refresh_meta();
        inner.resume_pending_hashes();
        tokio::spawn(engine_events(inner.clone(), ev_rx));
        tokio::spawn(reconcile_on_ready(inner.clone(), inner.engine.watch_client(), stop_rx.clone()));
        tokio::spawn(poll_loop(inner.clone(), stop_rx));
        Ok(Self { inner })
    }

    pub fn media(&self) -> &MediaManager {
        self.inner.media.get().expect("mídia iniciada")
    }

    pub fn snapshot(&self) -> CoreResult<Snapshot> {
        let records = self.inner.db.list().map_err(db_err)?;
        let tasks = records.iter().map(|r| self.inner.view_of(r)).collect();
        Ok(Snapshot {
            tasks,
            engine: lock(&self.inner.engine_state).clone(),
            stats: self.inner.stats(),
            settings: self.settings(),
        })
    }

    pub fn settings(&self) -> Settings {
        self.inner.settings.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn events(&self, id: &str) -> CoreResult<Vec<crate::views::EventView>> {
        Ok(self.inner.db.events(id, 100).map_err(db_err)?.into_iter().map(Into::into).collect())
    }

    /// Caminho do arquivo (ou pasta, em torrents) da tarefa, conferindo que existe.
    pub fn file_path(&self, id: &str) -> CoreResult<PathBuf> {
        let r = self.inner.require(id)?;
        let path = Path::new(&r.directory).join(&r.file_name);
        if !path.exists() {
            return Err(state_err("O arquivo não está mais na pasta de destino."));
        }
        Ok(path)
    }

    pub fn source_url(&self, id: &str) -> CoreResult<String> {
        Ok(self.inner.require(id)?.source_url)
    }

    pub fn insights(&self) -> CoreResult<Insights> {
        self.inner.insights()
    }

    pub fn host_limits(&self) -> BTreeMap<String, u32> {
        lock(&self.inner.host_limits).clone()
    }

    pub fn reset_host_limits(&self) -> CoreResult<()> {
        lock(&self.inner.host_limits).clear();
        self.inner.db.set_setting("host_limits", &BTreeMap::<String, u32>::new()).map_err(db_err)
    }

    /// Salva o progresso e encerra o aria2 (que salva a própria sessão) e os processos de mídia.
    pub async fn shutdown(&self) {
        let _ = self.inner.stop_tx.send(true);
        if let Some(m) = self.inner.media.get() {
            m.shutdown().await;
        }
        let _guard = self.inner.op_lock.lock().await;
        if let Some(client) = self.inner.engine.client() {
            let records = self.inner.db.list_engine_bound().unwrap_or_default();
            let gids: Vec<String> = records.iter().filter_map(|r| r.engine_ref.clone()).collect();
            if let Ok(statuses) = client.tell_many(&gids).await {
                for (r, s) in records.iter().zip(statuses) {
                    if let Some(s) = s
                        && (s.completed_length as i64) >= r.completed_bytes
                    {
                        let _ = self.inner.db.update_progress(&r.id, s.completed_length as i64, (s.total_length > 0).then_some(s.total_length as i64));
                    }
                }
            }
        }
        self.inner.engine.stop().await;
    }
}

pub(crate) fn global_options(s: &Settings) -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("max-concurrent-downloads".into(), json!(s.max_concurrent.to_string()));
    m.insert("max-overall-download-limit".into(), json!(s.speed_limit.to_string()));
    m.insert("seed-ratio".into(), json!(format!("{:.2}", s.seed_ratio)));
    m
}

pub(crate) fn human(b: u64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = b as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 { format!("{b} B") } else { format!("{v:.1} {}", U[i]) }
}

fn check_disk_space(dir: &Path, total: Option<u64>) -> CoreResult<()> {
    let (Some(total), Ok(free)) = (total, fs4::available_space(dir)) else { return Ok(()) };
    if free < total.saturating_add(DISK_MARGIN) {
        return Err(Diagnostic::new(
            DiagnosticKind::DiskFull,
            format!("o arquivo tem {} e a pasta tem {} livres", human(total), human(free)),
        ));
    }
    Ok(())
}

fn host_of(url: &str) -> String {
    url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_ascii_lowercase)).unwrap_or_default()
}

/// GID original da tarefa (derivado do UUID): estável entre reinícios.
fn original_gid(id: &str) -> String {
    uuid::Uuid::parse_str(id).map(|u| shift_download_engine::gid_from_bytes(u.as_bytes())).unwrap_or_default()
}

fn remove_with_retry(p: &Path) {
    for attempt in 0..10 {
        let r = if p.is_dir() { std::fs::remove_dir_all(p) } else { std::fs::remove_file(p) };
        match r {
            Ok(()) => return,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return,
            Err(e) => {
                if attempt == 9 {
                    tracing::warn!(error = %e, "não consegui apagar a parte baixada");
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

/// Só o arquivo de controle `.aria2` (o download em si fica).
fn remove_control_file(r: &DownloadRecord) {
    let mut control = Path::new(&r.directory).join(&r.file_name).into_os_string();
    control.push(".aria2");
    remove_with_retry(Path::new(&control));
}

/// Apaga a parte baixada e o arquivo de controle. No Windows o aria2 pode segurar
/// o arquivo por alguns instantes depois de remover a tarefa. Nunca apaga concluídos.
fn remove_partial(r: &DownloadRecord) {
    if r.status == TaskStatus::Completed {
        return;
    }
    let dir = Path::new(&r.directory);
    let target = dir.join(&r.file_name);
    let mut control = target.clone().into_os_string();
    control.push(".aria2");
    remove_with_retry(&target);
    remove_with_retry(Path::new(&control));
    if r.kind != "http"
        && let Some(meta) = r.torrent_meta.as_deref().and_then(|m| serde_json::from_str::<TorrentMeta>(m).ok())
    {
        remove_with_retry(&dir.join(format!("{}.torrent", meta.info_hash)));
        if meta.wrapper.is_some() {
            let _ = std::fs::remove_dir(dir); // só se ficou vazia
        }
    }
}

/// Monta as opções do aria2 e adiciona a tarefa ao motor com o GID dela.
async fn add_to_engine(inner: &Inner, client: &Aria2Client, r: &DownloadRecord, paused: bool) -> Result<(), shift_download_engine::RpcError> {
    let gid = r.engine_ref.clone().unwrap_or_default();
    let settings = inner.settings.read().unwrap_or_else(|e| e.into_inner()).clone();
    let conns = r.connections.clamp(1, 16).to_string();
    let mut o = Map::new();
    o.insert("gid".into(), json!(gid));
    o.insert("dir".into(), json!(r.directory));
    o.insert("pause".into(), json!(if paused { "true" } else { "false" }));
    o.insert("max-download-limit".into(), json!(r.speed_limit.max(0).to_string()));
    let add = |o: Map<String, Value>| async move {
        match r.kind.as_str() {
            "magnet" => client.add_uri(std::slice::from_ref(&r.source_url), o).await,
            "torrent" => {
                let file = inner.data_dir.join("torrents").join(format!("{}.torrent", r.id));
                let data = std::fs::read(&file).map_err(|e| shift_download_engine::RpcError::Decode(format!("cópia do .torrent: {e}")))?;
                use base64::Engine as _;
                client.add_torrent(&base64::engine::general_purpose::STANDARD.encode(data), o).await
            }
            _ => client.add_uri(std::slice::from_ref(&r.effective_url), o).await,
        }
    };
    if r.kind == "http" {
        o.insert("out".into(), json!(r.file_name));
        o.insert("split".into(), json!(conns));
        o.insert("max-connection-per-server".into(), json!(conns));
    } else {
        o.insert("seed-ratio".into(), json!(format!("{:.2}", settings.seed_ratio)));
        if settings.extra_trackers {
            o.insert("bt-tracker".into(), json!(crate::torrent::PUBLIC_TRACKERS.join(",")));
        }
        if settings.seed_ratio <= 0.0 {
            o.insert("seed-time".into(), json!("0"));
        }
        if let Some(meta) = r.torrent_meta.as_deref().and_then(|m| serde_json::from_str::<TorrentMeta>(m).ok())
            && !meta.selected.is_empty()
            && meta.selected.len() < meta.files.len()
        {
            let list: Vec<String> = meta.selected.iter().map(usize::to_string).collect();
            o.insert("select-file".into(), json!(list.join(",")));
        }
    }
    match add(o.clone()).await {
        Ok(_) => Ok(()),
        // GID ainda guardado como "parado" no aria2: esquece e tenta de novo.
        Err(e) if e.to_string().contains("already") || e.to_string().contains("GID") => {
            let _ = client.remove(&gid).await;
            let _ = client.forget(&gid).await;
            add(o).await.map(drop)
        }
        Err(e) => Err(e),
    }
}

impl Inner {
    fn arc(&self) -> Option<Arc<Inner>> {
        self.me.get().and_then(Weak::upgrade)
    }

    fn settings(&self) -> Settings {
        self.settings.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn require(&self, id: &str) -> CoreResult<DownloadRecord> {
        self.db.get(id).map_err(db_err)?.ok_or_else(|| state_err("Download não encontrado."))
    }

    fn refresh_meta(&self) {
        let tags = self.db.all_tags().unwrap_or_default();
        let duplicates = self.db.duplicate_hashes().unwrap_or_default().into_iter().collect();
        *lock(&self.meta) = MetaCache { tags, duplicates };
    }

    /// Visão com os números ao vivo do motor por cima do que está no banco.
    fn view_of(&self, r: &DownloadRecord) -> TaskView {
        let live = lock(&self.live);
        let l = live.get(&r.id);
        let mut v = match l.and_then(|l| l.bytes) {
            Some((done, total)) if r.status.is_engine_bound() => {
                let mut r = r.clone();
                r.completed_bytes = done;
                r.total_bytes = total.or(r.total_bytes);
                TaskView::from_record(&r, l.map(|l| l.numbers).unwrap_or_default())
            }
            _ => TaskView::from_record(r, l.map(|l| l.numbers).unwrap_or_default()),
        };
        drop(live);
        let meta = lock(&self.meta);
        v.tags = meta.tags.get(&r.id).cloned().unwrap_or_default();
        v.duplicate = r.sha256.as_ref().is_some_and(|h| meta.duplicates.contains(h));
        v
    }

    fn view(&self, id: &str) -> CoreResult<TaskView> {
        Ok(self.view_of(&self.require(id)?))
    }

    fn emit_task(&self, id: &str) {
        if let Ok(v) = self.view(id) {
            if let Some(l) = lock(&self.live).get_mut(id) {
                l.last_view = Some(v.clone());
            }
            self.sink.tasks_changed(&[v]);
        }
    }

    fn zero_live(&self, id: &str) {
        if let Some(l) = lock(&self.live).get_mut(id) {
            l.numbers = LiveNumbers::default();
        }
    }

    /// Nome livre na pasta: não existe no disco (nem como `.aria2`) nem está reservado por outra tarefa.
    fn free_name(&self, dir: &Path, wanted: &str) -> CoreResult<String> {
        let reserved: HashSet<String> =
            self.db.names_in_use(&dir.to_string_lossy()).map_err(db_err)?.into_iter().map(|n| n.to_lowercase()).collect();
        Ok(shift_security::resolve_conflict(wanted, |n| {
            reserved.contains(&n.to_lowercase()) || dir.join(n).exists() || dir.join(format!("{n}.aria2")).exists()
        }))
    }

    fn stats(&self) -> Stats {
        let mut s = Stats::default();
        for (status, n) in self.db.count_by_status().unwrap_or_default() {
            let n = n as u32;
            match status {
                TaskStatus::Downloading => s.active += n,
                TaskStatus::Queued | TaskStatus::Probing => s.queued += n,
                TaskStatus::Paused => s.paused += n,
                TaskStatus::Completed => s.completed += n,
                TaskStatus::Failed => s.failed += n,
                _ => {}
            }
        }
        s.speed_history = lock(&self.history).iter().copied().collect();
        s.download_speed = s.speed_history.last().copied().unwrap_or(0);
        s.upload_speed = self.last_upload.load(Ordering::Relaxed);
        s.schedule_blocked = self.schedule_blocked.load(Ordering::Relaxed);
        s
    }

    /// Registra a falha; agenda nova tentativa se for passageira; SmartSpeed aprende com ela.
    fn fail(&self, id: &str, diag: &Diagnostic, auto_retry: bool) {
        let Ok(r) = self.require(id) else { return };
        let _ = self.db.set_error(id, diag.kind.as_str(), &diag.detail);
        if r.status != TaskStatus::Failed {
            let _ = self.db.transition(id, TaskStatus::Failed, Some(&diag.title));
        }
        if r.kind == "http"
            && let Some(limit) = crate::smartspeed::after_failure(diag.kind, r.connections as u32)
        {
            let host = host_of(&r.source_url);
            let mut limits = lock(&self.host_limits);
            let cur = limits.get(&host).copied().unwrap_or(u32::MAX);
            if limit < cur && !host.is_empty() {
                limits.insert(host.clone(), limit);
                let _ = self.db.set_setting("host_limits", &*limits);
                let _ = self.db.log_event(id, "smartspeed", Some(&format!("SmartSpeed: {host} passa a usar no máximo {limit} conexões")));
            }
        }
        let will_retry = auto_retry && diag.retryable && r.retry_count < MAX_AUTO_RETRIES;
        if will_retry {
            let wait = RETRY_BASE_MS << r.retry_count;
            let _ = self.db.set_retry(id, r.retry_count + 1, Some(shift_persistence::now_ms() + wait));
            let _ = self.db.log_event(id, "retry", Some(&format!("nova tentativa em {} s", wait / 1000)));
        } else {
            let _ = self.db.set_retry(id, r.retry_count, None);
        }
        self.zero_live(id);
        if !will_retry && let Ok(v) = self.view(id) {
            self.sink.task_finished(&v);
        }
    }

    fn insights(&self) -> CoreResult<Insights> {
        let records = self.db.list().map_err(db_err)?;
        let limits = lock(&self.host_limits).clone();
        let mut hosts: BTreeMap<String, crate::views::HostStat> = BTreeMap::new();
        let live = lock(&self.live);
        for r in records.iter().filter(|r| r.kind == "http") {
            let h = host_of(&r.source_url);
            if h.is_empty() {
                continue;
            }
            let e = hosts.entry(h.clone()).or_insert_with(|| crate::views::HostStat {
                host: h.clone(),
                downloads: 0,
                bytes: 0,
                avg_speed: 0,
                failures: 0,
                connection_limit: limits.get(&h).copied(),
                active_connections: 0,
            });
            e.downloads += 1;
            if r.status == TaskStatus::Failed {
                e.failures += 1;
            }
            if r.status == TaskStatus::Completed {
                e.bytes += r.total_bytes.unwrap_or(r.completed_bytes);
            }
            if let Some(l) = live.get(&r.id) {
                e.active_connections += l.numbers.connections;
            }
        }
        drop(live);
        // Velocidade média: bytes / tempo dos concluídos.
        for h in hosts.values_mut() {
            let (b, ms): (i64, i64) = records
                .iter()
                .filter(|r| r.status == TaskStatus::Completed && host_of(&r.source_url) == h.host)
                .filter_map(|r| Some((r.total_bytes?, r.finished_at? - r.started_at?)))
                .filter(|(_, ms)| *ms > 0)
                .fold((0, 0), |(a, t), (b, ms)| (a + b, t + ms));
            if ms > 0 {
                h.avg_speed = (b as f64 / (ms as f64 / 1000.0)) as u64;
            }
        }
        let mut hosts: Vec<_> = hosts.into_values().collect();
        hosts.sort_by(|a, b| b.bytes.cmp(&a.bytes).then(b.downloads.cmp(&a.downloads)));
        hosts.truncate(20);
        let settings = self.settings();
        let restarts = match &*lock(&self.engine_state) {
            EngineState::Ready { restarts, .. } => *restarts,
            _ => 0,
        };
        let suggestions = crate::assistant::suggestions(&crate::assistant::Context {
            records: &records,
            free_space: fs4::available_space(&settings.download_dir).ok(),
            engine_restarts: restarts,
            host_limits: &limits,
            schedule_blocked: self.schedule_blocked.load(Ordering::Relaxed),
        });
        Ok(Insights { hosts, suggestions })
    }
}

async fn engine_events(inner: Arc<Inner>, mut rx: mpsc::UnboundedReceiver<EngineEvent>) {
    while let Some(ev) = rx.recv().await {
        let state = match ev {
            EngineEvent::Starting => {
                let cur = lock(&inner.engine_state).clone();
                if matches!(cur, EngineState::Restarting { .. }) { continue } else { EngineState::Starting }
            }
            EngineEvent::Ready { version, restarts } => EngineState::Ready { version, restarts },
            EngineEvent::Crashed { detail, .. } => EngineState::Restarting { detail },
            EngineEvent::GaveUp(detail) => EngineState::Failed { detail },
            EngineEvent::Stopped => EngineState::Stopped,
        };
        *lock(&inner.engine_state) = state.clone();
        inner.sink.engine_changed(&state);
    }
}

async fn reconcile_on_ready(inner: Arc<Inner>, mut client_rx: watch::Receiver<Option<Aria2Client>>, mut stop_rx: watch::Receiver<bool>) {
    loop {
        let client = client_rx.borrow_and_update().clone();
        if let Some(client) = client {
            let _guard = inner.op_lock.lock().await;
            inner.reconcile(&client).await;
        }
        tokio::select! {
            r = client_rx.changed() => if r.is_err() { break },
            _ = stop_rx.changed() => break,
        }
    }
}

async fn poll_loop(inner: Arc<Inner>, mut stop_rx: watch::Receiver<bool>) {
    let mut tick = tokio::time::interval(POLL_EVERY);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_sample = Instant::now();
    let mut last_schedule = Instant::now() - Duration::from_secs(60);
    loop {
        tokio::select! {
            _ = tick.tick() => {}
            _ = stop_rx.changed() => break,
        }
        if let Some(client) = inner.engine.client() {
            let _guard = inner.op_lock.lock().await;
            inner.poll(&client).await;
            inner.run_due_retries().await;
            if last_schedule.elapsed() >= Duration::from_secs(15) {
                last_schedule = Instant::now();
                inner.apply_schedule().await;
            }
        }
        if last_sample.elapsed() >= Duration::from_secs(1) {
            last_sample = Instant::now();
            let speed = inner.last_speed.load(Ordering::Relaxed);
            {
                let mut h = lock(&inner.history);
                if h.len() == HISTORY_LEN {
                    h.pop_front();
                }
                h.push_back(if inner.engine.client().is_some() { speed } else { 0 });
            }
            let stats = inner.stats();
            inner.check_after_all(&stats);
            inner.sink.stats(&stats);
        }
    }
}
