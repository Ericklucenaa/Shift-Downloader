//! Sincronização com o motor: acompanhamento, conclusão, integridade, reconciliação,
//! novas tentativas, agendamento e ação ao terminar tudo.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Instant;

use shift_download_engine::{Aria2Client, Aria2Status};
use shift_persistence::{DownloadRecord, TaskStatus, now_ms};

use super::{Inner, PERSIST_EVERY, add_to_engine, global_options, lock, rpc_diag};
use crate::diagnostics::{DiagnosticKind, from_aria2};

pub(super) fn differs<T: PartialEq>(a: &Option<T>, b: &Option<T>) -> bool {
    matches!((a, b), (Some(x), Some(y)) if x != y)
}

impl Inner {
    /// Depois de (re)iniciar o motor: remove o que ele tem a mais, recria o que falta.
    pub(super) async fn reconcile(&self, client: &Aria2Client) {
        let settings = self.settings();
        let _ = client.change_global(global_options(&settings)).await;
        let Ok(bound) = self.db.list_engine_bound() else { return };
        let Ok((live, stopped)) = client.list_gids().await else { return };
        let ours: HashSet<String> = bound.iter().filter_map(|r| r.engine_ref.clone()).collect();
        let live_set: HashSet<String> = live.iter().cloned().collect();
        for gid in live.iter().filter(|g| !ours.contains(*g)) {
            // Ex.: download que falhou e voltou pela sessão do aria2. A falha já está registrada.
            let _ = client.remove(gid).await;
            let _ = client.forget(gid).await;
        }
        for gid in stopped.iter().filter(|g| !ours.contains(*g)) {
            let _ = client.forget(gid).await;
        }
        // Torrents concluídos que estavam compartilhando quando o app fechou: o arquivo
        // de controle do aria2 não serve mais para nada.
        for r in self.db.list_by_status(TaskStatus::Completed).unwrap_or_default().iter().filter(|r| r.kind != "http") {
            super::remove_control_file(r);
        }
        for r in bound {
            let Some(gid) = &r.engine_ref else { continue };
            if live_set.contains(gid) || stopped.contains(gid) {
                if r.status == TaskStatus::Paused {
                    let _ = client.pause(gid).await;
                }
                continue;
            }
            self.recreate(client, &r).await;
        }
    }

    async fn recreate(&self, client: &Aria2Client, r: &DownloadRecord) {
        let paused = r.status == TaskStatus::Paused;
        let mut r = r.clone();
        // Magnet que já tinha passado dos metadados: recomeça pelo GID original
        // (o aria2 reaproveita o .torrent salvo e as partes já baixadas).
        let orig = super::original_gid(&r.id);
        if r.kind == "magnet" && r.engine_ref.as_deref() != Some(orig.as_str()) {
            r.engine_ref = Some(orig.clone());
            let _ = self.db.set_engine_ref(&r.id, &orig);
        }
        match add_to_engine(self, client, &r, paused).await {
            Ok(()) => {
                let _ = self.db.log_event(&r.id, "recovered", Some("tarefa recriada no motor a partir do progresso salvo"));
                if r.status == TaskStatus::Downloading {
                    let _ = self.db.transition(&r.id, TaskStatus::Queued, Some("aguardando o motor"));
                }
            }
            Err(e) => self.fail(&r.id, &rpc_diag(e), true),
        }
        self.emit_task(&r.id);
    }

