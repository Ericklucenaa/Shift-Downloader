//! Atualização do próprio app: procura a versão nova na release do GitHub (latest.json),
//! confere a assinatura e instala. A assinatura usa a chave pública do tauri.conf.json,
//! então um arquivo trocado no caminho é recusado.

use serde::Serialize;
use shift_core::{Diagnostic, DiagnosticKind};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::AppState;

type R<T> = Result<T, Diagnostic>;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub version: String,
    pub current: String,
    pub notes: Option<String>,
    pub date: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    downloaded: u64,
    total: Option<u64>,
}

fn fail(e: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::new(DiagnosticKind::Connection, format!("atualização: {e}"))
}

fn info(u: &Update) -> UpdateInfo {
    UpdateInfo {
        version: u.version.clone(),
        current: u.current_version.clone(),
        notes: u.body.clone().filter(|b| !b.trim().is_empty()),
        date: u.date.map(|d| d.to_string()),
    }
}

/// `None` quando já está na versão mais nova. Guarda a atualização achada para instalar depois.
#[tauri::command]
pub async fn app_update_check(app: AppHandle, state: State<'_, AppState>) -> R<Option<UpdateInfo>> {
    let found = app.updater().map_err(fail)?.check().await.map_err(fail)?;
    let view = found.as_ref().map(info);
    *state.update.lock().expect("update lock") = found;
    Ok(view)
}

/// Baixa, encerra o motor (salvando o progresso dos downloads) e roda o instalador.
/// No Windows o instalador fecha este processo e abre a versão nova ao terminar.
#[tauri::command]
pub async fn app_update_install(app: AppHandle, state: State<'_, AppState>) -> R<()> {
    let pending = state.update.lock().expect("update lock").take();
    let update = match pending {
        Some(u) => u,
        None => app
            .updater()
            .map_err(fail)?
            .check()
            .await
            .map_err(fail)?
            .ok_or_else(|| fail("nenhuma versão nova"))?,
    };
    let mut downloaded = 0u64;
    let emitter = app.clone();
    let bytes = update
        .download(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = emitter.emit("shift://update-progress", Progress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(fail)?;
    // O aria2c.exe fica na pasta do app: precisa estar fechado para o instalador substituí-lo.
    if let Some(s) = app.try_state::<AppState>() {
        s.manager.shutdown().await;
    }
    // Com o motor já parado, reabrir o app é a forma de voltar a um estado consistente se algo falhar.
    if let Err(e) = update.install(bytes) {
        tracing::error!(error = %e, "instalador da atualização não rodou");
    }
    app.restart();
}
