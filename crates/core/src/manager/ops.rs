//! Operações do usuário sobre tarefas existentes.

use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Map, json};
use shift_persistence::{DownloadRecord, TaskStatus};
use shift_security::sanitize_filename;

use super::{
    CoreResult, Inner, MAX_AUTO_RETRIES, Manager, add_to_engine, db_err, global_options, host_of, lock, remove_partial,
    rpc_diag, state_err,
};
use crate::category::category_of;
use crate::diagnostics::{Diagnostic, DiagnosticKind};
use crate::probe::probe;
use crate::views::{Settings, TaskView};

impl Manager {
    pub async fn pause(&self, id: &str) -> CoreResult<()> {
        let _guard = self.inner.op_lock.lock().await;
        self.inner.pause_one(id, "user").await
    }

    pub async fn resume(&self, id: &str) -> CoreResult<()> {
        let _guard = self.inner.op_lock.lock().await;
        self.inner.resume_one(id, true).await
    }

    pub async fn pause_all(&self) -> CoreResult<()> {
        let _guard = self.inner.op_lock.lock().await;
        for r in self.inner.db.list_engine_bound().map_err(db_err)? {
            if matches!(r.status, TaskStatus::Queued | TaskStatus::Downloading) {
                self.inner.pause_one(&r.id, "user").await?;
            }
        }
        Ok(())
    }

    pub async fn resume_all(&self) -> CoreResult<()> {
        let _guard = self.inner.op_lock.lock().await;
        for r in self.inner.db.list_engine_bound().map_err(db_err)? {
            if r.status == TaskStatus::Paused {
                self.inner.resume_one(&r.id, true).await?;
            }
        }
        Ok(())
    }

    /// Cancela e apaga a parte baixada (o arquivo nunca existia antes: nomes em conflito são renomeados).
    pub async fn cancel(&self, id: &str) -> CoreResult<()> {
        let _guard = self.inner.op_lock.lock().await;
        let r = self.inner.require(id)?;
        match r.status {
            TaskStatus::Completed => return Err(state_err("Este download já terminou. Use \"Remover da lista\".")),
            TaskStatus::Cancelled => return Ok(()),
            _ => {}
        }
        self.inner.release_from_engine(&r).await;
        remove_partial(&r);
        self.inner.db.set_retry(id, r.retry_count, None).map_err(db_err)?;
        self.inner.db.transition(id, TaskStatus::Cancelled, Some("cancelado pelo usuário")).map_err(db_err)?;
        self.inner.emit_task(id);
        Ok(())
    }

    /// Tira do histórico. Concluídos mantêm o arquivo (torrents param de compartilhar);
    /// falhas têm a parte baixada apagada.
    pub async fn remove(&self, id: &str) -> CoreResult<()> {
        let _guard = self.inner.op_lock.lock().await;
        let r = self.inner.require(id)?;
        match r.status {
            TaskStatus::Completed => {
                if r.kind != "http" {
                    // Para de compartilhar; os arquivos baixados ficam.
                    self.inner.release_from_engine(&r).await;
                    super::remove_control_file(&r);
                }
            }
            TaskStatus::Cancelled => {}
            TaskStatus::Failed => {
                self.inner.release_from_engine(&r).await;
                remove_partial(&r);
            }
            _ => return Err(state_err("Pause ou cancele o download antes de removê-lo da lista.")),
        }
        self.inner.db.delete(id).map_err(db_err)?;
        let _ = std::fs::remove_file(self.inner.data_dir.join("torrents").join(format!("{id}.torrent")));
        lock(&self.inner.live).remove(id);
        self.inner.refresh_meta();
        self.inner.sink.tasks_removed(&[id.to_string()]);
        Ok(())
    }

    /// Tenta de novo agora (zera a contagem de tentativas automáticas).
    pub async fn retry(&self, id: &str) -> CoreResult<()> {
        let _guard = self.inner.op_lock.lock().await;
        let r = self.inner.require(id)?;
        if r.status != TaskStatus::Failed {
            return Err(state_err("Só downloads com falha podem ser tentados de novo."));
        }
        self.inner.reload(r, None, false).await
    }