    pub(super) async fn poll(&self, client: &Aria2Client) {
        let Ok(bound) = self.db.list_engine_bound() else { return };
        let tasks: Vec<DownloadRecord> = bound.into_iter().filter(|r| r.engine_ref.is_some()).collect();
        let gids: Vec<String> = tasks.iter().filter_map(|r| r.engine_ref.clone()).collect();
        let Ok(statuses) = client.tell_many(&gids).await else { return };
        let mut changed = Vec::new();
        let (mut down, mut up) = (0u64, 0u64);
        for (r, st) in tasks.iter().zip(statuses) {
            match st {
                None => self.recreate(client, r).await,
                Some(s) => {
                    if s.status == "active" {
                        up += s.upload_speed;
                        self.had_activity.store(true, Ordering::Relaxed);
                        self.after_all_fired.store(false, Ordering::Relaxed);
                    }
                    self.apply_status(client, r, &s).await;
                    if s.status == "active" {
                        down += lock(&self.live).get(&r.id).map_or(0, |l| l.numbers.speed);
                    }
                }
            }
            if let Ok(v) = self.view(&r.id) {
                let mut live = lock(&self.live);
                let l = live.entry(r.id.clone()).or_default();
                if l.last_view.as_ref() != Some(&v) {
                    l.last_view = Some(v.clone());
                    changed.push(v);
                }
            }
        }
        if !changed.is_empty() {
            self.sink.tasks_changed(&changed);
        }
        self.last_speed.store(down, Ordering::Relaxed);
        self.last_upload.store(up, Ordering::Relaxed);
    }

    async fn apply_status(&self, client: &Aria2Client, r: &DownloadRecord, s: &Aria2Status) {
        let gid = r.engine_ref.as_deref().unwrap_or("");
        let torrent = r.kind != "http";

        // Magnet: terminou de buscar os metadados; o download de verdade tem outro GID.
        if torrent && s.status == "complete" && !s.followed_by.is_empty() {
            let next = s.followed_by[0].clone();
            let _ = self.db.set_engine_ref(&r.id, &next);
            let _ = client.forget(gid).await;
            let _ = self.db.log_event(&r.id, "metadata", Some("metadados recebidos; baixando os arquivos"));
            return;
        }
        if torrent
            && let Some(name) = s.bt_name.as_deref().map(shift_security::sanitize_filename)
            && !name.is_empty()
            && name != r.file_name
            && s.followed_by.is_empty()
            && s.total_length > 0
        {
            let _ = self.db.set_file_name(&r.id, &name);
        }

        let total = (s.total_length > 0).then_some(s.total_length as i64).or(r.total_bytes);
        // Download pausado/na fila restaurado da sessão aparece com 0 bytes até o aria2
        // reabrir o arquivo de controle. Isso não é progresso perdido: mantém o salvo.
        let reported = s.completed_length as i64;
        let completed = if reported < r.completed_bytes && (s.status != "active" || reported == 0) { r.completed_bytes } else { reported };
        {
            let mut live = lock(&self.live);
            let l = live.entry(r.id.clone()).or_default();
            let active = s.status == "active";
            l.numbers.speed = if active {
                l.speedo.sample(Instant::now(), reported)
            } else {
                l.speedo.reset();
                0
            };
            l.numbers.connections = if active { s.connections } else { 0 };
            l.numbers.upload_speed = if active { s.upload_speed } else { 0 };
            l.numbers.seeders = if active { s.num_seeders } else { 0 };
            l.bytes = Some((completed, total));
            let due = l.last_persist.is_none_or(|t| t.elapsed() >= PERSIST_EVERY);
            if (completed != r.completed_bytes || total != r.total_bytes) && due {
                let _ = self.db.update_progress(&r.id, completed, total);
                if torrent {
                    let _ = self.db.set_uploaded(&r.id, s.upload_length as i64);
                }
                l.last_persist = Some(Instant::now());
            }
        }
        if torrent && s.seeder && s.status == "active" {
            // Arquivos completos; o aria2 continua compartilhando até a razão configurada.
            self.finish_torrent(r, s, false, client).await;
            return;
        }
        let target = match s.status.as_str() {
            "active" | "waiting" if r.status == TaskStatus::Paused => {
                // Pausado no SHIFT manda: o aria2 volta a pausar.
                let _ = client.pause(gid).await;
                None
            }
            "active" => Some(TaskStatus::Downloading),
            "waiting" => Some(TaskStatus::Queued),
            "paused" => Some(TaskStatus::Paused),
            "complete" => {
                let _ = self.db.update_progress(&r.id, s.completed_length as i64, total);
                if torrent {
                    self.finish_torrent(r, s, true, client).await;
                } else {
                    self.finish_http(client, r, s).await;
                }
                None
            }
            "error" => {
                let _ = self.db.update_progress(&r.id, completed, total);
                let diag = from_aria2(s.error_code.as_deref().unwrap_or("1"), s.error_message.as_deref().unwrap_or(""));
                let _ = client.forget(gid).await;
                self.fail(&r.id, &diag, true);
                None
            }
            "removed" => {
                let _ = client.forget(gid).await;
                Some(TaskStatus::Cancelled)
            }
            _ => None,
        };
        if let Some(t) = target
            && t != r.status
            && r.status.can_transition(t)
        {
            // Mudança de situação grava o progresso exato, sem esperar o próximo ciclo.
            let _ = self.db.update_progress(&r.id, completed, total);
            let _ = self.db.transition(&r.id, t, None);
        }
    }

