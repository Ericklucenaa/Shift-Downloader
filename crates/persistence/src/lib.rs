//! Persistência local em SQLite: tarefas, eventos e configurações.
//!
//! Operações curtas e síncronas atrás de um `Mutex`. O banco usa WAL, então
//! leituras e escritas pequenas não seguram a interface.

mod model;

use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

pub use model::{DownloadRecord, EventRecord, MediaJob, TaskStatus};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Serialize, de::DeserializeOwned};

const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_inicial.sql"),
    include_str!("../migrations/0002_fases_2_a_4.sql"),
];

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("banco de dados: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("configuração inválida: {0}")]
    Json(#[from] serde_json::Error),
    #[error("tarefa {0} não encontrada")]
    NotFound(String),
    #[error("transição inválida de {from} para {to}")]
    InvalidTransition { from: &'static str, to: &'static str },
    #[error("o banco foi criado por uma versão mais nova do SHIFT (esquema {0})")]
    TooNew(i64),
}

pub type Result<T> = std::result::Result<T, DbError>;

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

pub struct Db {
    conn: Mutex<Connection>,
}

/// Resultado de uma transição aplicada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transition {
    pub from: TaskStatus,
    pub to: TaskStatus,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        Self::init(conn)
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        let db = Self { conn: Mutex::new(conn) };
        db.migrate()?;
        Ok(db)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Connection> {
        // Um pânico com o lock seguro não deixa o banco inconsistente (cada escrita é uma transação).
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn migrate(&self) -> Result<()> {
        let mut conn = self.lock();
        let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        let target = MIGRATIONS.len() as i64;
        if current > target {
            return Err(DbError::TooNew(current));
        }
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
            let tx = conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (i + 1) as i64)?;
            tx.commit()?;
        }
        Ok(())
    }

    pub fn schema_version(&self) -> Result<i64> {
        Ok(self.lock().query_row("PRAGMA user_version", [], |r| r.get(0))?)
    }

    // ---- tarefas ----------------------------------------------------------

    pub fn insert_download(&self, r: &DownloadRecord, detail: Option<&str>) -> Result<()> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO downloads (id, source_url, effective_url, protocol, file_name, directory, total_bytes,
                completed_bytes, status, priority, engine, engine_ref, resumable, etag, last_modified, content_type,
                connections, error_kind, error_detail, retry_count, next_retry_at, created_at, updated_at, started_at, finished_at,
                kind, category, sha256, expected_sha256, speed_limit, paused_reason, torrent_meta, uploaded_bytes)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,
                ?26,?27,?28,?29,?30,?31,?32,?33)",
            params![
                r.id, r.source_url, r.effective_url, r.protocol, r.file_name, r.directory, r.total_bytes,
                r.completed_bytes, r.status.as_str(), r.priority, r.engine, r.engine_ref, r.resumable, r.etag,
                r.last_modified, r.content_type, r.connections, r.error_kind, r.error_detail, r.retry_count,
                r.next_retry_at, r.created_at, r.updated_at, r.started_at, r.finished_at,
                r.kind, r.category, r.sha256, r.expected_sha256, r.speed_limit, r.paused_reason, r.torrent_meta,
                r.uploaded_bytes
            ],
        )?;
        insert_event(&tx, &r.id, "created", None, Some(r.status), detail)?;
        tx.commit()?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<Option<DownloadRecord>> {
        Ok(self.lock().query_row("SELECT * FROM downloads WHERE id = ?1", [id], row_to_record).optional()?)
    }

    pub fn require(&self, id: &str) -> Result<DownloadRecord> {
        self.get(id)?.ok_or_else(|| DbError::NotFound(id.to_string()))
    }

    /// Mais recentes primeiro.
    pub fn list(&self) -> Result<Vec<DownloadRecord>> {
        let conn = self.lock();
        let mut st = conn.prepare("SELECT * FROM downloads ORDER BY created_at DESC")?;
        let rows = st.query_map([], row_to_record)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Tarefas que o motor deveria conhecer (para reconstruir depois de uma queda).
    pub fn list_engine_bound(&self) -> Result<Vec<DownloadRecord>> {
        let conn = self.lock();
        let mut st = conn.prepare(
            "SELECT * FROM downloads WHERE status IN ('queued','downloading','paused')
             ORDER BY priority DESC, created_at ASC",
        )?;
        let rows = st.query_map([], row_to_record)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn count_by_status(&self) -> Result<Vec<(TaskStatus, i64)>> {
        let conn = self.lock();
        let mut st = conn.prepare("SELECT status, COUNT(*) FROM downloads GROUP BY status")?;
        let rows = st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (s, n) = row?;
            if let Some(s) = TaskStatus::parse(&s) {
                out.push((s, n));
            }
        }
        Ok(out)
    }

    /// Falhas com nova tentativa automática vencida.
    pub fn list_due_retries(&self, now: i64) -> Result<Vec<DownloadRecord>> {
        let conn = self.lock();
        let mut st = conn.prepare(
            "SELECT * FROM downloads WHERE status = 'failed' AND next_retry_at IS NOT NULL AND next_retry_at <= ?1",
        )?;
        let rows = st.query_map([now], row_to_record)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Atualiza o que muda quando o link é consultado de novo (nova tentativa).
    pub fn update_remote(&self, r: &DownloadRecord) -> Result<()> {
        self.lock().execute(
            "UPDATE downloads SET effective_url = ?2, total_bytes = ?3, completed_bytes = ?4, resumable = ?5,
                etag = ?6, last_modified = ?7, content_type = ?8, connections = ?9, updated_at = ?10 WHERE id = ?1",
            params![
                r.id, r.effective_url, r.total_bytes, r.completed_bytes, r.resumable, r.etag, r.last_modified,
                r.content_type, r.connections, now_ms()
            ],
        )?;
        Ok(())
    }

    /// Aplica uma transição validada e registra o evento, numa única transação.
    pub fn transition(&self, id: &str, to: TaskStatus, detail: Option<&str>) -> Result<Transition> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        let from: String = tx
            .query_row("SELECT status FROM downloads WHERE id = ?1", [id], |r| r.get(0))
            .optional()?
            .ok_or_else(|| DbError::NotFound(id.to_string()))?;
        let from = TaskStatus::parse(&from).unwrap_or(TaskStatus::Failed);
        if !from.can_transition(to) {
            return Err(DbError::InvalidTransition { from: from.as_str(), to: to.as_str() });
        }
        let now = now_ms();
        tx.execute(
            "UPDATE downloads SET status = ?2, updated_at = ?3,
                started_at = CASE WHEN ?2 = 'downloading' AND started_at IS NULL THEN ?3 ELSE started_at END,
                finished_at = CASE WHEN ?2 IN ('completed','cancelled','failed') THEN ?3 ELSE NULL END,
                error_kind = CASE WHEN ?2 = 'failed' THEN error_kind ELSE NULL END,
                error_detail = CASE WHEN ?2 = 'failed' THEN error_detail ELSE NULL END
             WHERE id = ?1",
            params![id, to.as_str(), now],
        )?;
        insert_event(&tx, id, "status", Some(from), Some(to), detail)?;
        tx.commit()?;
        Ok(Transition { from, to })
    }

    pub fn update_progress(&self, id: &str, completed: i64, total: Option<i64>) -> Result<()> {
        self.lock().execute(
            "UPDATE downloads SET completed_bytes = ?2, total_bytes = COALESCE(?3, total_bytes), updated_at = ?4 WHERE id = ?1",
            params![id, completed, total, now_ms()],
        )?;
        Ok(())
    }

    /// Guarda o diagnóstico da falha (antes da transição para `failed`).
    pub fn set_error(&self, id: &str, kind: &str, detail: &str) -> Result<()> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE downloads SET error_kind = ?2, error_detail = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, kind, detail, now_ms()],
        )?;
        insert_event_kind(&tx, id, "error", Some(&format!("{kind}: {detail}")))?;
        tx.commit()?;
        Ok(())
    }

    pub fn set_retry(&self, id: &str, count: i64, next_at: Option<i64>) -> Result<()> {
        self.lock().execute(
            "UPDATE downloads SET retry_count = ?2, next_retry_at = ?3, updated_at = ?4 WHERE id = ?1",
            params![id, count, next_at, now_ms()],
        )?;
        Ok(())
    }

    pub fn set_engine_ref(&self, id: &str, engine_ref: &str) -> Result<()> {
        self.lock().execute(
            "UPDATE downloads SET engine_ref = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, engine_ref, now_ms()],
        )?;
        Ok(())
    }

    pub fn log_event(&self, id: &str, kind: &str, detail: Option<&str>) -> Result<()> {
        let conn = self.lock();
        insert_event_kind(&conn, id, kind, detail)?;
        Ok(())
    }

    pub fn list_by_status(&self, status: TaskStatus) -> Result<Vec<DownloadRecord>> {
        let conn = self.lock();
        let mut st = conn.prepare("SELECT * FROM downloads WHERE status = ?1 ORDER BY created_at ASC")?;
        let rows = st.query_map([status.as_str()], row_to_record)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    /// Atualiza um campo simples da tarefa. `column` vem só de código (lista fechada).
    fn set_column(&self, id: &str, column: &'static str, value: &dyn rusqlite::ToSql) -> Result<()> {
        assert!(matches!(
            column,
            "sha256" | "expected_sha256" | "priority" | "speed_limit" | "paused_reason" | "file_name" | "category"
                | "torrent_meta" | "uploaded_bytes" | "source_url" | "directory"
        ));
        let sql = format!("UPDATE downloads SET {column} = ?2, updated_at = ?3 WHERE id = ?1");
        self.lock().execute(&sql, params![id, value, now_ms()])?;
        Ok(())
    }

    pub fn set_sha256(&self, id: &str, v: Option<&str>) -> Result<()> {
        self.set_column(id, "sha256", &v)
    }
    pub fn set_expected_sha256(&self, id: &str, v: Option<&str>) -> Result<()> {
        self.set_column(id, "expected_sha256", &v)
    }
    pub fn set_priority(&self, id: &str, v: i64) -> Result<()> {
        self.set_column(id, "priority", &v)
    }
    pub fn set_speed_limit(&self, id: &str, v: i64) -> Result<()> {
        self.set_column(id, "speed_limit", &v)
    }
    pub fn set_paused_reason(&self, id: &str, v: Option<&str>) -> Result<()> {
        self.set_column(id, "paused_reason", &v)
    }
    pub fn set_file_name(&self, id: &str, v: &str) -> Result<()> {
        self.set_column(id, "file_name", &v)
    }
    pub fn set_category(&self, id: &str, v: &str) -> Result<()> {
        self.set_column(id, "category", &v)
    }
    pub fn set_torrent_meta(&self, id: &str, v: Option<&str>) -> Result<()> {
        self.set_column(id, "torrent_meta", &v)
    }
    pub fn set_uploaded(&self, id: &str, v: i64) -> Result<()> {
        self.set_column(id, "uploaded_bytes", &v)
    }
    pub fn set_directory(&self, id: &str, v: &str) -> Result<()> {
        self.set_column(id, "directory", &v)
    }
    pub fn set_source_url(&self, id: &str, v: &str) -> Result<()> {
        self.set_column(id, "source_url", &v)
    }

    pub fn max_priority(&self) -> Result<i64> {
        Ok(self.lock().query_row("SELECT COALESCE(MAX(priority), 0) FROM downloads", [], |r| r.get(0))?)
    }

    // ---- etiquetas ----------------------------------------------------------

    pub fn set_tags(&self, id: &str, tags: &[String]) -> Result<()> {
        let mut conn = self.lock();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM download_tags WHERE download_id = ?1", [id])?;
        for t in tags {
            tx.execute("INSERT OR IGNORE INTO download_tags (download_id, tag) VALUES (?1, ?2)", params![id, t])?;
        }
        tx.commit()?;
        Ok(())
    }

    /// Todas as etiquetas, por tarefa.
    pub fn all_tags(&self) -> Result<std::collections::HashMap<String, Vec<String>>> {
        let conn = self.lock();
        let mut st = conn.prepare("SELECT download_id, tag FROM download_tags ORDER BY tag")?;
        let mut out: std::collections::HashMap<String, Vec<String>> = Default::default();
        for row in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))? {
            let (id, tag) = row?;
            out.entry(id).or_default().push(tag);
        }
        Ok(out)
    }

    /// Hashes que aparecem em mais de um download concluído (arquivos duplicados).
    pub fn duplicate_hashes(&self) -> Result<Vec<String>> {
        let conn = self.lock();
        let mut st = conn.prepare(
            "SELECT sha256 FROM downloads WHERE sha256 IS NOT NULL AND status = 'completed'
             GROUP BY sha256 HAVING COUNT(*) > 1",
        )?;
        let rows = st.query_map([], |r| r.get(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    // ---- mídia --------------------------------------------------------------

    pub fn insert_media(&self, j: &MediaJob) -> Result<()> {
        self.lock().execute(
            "INSERT INTO media_jobs (id, kind, source, title, output_dir, output_path, options, status, progress,
                total_bytes, thumbnail, duration, error_kind, error_detail, created_at, updated_at, finished_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
            params![
                j.id, j.kind, j.source, j.title, j.output_dir, j.output_path, j.options, j.status, j.progress,
                j.total_bytes, j.thumbnail, j.duration, j.error_kind, j.error_detail, j.created_at, j.updated_at,
                j.finished_at
            ],
        )?;
        Ok(())
    }

    pub fn get_media(&self, id: &str) -> Result<Option<MediaJob>> {
        Ok(self.lock().query_row("SELECT * FROM media_jobs WHERE id = ?1", [id], row_to_media).optional()?)
    }

    pub fn list_media(&self) -> Result<Vec<MediaJob>> {
        let conn = self.lock();
        let mut st = conn.prepare("SELECT * FROM media_jobs ORDER BY created_at DESC")?;
        let rows = st.query_map([], row_to_media)?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn set_media_status(&self, id: &str, status: &str, error: Option<(&str, &str)>) -> Result<()> {
        let now = now_ms();
        let finished = matches!(status, "completed" | "failed" | "cancelled").then_some(now);
        self.lock().execute(
            "UPDATE media_jobs SET status = ?2, error_kind = ?3, error_detail = ?4, updated_at = ?5, finished_at = ?6
             WHERE id = ?1",
            params![id, status, error.map(|e| e.0), error.map(|e| e.1), now, finished],
        )?;
        Ok(())
    }

    pub fn set_media_progress(&self, id: &str, progress: f64, total: Option<i64>) -> Result<()> {
        self.lock().execute(
            "UPDATE media_jobs SET progress = ?2, total_bytes = COALESCE(?3, total_bytes), updated_at = ?4 WHERE id = ?1",
            params![id, progress, total, now_ms()],
        )?;
        Ok(())
    }

    pub fn set_media_output(&self, id: &str, path: &str) -> Result<()> {
        self.lock().execute(
            "UPDATE media_jobs SET output_path = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, path, now_ms()],
        )?;
        Ok(())
    }

    pub fn delete_media(&self, id: &str) -> Result<bool> {
        Ok(self.lock().execute("DELETE FROM media_jobs WHERE id = ?1", [id])? > 0)
    }

    pub fn delete(&self, id: &str) -> Result<bool> {
        Ok(self.lock().execute("DELETE FROM downloads WHERE id = ?1", [id])? > 0)
    }

    /// Nomes reservados em `directory` por tarefas que ainda ocupam o arquivo.
    pub fn names_in_use(&self, directory: &str) -> Result<Vec<String>> {
        let conn = self.lock();
        let mut st = conn.prepare(
            "SELECT file_name FROM downloads WHERE directory = ?1 COLLATE NOCASE AND status NOT IN ('cancelled')",
        )?;
        let rows = st.query_map([directory], |r| r.get(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn events(&self, id: &str, limit: i64) -> Result<Vec<EventRecord>> {
        let conn = self.lock();
        let mut st = conn.prepare(
            "SELECT id, download_id, at, kind, from_status, to_status, detail FROM download_events
             WHERE download_id = ?1 ORDER BY at DESC, id DESC LIMIT ?2",
        )?;
        let rows = st.query_map(params![id, limit], |r| {
            Ok(EventRecord {
                id: r.get(0)?,
                download_id: r.get(1)?,
                at: r.get(2)?,
                kind: r.get(3)?,
                from_status: r.get(4)?,
                to_status: r.get(5)?,
                detail: r.get(6)?,
            })
        })?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    // ---- configurações ----------------------------------------------------

    pub fn setting<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let raw: Option<String> =
            self.lock().query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| r.get(0)).optional()?;
        Ok(match raw {
            Some(s) => Some(serde_json::from_str(&s)?),
            None => None,
        })
    }

    pub fn set_setting<T: Serialize>(&self, key: &str, value: &T) -> Result<()> {
        let json = serde_json::to_string(value)?;
        self.lock().execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, json],
        )?;
        Ok(())
    }
}

fn insert_event(
    conn: &Connection,
    id: &str,
    kind: &str,
    from: Option<TaskStatus>,
    to: Option<TaskStatus>,
    detail: Option<&str>,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO download_events (download_id, at, kind, from_status, to_status, detail) VALUES (?1,?2,?3,?4,?5,?6)",
        params![id, now_ms(), kind, from.map(TaskStatus::as_str), to.map(TaskStatus::as_str), detail],
    )?;
    Ok(())
}

fn insert_event_kind(conn: &Connection, id: &str, kind: &str, detail: Option<&str>) -> rusqlite::Result<()> {
    insert_event(conn, id, kind, None, None, detail)
}

fn row_to_record(r: &Row<'_>) -> rusqlite::Result<DownloadRecord> {
    let status: String = r.get("status")?;
    Ok(DownloadRecord {
        id: r.get("id")?,
        source_url: r.get("source_url")?,
        effective_url: r.get("effective_url")?,
        protocol: r.get("protocol")?,
        file_name: r.get("file_name")?,
        directory: r.get("directory")?,
        total_bytes: r.get("total_bytes")?,
        completed_bytes: r.get("completed_bytes")?,
        status: TaskStatus::parse(&status).unwrap_or(TaskStatus::Failed),
        priority: r.get("priority")?,
        engine: r.get("engine")?,
        engine_ref: r.get("engine_ref")?,
        resumable: r.get("resumable")?,
        etag: r.get("etag")?,
        last_modified: r.get("last_modified")?,
        content_type: r.get("content_type")?,
        connections: r.get("connections")?,
        error_kind: r.get("error_kind")?,
        error_detail: r.get("error_detail")?,
        retry_count: r.get("retry_count")?,
        next_retry_at: r.get("next_retry_at")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
        started_at: r.get("started_at")?,
        finished_at: r.get("finished_at")?,
        kind: r.get("kind")?,
        category: r.get("category")?,
        sha256: r.get("sha256")?,
        expected_sha256: r.get("expected_sha256")?,
        speed_limit: r.get("speed_limit")?,
        paused_reason: r.get("paused_reason")?,
        torrent_meta: r.get("torrent_meta")?,
        uploaded_bytes: r.get("uploaded_bytes")?,
    })
}

fn row_to_media(r: &Row<'_>) -> rusqlite::Result<MediaJob> {
    Ok(MediaJob {
        id: r.get("id")?,
        kind: r.get("kind")?,
        source: r.get("source")?,
        title: r.get("title")?,
        output_dir: r.get("output_dir")?,
        output_path: r.get("output_path")?,
        options: r.get("options")?,
        status: r.get("status")?,
        progress: r.get("progress")?,
        total_bytes: r.get("total_bytes")?,
        thumbnail: r.get("thumbnail")?,
        duration: r.get("duration")?,
        error_kind: r.get("error_kind")?,
        error_detail: r.get("error_detail")?,
        created_at: r.get("created_at")?,
        updated_at: r.get("updated_at")?,
        finished_at: r.get("finished_at")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str, name: &str) -> DownloadRecord {
        let now = now_ms();
        DownloadRecord {
            id: id.into(),
            source_url: "https://example.com/a.zip".into(),
            effective_url: "https://cdn.example.com/a.zip".into(),
            protocol: "https".into(),
            file_name: name.into(),
            directory: r"C:\Downloads".into(),
            total_bytes: Some(1000),
            completed_bytes: 0,
            status: TaskStatus::Queued,
            priority: 0,
            engine: "aria2".into(),
            engine_ref: Some("0123456789abcdef".into()),
            resumable: Some(true),
            etag: None,
            last_modified: None,
            content_type: Some("application/zip".into()),
            connections: 8,
            error_kind: None,
            error_detail: None,
            retry_count: 0,
            next_retry_at: None,
            created_at: now,
            updated_at: now,
            started_at: None,
            finished_at: None,
            ..Default::default()
        }
    }

    #[test]
    fn migracao_e_reabertura_no_disco() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("shift.db");
        {
            let db = Db::open(&path).unwrap();
            assert_eq!(db.schema_version().unwrap(), MIGRATIONS.len() as i64);
            db.insert_download(&sample("a", "a.zip"), None).unwrap();
        }
        let db = Db::open(&path).unwrap();
        assert_eq!(db.list().unwrap().len(), 1, "a tarefa sobrevive a reabrir o banco");
    }

    #[test]
    fn ciclo_de_vida_registra_eventos_e_datas() {
        let db = Db::open_in_memory().unwrap();
        db.insert_download(&sample("a", "a.zip"), Some("adicionado")).unwrap();
        db.transition("a", TaskStatus::Downloading, None).unwrap();
        db.update_progress("a", 500, Some(1000)).unwrap();
        db.transition("a", TaskStatus::Paused, Some("pelo usuário")).unwrap();
        db.transition("a", TaskStatus::Downloading, None).unwrap();
        db.transition("a", TaskStatus::Completed, None).unwrap();
        let r = db.require("a").unwrap();
        assert_eq!(r.status, TaskStatus::Completed);
        assert_eq!(r.completed_bytes, 500);
        assert!(r.started_at.is_some() && r.finished_at.is_some());
        let ev = db.events("a", 50).unwrap();
        assert_eq!(ev.len(), 5);
        assert_eq!(ev.last().unwrap().kind, "created");
    }

    #[test]
    fn recusa_transicao_invalida_sem_alterar_nada() {
        let db = Db::open_in_memory().unwrap();
        db.insert_download(&sample("a", "a.zip"), None).unwrap();
        db.transition("a", TaskStatus::Cancelled, None).unwrap();
        let err = db.transition("a", TaskStatus::Downloading, None).unwrap_err();
        assert!(matches!(err, DbError::InvalidTransition { .. }));
        assert_eq!(db.require("a").unwrap().status, TaskStatus::Cancelled);
    }

    #[test]
    fn erro_fica_ate_sair_de_failed() {
        let db = Db::open_in_memory().unwrap();
        db.insert_download(&sample("a", "a.zip"), None).unwrap();
        db.set_error("a", "network", "sem conexão").unwrap();
        db.transition("a", TaskStatus::Failed, None).unwrap();
        assert_eq!(db.require("a").unwrap().error_kind.as_deref(), Some("network"));
        db.transition("a", TaskStatus::Queued, Some("nova tentativa")).unwrap();
        assert_eq!(db.require("a").unwrap().error_kind, None);
    }

    #[test]
    fn nomes_em_uso_ignoram_canceladas_e_maiusculas() {
        let db = Db::open_in_memory().unwrap();
        db.insert_download(&sample("a", "a.zip"), None).unwrap();
        db.insert_download(&sample("b", "b.zip"), None).unwrap();
        db.transition("b", TaskStatus::Cancelled, None).unwrap();
        assert_eq!(db.names_in_use(r"c:\downloads").unwrap(), vec!["a.zip".to_string()]);
    }

    #[test]
    fn configuracoes_em_json() {
        let db = Db::open_in_memory().unwrap();
        assert_eq!(db.setting::<u32>("x").unwrap(), None);
        db.set_setting("x", &5u32).unwrap();
        db.set_setting("x", &7u32).unwrap();
        assert_eq!(db.setting::<u32>("x").unwrap(), Some(7));
    }

    #[test]
    fn campos_novos_etiquetas_duplicados_e_midia() {
        let db = Db::open_in_memory().unwrap();
        let mut a = sample("a", "a.zip");
        a.category = "archive".into();
        db.insert_download(&a, None).unwrap();
        db.insert_download(&sample("b", "b.zip"), None).unwrap();
        for id in ["a", "b"] {
            db.transition(id, TaskStatus::Downloading, None).unwrap();
            db.transition(id, TaskStatus::Completed, None).unwrap();
            db.set_sha256(id, Some("abc")).unwrap();
        }
        assert_eq!(db.duplicate_hashes().unwrap(), vec!["abc".to_string()]);
        db.set_tags("a", &["trabalho".into(), "iso".into()]).unwrap();
        assert_eq!(db.all_tags().unwrap()["a"], vec!["iso".to_string(), "trabalho".to_string()]);
        assert_eq!(db.require("a").unwrap().category, "archive");
        let job = MediaJob { id: "m".into(), kind: "convert".into(), status: "queued".into(), options: "{}".into(), ..Default::default() };
        db.insert_media(&job).unwrap();
        db.set_media_progress("m", 0.5, None).unwrap();
        db.set_media_status("m", "failed", Some(("io", "x"))).unwrap();
        let m = db.get_media("m").unwrap().unwrap();
        assert_eq!((m.status.as_str(), m.progress, m.error_kind.as_deref()), ("failed", 0.5, Some("io")));
        assert!(m.finished_at.is_some());
    }

    #[test]
    fn excluir_remove_eventos_em_cascata() {
        let db = Db::open_in_memory().unwrap();
        db.insert_download(&sample("a", "a.zip"), None).unwrap();
        assert!(db.delete("a").unwrap());
        assert!(db.events("a", 10).unwrap().is_empty());
        assert!(!db.delete("a").unwrap());
    }
}
