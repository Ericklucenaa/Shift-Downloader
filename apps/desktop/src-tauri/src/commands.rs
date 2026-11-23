//! Comandos expostos à interface. A interface nunca passa caminhos para abrir:
//! ela passa o ID da tarefa e o caminho vem do banco.

use std::path::Path;
use std::sync::atomic::Ordering;

use serde::Serialize;
use shift_core::{
    AddRequest, BatchResult, ConvertRequest, Diagnostic, DiagnosticKind, EventView, FileProbe, Insights, MediaDownloadRequest,
    MediaInfo, MediaView, ProbeView, Settings, Snapshot, TaskView, ToolsInfo, TorrentInfo, TorrentRequest,
};
use tauri::{Manager, State};
use tauri_plugin_opener::OpenerExt;

use crate::AppState;

type R<T> = Result<T, Diagnostic>;

fn io(e: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::new(DiagnosticKind::Io, e.to_string())
}

#[tauri::command]
pub fn get_snapshot(state: State<'_, AppState>) -> R<Snapshot> {
    state.manager.snapshot()
}

#[tauri::command]
pub async fn probe_url(state: State<'_, AppState>, url: String) -> R<ProbeView> {
    state.manager.probe_url(&url).await
}

#[tauri::command]
pub async fn add_download(state: State<'_, AppState>, request: AddRequest) -> R<TaskView> {
    state.manager.add(request).await
}

#[tauri::command]
pub async fn add_batch(state: State<'_, AppState>, urls: Vec<String>, directory: Option<String>, start_paused: bool) -> R<Vec<BatchResult>> {
    Ok(state.manager.add_batch(urls, directory, start_paused).await)
}

#[tauri::command]
pub fn torrent_preview(state: State<'_, AppState>, path: String) -> R<TorrentInfo> {
    state.manager.torrent_preview(&path)
}

#[tauri::command]
pub async fn add_torrent(state: State<'_, AppState>, request: TorrentRequest) -> R<TaskView> {
    state.manager.add_torrent(request).await
}

#[tauri::command]
pub async fn pause_download(state: State<'_, AppState>, id: String) -> R<()> {
    state.manager.pause(&id).await
}

#[tauri::command]
pub async fn resume_download(state: State<'_, AppState>, id: String) -> R<()> {
    state.manager.resume(&id).await
}

#[tauri::command]
pub async fn cancel_download(state: State<'_, AppState>, id: String) -> R<()> {
    state.manager.cancel(&id).await
}

#[tauri::command]
pub async fn retry_download(state: State<'_, AppState>, id: String) -> R<()> {
    state.manager.retry(&id).await
}

#[tauri::command]
pub async fn remove_download(state: State<'_, AppState>, id: String) -> R<()> {
    state.manager.remove(&id).await
}

#[tauri::command]
pub async fn pause_all(state: State<'_, AppState>) -> R<()> {
    state.manager.pause_all().await
}

#[tauri::command]
pub async fn resume_all(state: State<'_, AppState>) -> R<()> {
    state.manager.resume_all().await
}

#[tauri::command]
pub async fn change_url(state: State<'_, AppState>, id: String, url: String) -> R<TaskView> {
    state.manager.change_url(&id, &url).await
}

#[tauri::command]
pub async fn move_in_queue(state: State<'_, AppState>, id: String, to: String) -> R<()> {
    state.manager.move_in_queue(&id, &to).await
}

#[tauri::command]
pub async fn set_speed_limit(state: State<'_, AppState>, id: String, bps: u64) -> R<()> {
    state.manager.set_speed_limit(&id, bps).await
}

#[tauri::command]
pub async fn rename_download(state: State<'_, AppState>, id: String, name: String) -> R<TaskView> {
    state.manager.rename(&id, &name).await
}

#[tauri::command]
pub async fn set_tags(state: State<'_, AppState>, id: String, tags: Vec<String>) -> R<TaskView> {
    state.manager.set_tags(&id, tags).await
}

#[tauri::command]
pub fn task_events(state: State<'_, AppState>, id: String) -> R<Vec<EventView>> {
    state.manager.events(&id)
}

#[tauri::command]
pub fn insights(state: State<'_, AppState>) -> R<Insights> {
    state.manager.insights()
}

#[tauri::command]
pub fn reset_host_limits(state: State<'_, AppState>) -> R<()> {
    state.manager.reset_host_limits()
}