    /// Troca o link (ex.: o anterior expirou). Mantém o progresso se o arquivo for o mesmo.
    pub async fn change_url(&self, id: &str, new_url: &str) -> CoreResult<TaskView> {
        let url = shift_security::parse_download_url(new_url).map_err(|e| Diagnostic::new(DiagnosticKind::InvalidUrl, e.to_string()))?;
        let _guard = self.inner.op_lock.lock().await;
        let r = self.inner.require(id)?;
        if r.kind != "http" {
            return Err(state_err("Só downloads por link (HTTP/HTTPS) podem trocar de link."));
        }
        if !matches!(r.status, TaskStatus::Failed | TaskStatus::Paused | TaskStatus::Queued) {
            return Err(state_err("Pause o download antes de trocar o link."));
        }
        self.inner.reload(r, Some(url), false).await?;
        self.inner.view(id)
    }

    /// Move na fila: "top" ou "bottom".
    pub async fn move_in_queue(&self, id: &str, to: &str) -> CoreResult<()> {
        let _guard = self.inner.op_lock.lock().await;
        let r = self.inner.require(id)?;
        let max = self.inner.db.max_priority().map_err(db_err)?;
        let (prio, pos, how) = match to {
            "top" => (max + 1, 0, "POS_SET"),
            "bottom" => (0, 0, "POS_END"),
            _ => return Err(state_err("posição inválida")),
        };
        self.inner.db.set_priority(id, prio).map_err(db_err)?;
        if let (Some(client), Some(gid)) = (self.inner.engine.client(), &r.engine_ref) {
            // Só vale para quem está esperando; downloads ativos continuam.
            let _ = client.change_position(gid, pos, how).await;
        }
        let _ = self.inner.db.log_event(id, "queue", Some(if to == "top" { "movido para o topo da fila" } else { "movido para o fim da fila" }));
        self.inner.emit_task(id);
        Ok(())
    }

    /// Limite de velocidade só deste download (bytes/s; 0 = sem limite).
    pub async fn set_speed_limit(&self, id: &str, bps: u64) -> CoreResult<()> {
        let _guard = self.inner.op_lock.lock().await;
        let r = self.inner.require(id)?;
        self.inner.db.set_speed_limit(id, bps as i64).map_err(db_err)?;
        if r.status.is_engine_bound()
            && let (Some(client), Some(gid)) = (self.inner.engine.client(), &r.engine_ref)
        {
            let mut o = Map::new();
            o.insert("max-download-limit".into(), json!(bps.to_string()));
            client.change_option(gid, o).await.map_err(rpc_diag)?;
        }
        self.inner.emit_task(id);
        Ok(())
    }

    /// Renomeia um download concluído (no disco e no histórico), sem sobrescrever nada.
    pub async fn rename(&self, id: &str, new_name: &str) -> CoreResult<TaskView> {
        let _guard = self.inner.op_lock.lock().await;
        let r = self.inner.require(id)?;
        if r.status != TaskStatus::Completed {
            return Err(state_err("Só dá para renomear depois que o download terminar."));
        }
        let wanted = sanitize_filename(new_name);
        if wanted == r.file_name {
            return self.inner.view(id);
        }
        let dir = PathBuf::from(&r.directory);
        let name = self.inner.free_name(&dir, &wanted)?;
        std::fs::rename(dir.join(&r.file_name), dir.join(&name)).map_err(|e| Diagnostic::new(DiagnosticKind::Io, e.to_string()))?;
        self.inner.db.set_file_name(id, &name).map_err(db_err)?;
        if r.kind == "http" {
            self.inner.db.set_category(id, category_of(&name)).map_err(db_err)?;
        }
        let _ = self.inner.db.log_event(id, "rename", Some(&format!("renomeado para {name}")));
        self.inner.emit_task(id);
        self.inner.view(id)
    }

