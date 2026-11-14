//! Adicionar downloads: link HTTP(S), vários links de uma vez, magnet e .torrent.

use std::path::PathBuf;
use std::time::Instant;

use serde::{Deserialize, Serialize};
use shift_persistence::{DownloadRecord, TaskStatus, now_ms};
use shift_security::{parse_download_url, sanitize_filename};

use super::{
    CoreResult, Inner, Manager, PROBE_TTL, TorrentMeta, add_to_engine, check_disk_space, db_err, host_of, human, lock,
    original_gid, rpc_diag, state_err,
};
use crate::category::{category_of, folder_name};
use crate::diagnostics::{Diagnostic, DiagnosticKind};
use crate::hashing::normalize_sha256;
use crate::probe::probe;
use crate::torrent::{TorrentInfo, parse_magnet, parse_torrent};
use crate::views::{ProbeView, TaskView};

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AddRequest {
    pub url: String,
    pub probe_id: Option<String>,
    pub file_name: Option<String>,
    pub directory: Option<String>,
    #[serde(default)]
    pub start_paused: bool,
    /// Hash informado pela página de download (verificado ao concluir).
    #[serde(default)]
    pub expected_sha256: Option<String>,
    /// bytes/s; 0/None = sem limite próprio.
    #[serde(default)]
    pub speed_limit: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentRequest {
    /// Link magnet ou caminho de um arquivo .torrent.
    pub source: String,
    /// Índices (base 1) dos arquivos escolhidos; None = todos.
    pub selected: Option<Vec<usize>>,
    pub directory: Option<String>,
    #[serde(default)]
    pub start_paused: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchResult {
    pub url: String,
    pub task: Option<TaskView>,
    pub error: Option<Diagnostic>,
}

fn invalid(e: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::new(DiagnosticKind::InvalidUrl, e.to_string())
}

impl Manager {
    /// Consulta o link sem criar a tarefa: a interface mostra nome, tamanho e retomada.
    pub async fn probe_url(&self, input: &str) -> CoreResult<ProbeView> {
        let url = parse_download_url(input).map_err(invalid)?;
        let result = probe(&url, &self.inner.user_agent).await?;
        let settings = self.settings();
        let category = category_of(&result.file_name).to_string();
        let dir = self.inner.target_dir(&settings, None, &category);
        let name = self.inner.free_name(&dir, &result.file_name)?;
        let view = ProbeView {
            probe_id: uuid::Uuid::new_v4().to_string(),
            host: result.effective_url.host_str().unwrap_or("").to_string(),
            display_url: shift_security::redact_url(&result.source_url),
            renamed: name != result.file_name,
            file_name: name,
            directory: dir.to_string_lossy().to_string(),
            total_bytes: result.total_bytes,
            resumable: result.resumable,
            content_type: result.content_type.clone(),
            redirects: result.redirects,
            free_space: fs4::available_space(dir.ancestors().find(|p| p.exists()).unwrap_or(&dir)).ok(),
            category,
        };
        let mut probes = lock(&self.inner.probes);
        probes.retain(|_, (at, _)| at.elapsed() < PROBE_TTL);
        probes.insert(view.probe_id.clone(), (Instant::now(), result));
        Ok(view)
    }

    pub async fn add(&self, req: AddRequest) -> CoreResult<TaskView> {
        let url = parse_download_url(&req.url).map_err(invalid)?;
        let expected = match req.expected_sha256.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
            Some(h) => Some(normalize_sha256(h).ok_or_else(|| state_err("O SHA-256 informado não é válido (64 caracteres hexadecimais)."))?),
            None => None,
        };
        let cached = req.probe_id.as_ref().and_then(|id| {
            let probes = lock(&self.inner.probes);
            probes.get(id).filter(|(at, p)| at.elapsed() < PROBE_TTL && p.source_url == url).map(|(_, p)| p.clone())
        });
        let probed = match cached {
            Some(p) => p,
            None => probe(&url, &self.inner.user_agent).await?,
        };
        let settings = self.settings();
        let wanted = sanitize_filename(req.file_name.as_deref().filter(|n| !n.trim().is_empty()).unwrap_or(&probed.file_name));
        let category = category_of(&wanted).to_string();
        let dir = self.inner.target_dir(&settings, req.directory.as_deref(), &category);
        if !dir.is_absolute() {
            return Err(Diagnostic::new(DiagnosticKind::Permission, "a pasta de destino precisa ser um caminho completo"));
        }
        std::fs::create_dir_all(&dir).map_err(|e| Diagnostic::new(DiagnosticKind::Permission, e.to_string()))?;

        let _guard = self.inner.op_lock.lock().await;
        let name = self.inner.free_name(&dir, &wanted)?;
        check_disk_space(&dir, probed.total_bytes)?;

        let id = uuid::Uuid::new_v4();
        let now = now_ms();
        let resumable = probed.resumable;
        let host = host_of(url.as_str());
        let limits = lock(&self.inner.host_limits).clone();
        let conns = crate::smartspeed::connections_for(&host, &settings.speed_mode, settings.connections, &limits);
        let blocked = !settings.schedule.allows_now();
        let paused = req.start_paused || blocked;
        let record = DownloadRecord {
            id: id.to_string(),
            source_url: url.to_string(),
            effective_url: probed.effective_url.to_string(),
            protocol: probed.effective_url.scheme().to_string(),
            file_name: name.clone(),
            directory: dir.to_string_lossy().to_string(),
            total_bytes: probed.total_bytes.map(|b| b as i64),
            status: if paused { TaskStatus::Paused } else { TaskStatus::Queued },
            engine: "aria2".into(),
            engine_ref: Some(original_gid(&id.to_string())),
            resumable,
            etag: probed.etag.clone(),
            last_modified: probed.last_modified.clone(),
            content_type: probed.content_type.clone(),
            connections: if resumable == Some(false) { 1 } else { conns as i64 },
            created_at: now,
            updated_at: now,
            kind: "http".into(),
            category: category_of(&name).to_string(),
            expected_sha256: expected,
            speed_limit: req.speed_limit.unwrap_or(0) as i64,
            paused_reason: if req.start_paused { Some("user".into()) } else if blocked { Some("schedule".into()) } else { None },
            ..Default::default()
        };
        let note = format!(
            "adicionado; {}; retomada: {}; {} conexões",
            record.total_bytes.map_or("tamanho desconhecido".to_string(), |b| human(b as u64)),
            match resumable { Some(true) => "sim", Some(false) => "não", None => "desconhecida" },
            record.connections
        );
        self.inner.db.insert_download(&record, Some(&note)).map_err(db_err)?;
        if let Some(client) = self.inner.engine.client()
            && let Err(e) = add_to_engine(&self.inner, &client, &record, paused).await
        {
            self.inner.fail(&record.id, &rpc_diag(e), false);
        }
        // Sem motor no momento: a reconciliação adiciona quando ele subir.
        self.inner.emit_task(&record.id);
        self.inner.view(&record.id)
    }

    /// Vários links de uma vez (um por linha). Cada um é consultado e adicionado;
    /// falhas não impedem os demais.
    pub async fn add_batch(&self, urls: Vec<String>, directory: Option<String>, start_paused: bool) -> Vec<BatchResult> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for raw in urls.into_iter().map(|u| u.trim().to_string()).filter(|u| !u.is_empty()).take(500) {
            if !seen.insert(raw.clone()) {
                continue;
            }
            let res = if raw.starts_with("magnet:") {
                self.add_torrent(TorrentRequest { source: raw.clone(), selected: None, directory: directory.clone(), start_paused }).await
            } else {
                self.add(AddRequest { url: raw.clone(), directory: directory.clone(), start_paused, ..Default::default() }).await
            };
            match res {
                Ok(task) => out.push(BatchResult { url: raw, task: Some(task), error: None }),
                Err(e) => out.push(BatchResult { url: raw, task: None, error: Some(e) }),
            }
        }
        out
    }

    /// Lê um .torrent para a interface mostrar os arquivos antes de adicionar.
    pub fn torrent_preview(&self, path: &str) -> CoreResult<TorrentInfo> {
        let p = PathBuf::from(path);
        if p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()) != Some("torrent".into()) {
            return Err(state_err("Escolha um arquivo .torrent."));
        }
        let meta = std::fs::metadata(&p).map_err(|e| Diagnostic::new(DiagnosticKind::Io, e.to_string()))?;
        if meta.len() > 16 * 1024 * 1024 {
            return Err(state_err("Arquivo .torrent grande demais."));
        }
        parse_torrent(&std::fs::read(&p).map_err(|e| Diagnostic::new(DiagnosticKind::Io, e.to_string()))?)
    }

    pub async fn add_torrent(&self, req: TorrentRequest) -> CoreResult<TaskView> {
        let settings = self.settings();
        let base = self.inner.target_dir(&settings, req.directory.as_deref(), "other");
        let base = if req.directory.is_none() && settings.organize_by_type { PathBuf::from(&settings.download_dir).join("Torrents") } else { base };
        std::fs::create_dir_all(&base).map_err(|e| Diagnostic::new(DiagnosticKind::Permission, e.to_string()))?;
        let id = uuid::Uuid::new_v4().to_string();

        let (kind, name, total, meta, source_url, torrent_bytes) = if req.source.trim().starts_with("magnet:") {
            let m = parse_magnet(&req.source)?;
            let name = m.name.clone().unwrap_or_else(|| format!("magnet-{}", &m.info_hash[..8.min(m.info_hash.len())]));
            let meta = TorrentMeta { info_hash: m.info_hash, files: vec![], selected: vec![], wrapper: None };
            ("magnet", name, None, meta, req.source.trim().to_string(), None)
        } else {
            let info = self.torrent_preview(&req.source)?;
            let all: Vec<usize> = info.files.iter().map(|f| f.index).collect();
            let selected: Vec<usize> = match &req.selected {
                Some(s) => s.iter().copied().filter(|i| all.contains(i)).collect(),
                None => all.clone(),
            };
            if selected.is_empty() {
                return Err(state_err("Escolha pelo menos um arquivo do torrent."));
            }
            let total: u64 = info.files.iter().filter(|f| selected.contains(&f.index)).map(|f| f.length).sum();
            let meta = TorrentMeta { info_hash: info.info_hash.clone(), files: info.files.clone(), selected, wrapper: None };
            let data = std::fs::read(&req.source).map_err(|e| Diagnostic::new(DiagnosticKind::Io, e.to_string()))?;
            ("torrent", info.name.clone(), Some(total), meta, req.source.clone(), Some(data))
        };

        let _guard = self.inner.op_lock.lock().await;
        // Nunca escreve por cima: se já existe algo com esse nome, usa uma pasta-invólucro.
        let mut meta = meta;
        let mut dir = base.clone();
        if base.join(&name).exists() || self.inner.db.names_in_use(&base.to_string_lossy()).map_err(db_err)?.iter().any(|n| n.eq_ignore_ascii_case(&name)) {
            let wrapper = self.inner.free_name(&base, &name)?;
            dir = base.join(&wrapper);
            meta.wrapper = Some(wrapper);
        }
        std::fs::create_dir_all(&dir).map_err(|e| Diagnostic::new(DiagnosticKind::Permission, e.to_string()))?;
        check_disk_space(&dir, total)?;
        if let Some(data) = torrent_bytes {
            let store = self.inner.data_dir.join("torrents");
            std::fs::create_dir_all(&store).map_err(|e| Diagnostic::new(DiagnosticKind::Io, e.to_string()))?;
            std::fs::write(store.join(format!("{id}.torrent")), data).map_err(|e| Diagnostic::new(DiagnosticKind::Io, e.to_string()))?;
        }
        let blocked = !settings.schedule.allows_now();
        let paused = req.start_paused || blocked;
        let now = now_ms();
        let name_for_category = name.clone();
        let record = DownloadRecord {
            id: id.clone(),
            source_url,
            effective_url: String::new(),
            protocol: "bittorrent".into(),
            file_name: name,
            directory: dir.to_string_lossy().to_string(),
            total_bytes: total.map(|t| t as i64),
            status: if paused { TaskStatus::Paused } else { TaskStatus::Queued },
            engine: "aria2".into(),
            engine_ref: Some(original_gid(&id)),
            resumable: Some(true),
            connections: 0,
            created_at: now,
            updated_at: now,
            kind: kind.into(),
            // Categoria do maior arquivo escolhido (ou do nome, em magnets/arquivo único).
            category: meta
                .files
                .iter()
                .filter(|f| meta.selected.contains(&f.index))
                .max_by_key(|f| f.length)
                .map_or_else(|| category_of(&name_for_category), |f| category_of(&f.path))
                .to_string(),
            paused_reason: if req.start_paused { Some("user".into()) } else if blocked { Some("schedule".into()) } else { None },
            torrent_meta: serde_json::to_string(&meta).ok(),
            ..Default::default()
        };
        let note = match total {
            Some(t) => format!("torrent adicionado; {} arquivo(s) escolhido(s), {}", meta.selected.len(), human(t)),
            None => "magnet adicionado; buscando os metadados com outros participantes".into(),
        };
        self.inner.db.insert_download(&record, Some(&note)).map_err(db_err)?;
        if let Some(client) = self.inner.engine.client()
            && let Err(e) = add_to_engine(&self.inner, &client, &record, paused).await
        {
            self.inner.fail(&record.id, &rpc_diag(e), false);
        }
        self.inner.emit_task(&record.id);
        self.inner.view(&record.id)
    }
}

impl Inner {
    /// Pasta de destino: a escolhida pelo usuário ou a padrão (com subpasta por tipo, se ligado).
    pub(super) fn target_dir(&self, settings: &crate::views::Settings, chosen: Option<&str>, category: &str) -> PathBuf {
        match chosen.map(str::trim).filter(|d| !d.is_empty()) {
            Some(d) if std::path::Path::new(d) != std::path::Path::new(&settings.download_dir) => PathBuf::from(d),
            _ if settings.organize_by_type => PathBuf::from(&settings.download_dir).join(folder_name(category, &settings.language)),
            _ => PathBuf::from(&settings.download_dir),
        }
    }
}