#[tauri::command]
pub async fn update_settings(state: State<'_, AppState>, settings: Settings) -> R<Settings> {
    let s = state.manager.update_settings(settings).await?;
    state.sink.notify.store(s.notify, Ordering::Relaxed);
    Ok(s)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase", tag = "result")]
pub enum OpenResult {
    Opened,
    /// Executável/script: a interface confirma com o usuário e chama de novo com `confirmed`.
    NeedsConfirmation { file_name: String },
}

fn open_checked(app: &tauri::AppHandle, path: &Path, confirmed: bool) -> R<OpenResult> {
    if path.is_file() && shift_security::is_dangerous_extension(path) && !confirmed {
        let file_name = path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        return Ok(OpenResult::NeedsConfirmation { file_name });
    }
    app.opener().open_path(path.to_string_lossy(), None::<&str>).map_err(io)?;
    Ok(OpenResult::Opened)
}

#[tauri::command]
pub fn open_file(app: tauri::AppHandle, state: State<'_, AppState>, id: String, confirmed: bool) -> R<OpenResult> {
    open_checked(&app, &state.manager.file_path(&id)?, confirmed)
}

#[tauri::command]
pub fn reveal_file(app: tauri::AppHandle, state: State<'_, AppState>, id: String) -> R<()> {
    app.opener().reveal_item_in_dir(state.manager.file_path(&id)?).map_err(io)
}

/// Página de doação (Ko-fi). Endereço fixo: a interface não escolhe a URL.
pub const DONATE_URL: &str = "https://ko-fi.com/ericklucena";

#[tauri::command]
pub fn open_donate(app: tauri::AppHandle) -> R<()> {
    app.opener().open_url(DONATE_URL, None::<&str>).map_err(io)
}

#[tauri::command]
pub fn source_url(state: State<'_, AppState>, id: String) -> R<String> {
    state.manager.source_url(&id)
}

// ---- mídia --------------------------------------------------------------------

#[tauri::command]
pub fn media_list(state: State<'_, AppState>) -> R<Vec<MediaView>> {
    state.manager.media().list()
}

#[tauri::command]
pub async fn media_analyze(state: State<'_, AppState>, url: String) -> R<MediaInfo> {
    state.manager.media().analyze(&url).await
}

#[tauri::command]
pub fn media_add_download(state: State<'_, AppState>, request: MediaDownloadRequest) -> R<MediaView> {
    state.manager.media().add_download(request)
}

#[tauri::command]
pub async fn media_probe_file(state: State<'_, AppState>, path: String) -> R<FileProbe> {
    state.manager.media().probe_file(&path).await
}

#[tauri::command]
pub fn media_add_convert(state: State<'_, AppState>, request: ConvertRequest) -> R<MediaView> {
    state.manager.media().add_convert(request)
}

#[tauri::command]
pub async fn media_pause(state: State<'_, AppState>, id: String) -> R<()> {
    state.manager.media().pause(&id).await
}

#[tauri::command]
pub fn media_resume(state: State<'_, AppState>, id: String) -> R<()> {
    state.manager.media().resume(&id)
}

#[tauri::command]
pub async fn media_cancel(state: State<'_, AppState>, id: String) -> R<()> {
    state.manager.media().cancel(&id).await
}

#[tauri::command]
pub fn media_remove(state: State<'_, AppState>, id: String) -> R<()> {
    state.manager.media().remove(&id)
}

#[tauri::command]
pub fn media_open(app: tauri::AppHandle, state: State<'_, AppState>, id: String, confirmed: bool) -> R<OpenResult> {
    open_checked(&app, &state.manager.media().output_path(&id)?, confirmed)
}

#[tauri::command]
pub fn media_reveal(app: tauri::AppHandle, state: State<'_, AppState>, id: String) -> R<()> {
    // Com o arquivo: abre a pasta já com ele selecionado. Sem ele: abre a pasta de destino.
    let (path, is_file) = state.manager.media().reveal_target(&id)?;
    if is_file {
        app.opener().reveal_item_in_dir(path).map_err(io)
    } else {
        app.opener().open_path(path.to_string_lossy(), None::<&str>).map_err(io)
    }
}

#[tauri::command]
pub async fn media_tools(state: State<'_, AppState>) -> R<ToolsInfo> {
    Ok(state.manager.media().tools_info().await)
}

#[tauri::command]
pub async fn media_update_ytdlp(state: State<'_, AppState>) -> R<String> {
    state.manager.media().update_ytdlp().await
}

// ---- navegador e sistema -------------------------------------------------------

#[tauri::command]
pub fn browser_status(app: tauri::AppHandle, state: State<'_, AppState>) -> R<crate::browser::BrowserStatus> {
    let ext = crate::browser::extension_dir(app.path().resource_dir().ok());
    Ok(crate::browser::BrowserStatus {
        registered: state.browsers.clone(),
        extension_dir: ext.map(|p| p.to_string_lossy().trim_start_matches(r"\\?\").to_string()),
        extension_id: crate::browser::EXTENSION_ID.into(),
        error: state.browser_error.clone(),
    })
}

#[tauri::command]
pub fn open_extension_folder(app: tauri::AppHandle) -> R<()> {
    let dir = crate::browser::extension_dir(app.path().resource_dir().ok()).ok_or_else(|| io("pasta da extensão não encontrada"))?;
    app.opener().open_path(dir.to_string_lossy(), None::<&str>).map_err(io)
}

/// Ação ao terminar tudo. Só executa a ação que está configurada (a interface já mostrou
/// a contagem regressiva com opção de cancelar).
#[tauri::command]
pub async fn power_action(state: State<'_, AppState>, action: String) -> R<()> {
    let configured = state.manager.settings().after_all;
    if action != configured || action == "nothing" {
        return Err(Diagnostic::new(DiagnosticKind::Unknown, "ação não configurada"));
    }
    crate::power::run(&action).map_err(io)
}

// ---- Windows: programa padrão e início automático -------------------------------

#[tauri::command]
pub fn take_launch_target(state: State<'_, AppState>) -> Option<crate::bridge::ExternalAdd> {
    state.launch.lock().ok().and_then(|mut l| l.take())
}

fn exe() -> R<std::path::PathBuf> {
    std::env::current_exe().map_err(io)
}

#[tauri::command]
pub fn defaults_status() -> R<crate::defaults::DefaultsStatus> {
    Ok(crate::defaults::status(&exe()?))
}

/// Assume magnet/.torrent e abre a tela de Aplicativos padrão do Windows já no SHIFT,
/// porque só o usuário pode trocar uma escolha feita antes por ele.
#[tauri::command]
pub fn make_default(app: tauri::AppHandle) -> R<crate::defaults::DefaultsStatus> {
    let exe = exe()?;
    crate::defaults::claim(&exe).map_err(io)?;
    crate::defaults::clear_installer_flag();
    let st = crate::defaults::status(&exe);
    if !(st.magnet_default && st.torrent_default) {
        let _ = app.opener().open_url("ms-settings:defaultapps?registeredAppUser=SHIFT", None::<&str>);
    }
    Ok(st)
}

#[tauri::command]
pub fn open_default_apps(app: tauri::AppHandle) -> R<()> {
    app.opener().open_url("ms-settings:defaultapps?registeredAppUser=SHIFT", None::<&str>).map_err(io)
}

#[tauri::command]
pub fn set_autostart(on: bool) -> R<crate::defaults::DefaultsStatus> {
    let exe = exe()?;
    crate::defaults::set_autostart(&exe, on).map_err(io)?;
    crate::defaults::clear_installer_flag();
    Ok(crate::defaults::status(&exe))
}

// ---- menu de contexto -------------------------------------------------------------

#[tauri::command]
pub async fn move_download(state: State<'_, AppState>, id: String, directory: String) -> R<TaskView> {
    state.manager.move_to(&id, &directory).await
}

#[tauri::command]
pub async fn delete_file(state: State<'_, AppState>, id: String) -> R<()> {
    state.manager.delete_file(&id).await
}

#[tauri::command]
pub async fn torrent_files(state: State<'_, AppState>, id: String) -> R<shift_core::TorrentMeta> {
    state.manager.torrent_files(&id).await
}

#[tauri::command]
pub async fn set_torrent_files(state: State<'_, AppState>, id: String, selected: Vec<usize>) -> R<TaskView> {
    state.manager.set_torrent_files(&id, selected).await
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub clean: bool,
    pub detail: String,
}

/// Verificação sob demanda com o Microsoft Defender do próprio Windows (MpCmdRun).
/// Se encontrar ameaça, o Defender age como de costume (quarentena).
#[tauri::command]
pub async fn scan_file(state: State<'_, AppState>, id: String, media: bool) -> R<ScanResult> {
    let path = if media { state.manager.media().output_path(&id)? } else { state.manager.file_path(&id)? };
    let defender = std::env::var_os("ProgramFiles")
        .map(|p| std::path::PathBuf::from(p).join("Windows Defender").join("MpCmdRun.exe"))
        .filter(|p| p.is_file())
        .ok_or_else(|| Diagnostic::new(DiagnosticKind::ToolMissing, "Microsoft Defender (MpCmdRun.exe) não encontrado"))?;
    let mut cmd = tokio::process::Command::new(defender);
    cmd.args(["-Scan", "-ScanType", "3", "-File"]).arg(&path).stdin(std::process::Stdio::null());
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    let out = tokio::time::timeout(std::time::Duration::from_secs(900), cmd.output())
        .await
        .map_err(|_| io("a verificação demorou demais"))?
        .map_err(io)?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    match out.status.code() {
        Some(0) => Ok(ScanResult { clean: true, detail: lines.last().unwrap_or(&"").to_string() }),
        Some(2) => Ok(ScanResult { clean: false, detail: lines.iter().filter(|l| l.contains("Threat") || l.contains("Ameaça")).copied().collect::<Vec<_>>().join(" | ") }),
        code => Err(io(format!("o Defender não conseguiu verificar (código {code:?}): {}", lines.last().unwrap_or(&"")))),
    }
}