    /// Move um download concluído para outra pasta (sem sobrescrever nada lá).
    pub async fn move_to(&self, id: &str, dir: &str) -> CoreResult<TaskView> {
        let target_dir = PathBuf::from(dir);
        if !target_dir.is_absolute() {
            return Err(state_err("Escolha uma pasta de destino completa."));
        }
        let _guard = self.inner.op_lock.lock().await;
        let r = self.inner.require(id)?;
        if r.status != TaskStatus::Completed {
            return Err(state_err("Só dá para mover depois que o download terminar."));
        }
        std::fs::create_dir_all(&target_dir).map_err(|e| Diagnostic::new(DiagnosticKind::Permission, e.to_string()))?;
        let from = PathBuf::from(&r.directory).join(&r.file_name);
        if !from.exists() {
            return Err(state_err("O arquivo não está mais na pasta de origem."));
        }
        if r.kind != "http" {
            self.inner.release_from_engine(&r).await; // para de compartilhar antes de mover
        }
        let name = self.inner.free_name(&target_dir, &r.file_name)?;
        let to = target_dir.join(&name);
        let moved = tokio::task::spawn_blocking(move || move_path(&from, &to)).await.map_err(|e| Diagnostic::new(DiagnosticKind::Io, e.to_string()))?;
        moved.map_err(|e| Diagnostic::new(DiagnosticKind::Io, e.to_string()))?;
        self.inner.db.set_directory(id, &target_dir.to_string_lossy()).map_err(db_err)?;
        if name != r.file_name {
            self.inner.db.set_file_name(id, &name).map_err(db_err)?;
        }
        let _ = self.inner.db.log_event(id, "rename", Some(&format!("movido para {}", target_dir.display())));
        self.inner.emit_task(id);
        self.inner.view(id)
    }

    /// Manda o arquivo de um download concluído para a Lixeira e tira da lista.
    pub async fn delete_file(&self, id: &str) -> CoreResult<()> {
        let r = self.inner.require(id)?;
        if r.status != TaskStatus::Completed {
            return Err(state_err("Para downloads não concluídos, use \"Cancelar\": a parte baixada é apagada."));
        }
        let path = PathBuf::from(&r.directory).join(&r.file_name);
        if r.kind != "http" {
            let _guard = self.inner.op_lock.lock().await;
            self.inner.release_from_engine(&r).await;
        }
        if path.exists() {
            trash::delete(&path).map_err(|e| Diagnostic::new(DiagnosticKind::Io, format!("não consegui mandar para a Lixeira: {e}")))?;
        }
        self.remove(id).await
    }

    /// Arquivos do torrent e quais estão escolhidos.
    pub async fn torrent_files(&self, id: &str) -> CoreResult<crate::TorrentMeta> {
        let r = self.inner.require(id)?;
        let mut meta: crate::TorrentMeta = r
            .torrent_meta
            .as_deref()
            .and_then(|m| serde_json::from_str(m).ok())
            .ok_or_else(|| state_err("Este download não é um torrent."))?;
        if meta.files.is_empty() {
            // Magnet: a lista vem do aria2 depois que os metadados chegam.
            let (Some(client), Some(gid)) = (self.inner.engine.client(), r.engine_ref.as_deref()) else {
                return Err(state_err("A lista de arquivos ainda não chegou dos outros participantes."));
            };
            let s = client.tell_status(gid).await.map_err(rpc_diag)?;
            let base = PathBuf::from(&r.directory);
            meta.files = s
                .files
                .iter()
                .enumerate()
                .filter(|(_, f)| !f.path.starts_with("[METADATA]") && !f.path.is_empty())
                .map(|(i, f)| crate::torrent::TorrentFile {
                    index: i + 1,
                    path: std::path::Path::new(&f.path).strip_prefix(&base).map(|p| p.to_string_lossy().replace('\\', "/")).unwrap_or(f.path.clone()),
                    length: f.length,
                })
                .collect();
            meta.selected = s.files.iter().enumerate().filter(|(_, f)| f.selected).map(|(i, _)| i + 1).collect();
            if meta.files.is_empty() {
                return Err(state_err("A lista de arquivos ainda não chegou dos outros participantes."));
            }
        }
        if meta.selected.is_empty() {
            meta.selected = meta.files.iter().map(|f| f.index).collect();
        }
        Ok(meta)
    }

