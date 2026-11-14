//! Eventos do núcleo → interface (e notificações do sistema). Nomes espelhados em `src/lib/ipc.ts`.

use std::sync::atomic::{AtomicBool, Ordering};

use shift_core::{EngineState, EventSink, MediaView, Stats, TaskStatus, TaskView};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

pub struct TauriSink {
    app: AppHandle,
    pub notify: AtomicBool,
}

impl TauriSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app, notify: AtomicBool::new(true) }
    }

    /// Notifica só quando a janela não está em foco (quem está olhando já vê na tela).
    fn notify(&self, title: &str, body: &str) {
        if !self.notify.load(Ordering::Relaxed) {
            return;
        }
        let focused = self.app.get_webview_window("main").and_then(|w| w.is_focused().ok()).unwrap_or(false);
        if focused {
            return;
        }
        let _ = self.app.notification().builder().title(title).body(body).show();
    }
}

impl EventSink for TauriSink {
    fn tasks_changed(&self, tasks: &[TaskView]) {
        let _ = self.app.emit("shift://tasks", tasks);
    }
    fn tasks_removed(&self, ids: &[String]) {
        let _ = self.app.emit("shift://removed", ids);
    }
    fn engine_changed(&self, state: &EngineState) {
        let _ = self.app.emit("shift://engine", state);
    }
    fn stats(&self, stats: &Stats) {
        let _ = self.app.emit("shift://stats", stats);
        crate::tray::set_speed(&self.app, stats.download_speed, stats.active);
    }
    fn task_finished(&self, task: &TaskView) {
        match task.status {
            TaskStatus::Completed => self.notify("Download concluído", &task.file_name),
            TaskStatus::Failed => {
                let why = task.error.as_ref().map(|e| e.title.clone()).unwrap_or_default();
                self.notify("Download falhou", &format!("{} — {why}", task.file_name));
            }
            _ => {}
        }
    }
    fn after_all(&self, action: &str) {
        let _ = self.app.emit("shift://after-all", action);
    }
    fn media_changed(&self, jobs: &[MediaView]) {
        let _ = self.app.emit("shift://media", jobs);
    }
    fn media_removed(&self, ids: &[String]) {
        let _ = self.app.emit("shift://media-removed", ids);
    }
    fn media_finished(&self, job: &MediaView) {
        let title = match (job.kind.as_str(), job.status.as_str()) {
            ("download", "completed") => "Vídeo/áudio baixado",
            ("convert", "completed") => "Conversão concluída",
            (_, "failed") => "Mídia: falhou",
            _ => return,
        };
        self.notify(title, &job.title);
    }
}