    async fn finish_torrent(&self, r: &DownloadRecord, s: &Aria2Status, engine_done: bool, client: &Aria2Client) {
        let _ = self.db.update_progress(&r.id, s.completed_length as i64, Some(s.total_length as i64));
        let _ = self.db.set_uploaded(&r.id, s.upload_length as i64);
        let _ = self.db.transition(&r.id, TaskStatus::Verifying, Some("partes conferidas pelo protocolo BitTorrent"));
        let note = if engine_done { "concluído".to_string() } else { format!("concluído; compartilhando até a razão {:.1}", self.settings().seed_ratio) };
        let _ = self.db.transition(&r.id, TaskStatus::Completed, Some(&note));
        if engine_done {
            let _ = client.forget(r.engine_ref.as_deref().unwrap_or("")).await;
        }
        self.zero_live(&r.id);
        if let Ok(v) = self.view(&r.id) {
            self.sink.task_finished(&v);
        }
    }

    /// Confere o tamanho no disco; o SHA-256 é calculado fora do ciclo (pode demorar).
    async fn finish_http(&self, client: &Aria2Client, r: &DownloadRecord, s: &Aria2Status) {
        let gid = r.engine_ref.as_deref().unwrap_or("");
        let _ = self.db.transition(&r.id, TaskStatus::Verifying, Some("conferindo o arquivo"));
        let path = s.path.as_ref().map(PathBuf::from).unwrap_or_else(|| Path::new(&r.directory).join(&r.file_name));
        let size = std::fs::metadata(&path).map(|m| m.len());
        let ok = match (&size, s.total_length) {
            (Ok(sz), 0) => *sz == s.completed_length,
            (Ok(sz), t) => *sz == t,
            (Err(_), _) => false,
        };
        let _ = client.forget(gid).await;
        self.zero_live(&r.id);
        if !ok {
            let detail = match size {
                Ok(sz) => format!("o arquivo tem {sz} bytes, o esperado era {}", s.total_length),
                Err(e) => format!("arquivo não encontrado depois de concluir: {e}"),
            };
            let _ = self.db.set_error(&r.id, DiagnosticKind::Io.as_str(), &detail);
            let _ = self.db.transition(&r.id, TaskStatus::Failed, Some("conferência do arquivo falhou"));
            if let Ok(v) = self.view(&r.id) {
                self.sink.task_finished(&v);
            }
            return;
        }
        let _ = self.db.update_progress(&r.id, s.completed_length as i64, Some(s.completed_length as i64));
        if r.expected_sha256.is_some() {
            // Fica em "conferindo" até o hash bater (ou não).
            self.spawn_hash(r.id.clone(), path, true);
        } else {
            let _ = self.db.transition(&r.id, TaskStatus::Completed, Some("tamanho conferido"));
            if let Ok(v) = self.view(&r.id) {
                self.sink.task_finished(&v);
            }
            self.spawn_hash(r.id.clone(), path, false);
        }
    }