    /// Muda quais arquivos do torrent baixar (vale para o que ainda não foi baixado).
    pub async fn set_torrent_files(&self, id: &str, selected: Vec<usize>) -> CoreResult<TaskView> {
        let mut meta = self.torrent_files(id).await?;
        let valid: Vec<usize> = selected.into_iter().filter(|i| meta.files.iter().any(|f| f.index == *i)).collect();
        if valid.is_empty() {
            return Err(state_err("Escolha pelo menos um arquivo do torrent."));
        }
        let _guard = self.inner.op_lock.lock().await;
        let r = self.inner.require(id)?;
        meta.selected = valid.clone();
        self.inner.db.set_torrent_meta(id, serde_json::to_string(&meta).ok().as_deref()).map_err(db_err)?;
        let total: u64 = meta.files.iter().filter(|f| valid.contains(&f.index)).map(|f| f.length).sum();
        self.inner.db.update_progress(id, r.completed_bytes.min(total as i64), Some(total as i64)).map_err(db_err)?;
        if r.status.is_engine_bound()
            && let (Some(client), Some(gid)) = (self.inner.engine.client(), &r.engine_ref)
        {
            let list: Vec<String> = valid.iter().map(usize::to_string).collect();
            let mut o = Map::new();
            o.insert("select-file".into(), json!(list.join(",")));
            client.change_option(gid, o).await.map_err(rpc_diag)?;
        }
        let _ = self.inner.db.log_event(id, "queue", Some(&format!("{} arquivo(s) escolhido(s)", valid.len())));
        self.inner.emit_task(id);
        self.inner.view(id)
    }

    pub async fn set_tags(&self, id: &str, tags: Vec<String>) -> CoreResult<TaskView> {
        let mut clean: Vec<String> = Vec::new();
        for t in tags {
            let t: String = t.trim().chars().filter(|c| !c.is_control()).take(32).collect();
            if !t.is_empty() && !clean.iter().any(|c| c.eq_ignore_ascii_case(&t)) {
                clean.push(t);
            }
        }
        clean.truncate(12);
        self.inner.require(id)?;
        self.inner.db.set_tags(id, &clean).map_err(db_err)?;
        self.inner.refresh_meta();
        self.inner.emit_task(id);
        self.inner.view(id)
    }

    pub async fn update_settings(&self, mut s: Settings) -> CoreResult<Settings> {
        let dir = PathBuf::from(&s.download_dir);
        if !dir.is_absolute() {
            return Err(Diagnostic::new(DiagnosticKind::Permission, "a pasta padrão precisa ser um caminho completo"));
        }
        std::fs::create_dir_all(&dir).map_err(|e| Diagnostic::new(DiagnosticKind::Permission, e.to_string()))?;
        if !s.schedule.is_valid() {
            return Err(state_err("Horário do agendamento inválido. Use o formato HH:MM."));
        }
        s.max_concurrent = s.max_concurrent.clamp(1, 20);
        s.connections = s.connections.clamp(1, 16);
        s.seed_ratio = s.seed_ratio.clamp(0.0, 100.0);
        s.max_media_jobs = s.max_media_jobs.clamp(1, 4);
        if !matches!(s.theme.as_str(), "system" | "light" | "dark") {
            s.theme = "system".into();
        }
        if !matches!(s.language.as_str(), "pt-BR" | "en" | "es") {
            s.language = "pt-BR".into();
        }
        if !matches!(s.speed_mode.as_str(), "economy" | "balanced" | "max") {
            s.speed_mode = "balanced".into();
        }
        if !matches!(s.button_style.as_str(), "classic" | "liquid") {
            s.button_style = "liquid".into();
        }
        if !crate::schedule::AFTER_ALL.contains(&s.after_all.as_str()) {
            s.after_all = "nothing".into();
        }
        self.inner.db.set_setting("settings", &s).map_err(db_err)?;
        *self.inner.settings.write().unwrap_or_else(|e| e.into_inner()) = s.clone();
        if let Some(client) = self.inner.engine.client() {
            client.change_global(global_options(&s)).await.map_err(rpc_diag)?;
        }
        // O agendamento pode ter mudado: aplica já.
        let _guard = self.inner.op_lock.lock().await;
        self.inner.apply_schedule().await;
        Ok(s)
    }
}

/// rename quando dá; entre discos diferentes, copia e apaga o original.
fn move_path(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    if std::fs::rename(from, to).is_ok() {
        return Ok(());
    }
    if from.is_dir() {
        copy_dir(from, to)?;
        std::fs::remove_dir_all(from)
    } else {
        std::fs::copy(from, to)?;
        std::fs::remove_file(from)
    }
}

fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)? {
        let e = e?;
        let dest = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &dest)?;
        } else {
            std::fs::copy(e.path(), dest)?;
        }
    }
    Ok(())
}

impl Inner {
    pub(super) async fn pause_one(&self, id: &str, reason: &str) -> CoreResult<()> {
        let r = self.require(id)?;
        if !matches!(r.status, TaskStatus::Queued | TaskStatus::Downloading) {
            return Ok(());
        }
        if let (Some(client), Some(gid)) = (self.engine.client(), &r.engine_ref)
            && let Err(e) = client.pause(gid).await
            && !e.is_not_found()
        {
            return Err(rpc_diag(e));
        }
        // Grava onde parou (o motor só para de verdade depois do forcePause).
        if let (Some(client), Some(gid)) = (self.engine.client(), &r.engine_ref)
            && let Ok(s) = client.tell_status(gid).await
            && (s.completed_length as i64) >= r.completed_bytes
        {
            let total = (s.total_length > 0).then_some(s.total_length as i64);
            let _ = self.db.update_progress(id, s.completed_length as i64, total);
            if let Some(l) = lock(&self.live).get_mut(id) {
                l.bytes = Some((s.completed_length as i64, total.or(r.total_bytes)));
            }
        }
        self.zero_live(id);
        self.db.set_paused_reason(id, Some(reason)).map_err(db_err)?;
        let why = if reason == "schedule" { "pausado pelo agendamento" } else { "pausado pelo usuário" };
        self.db.transition(id, TaskStatus::Paused, Some(why)).map_err(db_err)?;
        self.emit_task(id);
        Ok(())
    }

    /// `by_user`: o usuário pediu (vale mesmo fora da janela do agendamento).
    pub(super) async fn resume_one(&self, id: &str, by_user: bool) -> CoreResult<()> {
        let r = self.require(id)?;
        if r.status != TaskStatus::Paused {
            return Ok(());
        }
        if let (Some(client), Some(gid)) = (self.engine.client(), &r.engine_ref) {
            match client.unpause(gid).await {
                Ok(()) => {}
                // O motor não conhece a tarefa (sessão perdida): recria a partir do checkpoint.
                Err(e) if e.is_not_found() => add_to_engine(self, &client, &r, false).await.map_err(rpc_diag)?,
                Err(e) if e.to_string().contains("cannot be unpaused") => {}
                Err(e) => return Err(rpc_diag(e)),
            }
        }
        let blocked = self.schedule_blocked.load(std::sync::atomic::Ordering::Relaxed);
        self.db.set_paused_reason(id, if by_user && blocked { Some("override") } else { None }).map_err(db_err)?;
        let why = if by_user { "retomado pelo usuário" } else { "retomado pelo agendamento" };
        self.db.transition(id, TaskStatus::Queued, Some(why)).map_err(db_err)?;
        self.emit_task(id);
        Ok(())
    }

    /// Tira a tarefa do motor e espera ele soltar o arquivo.
    pub(super) async fn release_from_engine(&self, r: &DownloadRecord) {
        let Some(client) = self.engine.client() else { return };
        let mut gids: Vec<String> = r.engine_ref.iter().cloned().collect();
        let orig = super::original_gid(&r.id);
        if !gids.contains(&orig) {
            gids.push(orig);
        }
        for gid in &gids {
            let _ = client.remove(gid).await;
            for _ in 0..30 {
                match client.tell_status(gid).await {
                    Ok(s) if s.status != "removed" && s.status != "complete" && s.status != "error" => {
                        tokio::time::sleep(Duration::from_millis(100)).await
                    }
                    _ => break,
                }
            }
            let _ = client.forget(gid).await;
        }
        self.zero_live(&r.id);
    }