    /// Calcula o SHA-256 em segundo plano (um arquivo por vez, para não disputar o disco).
    /// `verify`: compara com o hash informado e decide entre concluído e falha.
    pub(super) fn spawn_hash(&self, id: String, path: PathBuf, verify: bool) {
        let Some(me) = self.arc() else { return };
        tokio::spawn(async move {
            let Ok(_permit) = me.hash_lock.clone().acquire_owned().await else { return };
            let p = path.clone();
            let result = tokio::task::spawn_blocking(move || crate::hashing::sha256_file(&p)).await;
            let Ok(r) = me.require(&id) else { return };
            let result = match result {
                Ok(Ok(h)) => Ok(h),
                Ok(Err(e)) => Err(e.to_string()),
                Err(e) => Err(e.to_string()),
            };
            match result {
                Ok(actual) => {
                    let _ = me.db.set_sha256(&id, Some(&actual));
                    if verify {
                        match r.expected_sha256.as_deref() {
                            Some(exp) if exp == actual => {
                                let _ = me.db.transition(&id, TaskStatus::Completed, Some("SHA-256 conferido"));
                            }
                            Some(exp) => {
                                let detail = format!("esperado {exp}, obtido {actual}");
                                let _ = me.db.set_error(&id, DiagnosticKind::Checksum.as_str(), &detail);
                                let _ = me.db.transition(&id, TaskStatus::Failed, Some("SHA-256 não confere"));
                            }
                            None => {
                                let _ = me.db.transition(&id, TaskStatus::Completed, Some("tamanho conferido"));
                            }
                        }
                        if let Ok(v) = me.view(&id) {
                            me.sink.task_finished(&v);
                        }
                    }
                }
                Err(msg) => {
                    if verify {
                        let _ = me.db.set_error(&id, DiagnosticKind::Io.as_str(), &format!("não consegui ler o arquivo para conferir: {msg}"));
                        let _ = me.db.transition(&id, TaskStatus::Failed, Some("conferência do arquivo falhou"));
                    }
                }
            }
            me.refresh_meta();
            me.emit_task(&id);
        });
    }

    /// Na partida: termina conferências interrompidas e calcula hashes que faltam.
    pub(super) fn resume_pending_hashes(&self) {
        for r in self.db.list_by_status(TaskStatus::Verifying).unwrap_or_default() {
            let path = Path::new(&r.directory).join(&r.file_name);
            if r.kind == "http" && path.is_file() {
                self.spawn_hash(r.id.clone(), path, true);
            } else {
                let _ = self.db.transition(&r.id, TaskStatus::Completed, Some("conferido"));
            }
        }
        for r in self.db.list_by_status(TaskStatus::Completed).unwrap_or_default() {
            let path = Path::new(&r.directory).join(&r.file_name);
            if r.kind == "http" && r.sha256.is_none() && path.is_file() {
                self.spawn_hash(r.id.clone(), path, false);
            }
        }
    }

    pub(super) async fn run_due_retries(&self) {
        let Ok(due) = self.db.list_due_retries(now_ms()) else { return };
        for r in due {
            let _ = self.reload(r, None, true).await;
        }
    }

    /// Fora da janela: pausa (motivo "agendamento"). Dentro: retoma o que o agendamento pausou.
    pub(super) async fn apply_schedule(&self) {
        let s = self.settings();
        let allowed = s.schedule.allows_now();
        let was_blocked = self.schedule_blocked.swap(!allowed, Ordering::Relaxed);
        let Ok(bound) = self.db.list_engine_bound() else { return };
        for r in bound {
            match (allowed, r.status, r.paused_reason.as_deref()) {
                (false, TaskStatus::Queued | TaskStatus::Downloading, reason) if reason != Some("override") => {
                    let _ = self.pause_one(&r.id, "schedule").await;
                }
                (true, TaskStatus::Paused, Some("schedule")) => {
                    let _ = self.resume_one(&r.id, false).await;
                }
                (true, _, Some("override")) if was_blocked => {
                    let _ = self.db.set_paused_reason(&r.id, None);
                }
                _ => {}
            }
        }
    }

    /// Avisa a interface quando tudo terminou e há uma ação configurada (uma vez por rodada).
    pub(super) fn check_after_all(&self, stats: &crate::views::Stats) {
        let action = self.settings().after_all;
        let media_busy = self.media.get().is_some_and(|m| m.busy());
        if action == "nothing" || stats.active + stats.queued > 0 || media_busy {
            return;
        }
        if self.had_activity.swap(false, Ordering::Relaxed) && !self.after_all_fired.swap(true, Ordering::Relaxed) {
            self.sink.after_all(&action);
        }
    }
}