    /// Consulta o link de novo (o mesmo ou um novo) e devolve a tarefa ao motor.
    /// Se o arquivo mudou no servidor (tamanho/ETag) ou não dá para retomar, recomeça do zero.
    pub(super) async fn reload(&self, r: DownloadRecord, new_url: Option<url::Url>, auto: bool) -> CoreResult<()> {
        let keep_paused = r.status == TaskStatus::Paused;
        if r.kind != "http" {
            // Torrent: o aria2 confere as partes existentes; basta devolver ao motor.
            self.release_from_engine(&r).await;
            let mut next = r.clone();
            next.engine_ref = Some(super::original_gid(&r.id));
            self.db.set_engine_ref(&r.id, next.engine_ref.as_deref().unwrap_or("")).map_err(db_err)?;
            self.db.set_retry(&r.id, if auto { r.retry_count } else { 0 }, None).map_err(db_err)?;
            if r.status == TaskStatus::Failed {
                self.db.transition(&r.id, TaskStatus::Queued, Some("nova tentativa")).map_err(db_err)?;
            }
            if let Some(client) = self.engine.client()
                && let Err(e) = add_to_engine(self, &client, &next, keep_paused).await
            {
                self.fail(&r.id, &rpc_diag(e), true);
            }
            self.emit_task(&r.id);
            return Ok(());
        }
        let url = match &new_url {
            Some(u) => u.clone(),
            None => url::Url::parse(&r.source_url).map_err(|_| Diagnostic::new(DiagnosticKind::InvalidUrl, "URL salva inválida"))?,
        };
        let fresh = match probe(&url, &self.user_agent).await {
            Ok(p) => p,
            Err(diag) => {
                if auto {
                    self.fail(&r.id, &diag, true);
                } else if r.status == TaskStatus::Failed {
                    let _ = self.db.set_error(&r.id, diag.kind.as_str(), &diag.detail);
                }
                self.emit_task(&r.id);
                return Err(diag);
            }
        };
        let mut next = r.clone();
        let changed = super::sync::differs(&r.etag, &fresh.etag)
            || super::sync::differs(&r.total_bytes, &fresh.total_bytes.map(|b| b as i64))
            || (r.etag.is_none() && fresh.etag.is_none() && new_url.is_none() && super::sync::differs(&r.last_modified, &fresh.last_modified));
        let checksum_failed = r.error_kind.as_deref() == Some("checksum");
        let restart = changed || fresh.resumable == Some(false) || checksum_failed;
        self.release_from_engine(&r).await;
        if restart && (r.completed_bytes > 0 || checksum_failed) {
            let mut wipe = r.clone();
            wipe.status = TaskStatus::Failed; // garante que remove_partial apague
            remove_partial(&wipe);
            next.completed_bytes = 0;
            let why = if checksum_failed {
                "o arquivo anterior não conferia com o SHA-256; baixando de novo"
            } else if changed {
                "o arquivo mudou no servidor; recomeçando do início"
            } else {
                "o servidor não permite retomar; recomeçando do início"
            };
            let _ = self.db.log_event(&r.id, "restart", Some(why));
        }
        if let Some(u) = &new_url {
            self.db.set_source_url(&r.id, u.as_str()).map_err(db_err)?;
            let _ = self.db.log_event(&r.id, "relink", Some(&format!("link trocado; {}", if restart { "recomeçando" } else { "mesmo arquivo, progresso mantido" })));
        }
        next.effective_url = fresh.effective_url.to_string();
        next.total_bytes = fresh.total_bytes.map(|b| b as i64);
        next.resumable = fresh.resumable;
        next.etag = fresh.etag;
        next.last_modified = fresh.last_modified;
        next.content_type = fresh.content_type.or(r.content_type.clone());
        let limit = lock(&self.host_limits).get(&host_of(url.as_str())).copied();
        if let Some(l) = limit {
            next.connections = next.connections.min(l as i64).max(1);
        }
        if fresh.resumable == Some(false) {
            next.connections = 1;
        }
        self.db.update_remote(&next).map_err(db_err)?;
        if let Some(l) = lock(&self.live).get_mut(&r.id) {
            l.bytes = None;
        }
        self.db.set_retry(&r.id, if auto { r.retry_count } else { 0 }, None).map_err(db_err)?;
        if r.status == TaskStatus::Failed {
            let why = if auto { format!("nova tentativa automática ({}/{MAX_AUTO_RETRIES})", r.retry_count) } else { "nova tentativa pelo usuário".into() };
            self.db.transition(&r.id, TaskStatus::Queued, Some(&why)).map_err(db_err)?;
        }
        if let Some(client) = self.engine.client()
            && let Err(e) = add_to_engine(self, &client, &next, keep_paused).await
        {
            self.fail(&r.id, &rpc_diag(e), true);
        }
        self.emit_task(&r.id);
        Ok(())
    }
}
