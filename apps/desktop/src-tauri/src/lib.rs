//! Camada desktop: liga o núcleo (shift-core) à janela Tauri.
//! Aqui não há regra de negócio: comandos tipados entram, eventos saem.

mod bridge;
mod browser;
mod commands;
mod defaults;
pub mod native_host;
mod power;
mod sink;
mod tray;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};

use shift_core::{Manager, ManagerConfig};
use tauri::{Manager as _, RunEvent, WindowEvent};

pub struct AppState {
    pub manager: Manager,
    pub sink: Arc<sink::TauriSink>,
    pub browsers: Vec<String>,
    pub browser_error: Option<String>,
    /// Magnet/.torrent com que o Windows abriu o SHIFT (a interface busca quando estiver pronta).
    pub launch: Mutex<Option<bridge::ExternalAdd>>,
}

/// O aria2 vai empacotado como sidecar: no app instalado (e no `tauri dev`) fica ao
/// lado do executável como `aria2c.exe`; rodando via cargo puro, vem de `binaries/`.
fn aria2_path() -> PathBuf {
    let exe_dir = std::env::current_exe().ok().and_then(|p| p.parent().map(PathBuf::from)).unwrap_or_default();
    let bundled = exe_dir.join(if cfg!(windows) { "aria2c.exe" } else { "aria2c" });
    if bundled.is_file() {
        return bundled;
    }
    let triple = if cfg!(windows) { "x86_64-pc-windows-msvc.exe" } else { "x86_64-unknown-linux-gnu" };
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("binaries").join(format!("aria2c-{triple}"))
}

/// yt-dlp e FFmpeg: recursos do instalador; em desenvolvimento, `src-tauri/tools`.
fn tools_dir(resource_dir: Option<PathBuf>) -> Option<PathBuf> {
    [resource_dir.map(|r| r.join("tools")), Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tools"))]
        .into_iter()
        .flatten()
        .find(|p| p.join("yt-dlp.exe").is_file() || p.join("yt-dlp").is_file())
}

pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "warn,shift=info".into()))
        .with_target(false)
        .init();

    let shutting_down = Arc::new(AtomicBool::new(false));
    let app = tauri::Builder::default()
        // Segunda instância (ex.: aberta pela extensão) só traz a janela para a frente.
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            bridge::focus_main(app);
            // Clicar num magnet ou abrir um .torrent com o SHIFT já aberto.
            if let Some(target) = defaults::launch_target(&args) {
                use tauri::Emitter;
                let _ = app.emit("shift://external-add", &target);
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let data = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data)?;
            let downloads = app.path().download_dir().unwrap_or_else(|_| data.join("Downloads"));
            let cfg = ManagerConfig {
                db_path: data.join("shift.db"),
                aria2_binary: aria2_path(),
                session_file: data.join("aria2.session"),
                default_download_dir: downloads,
                user_agent: format!("ShiftDownloader/{}", env!("CARGO_PKG_VERSION")),
                tools_dir: tools_dir(app.path().resource_dir().ok()),
            };
            let sink = Arc::new(sink::TauriSink::new(app.handle().clone()));
            let manager = tauri::async_runtime::block_on(Manager::start(cfg, sink.clone()))
                .map_err(|d| format!("{}: {}", d.title, d.detail))?;
            sink.notify.store(manager.settings().notify, Ordering::Relaxed);
            // Builds de desenvolvimento não mexem no registro (para não "roubar" as associações
            // e a integração do SHIFT instalado), a menos que SHIFT_REGISTER esteja definido.
            let register_system = !cfg!(debug_assertions) || std::env::var_os("SHIFT_REGISTER").is_some();
            let (browsers, browser_error) = if register_system {
                match browser::register(&data) {
                    Ok(b) => (b, None),
                    Err(e) => (vec![], Some(e)),
                }
            } else {
                (vec![], Some("desenvolvimento: registro desligado (defina SHIFT_REGISTER=1)".into()))
            };
            if register_system && let Ok(exe) = std::env::current_exe() {
                // Aparece em "Abrir com" e em Aplicativos padrão (sem trocar o padrão de ninguém).
                if let Err(e) = defaults::register_capabilities(&exe) {
                    tracing::warn!(error = %e, "não consegui registrar magnet/.torrent");
                }
                // Instalações até a 0.4.1 ficavam em outra pasta: corrige caminhos que ainda apontem para lá.
                if let Err(e) = defaults::refresh_paths(&exe) {
                    tracing::warn!(error = %e, "não consegui atualizar caminhos antigos");
                }
            }
            let args: Vec<String> = std::env::args().collect();
            let launch = Mutex::new(defaults::launch_target(&args));
            // Iniciado com o Windows: fica só na bandeja, sem abrir a janela.
            if args.iter().any(|a| a == "--minimized")
                && let Some(w) = app.get_webview_window("main")
            {
                let _ = w.hide();
            }
            if let Err(e) = tray::create(app.handle(), &manager.settings().language) {
                tracing::warn!(error = %e, "ícone da bandeja indisponível");
            }
            let handle = app.handle().clone();
            let data2 = data.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = bridge::start(handle, data2).await {
                    tracing::warn!(error = %e, "ponte com o navegador indisponível");
                }
            });
            app.manage(AppState { manager, sink, browsers, browser_error, launch });
            Ok(())
        })
        .on_window_event(|window, event| {
            // Fechar a janela: por padrão o SHIFT fica na bandeja e continua baixando.
            if let WindowEvent::CloseRequested { api, .. } = event
                && let Some(state) = window.app_handle().try_state::<AppState>()
            {
                let s = state.manager.settings();
                if s.close_to_tray && window.app_handle().tray_by_id(tray::TRAY_ID).is_some() {
                    api.prevent_close();
                    tray::hide_to_tray(window, &s.language);
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::probe_url,
            commands::add_download,
            commands::add_batch,
            commands::torrent_preview,
            commands::add_torrent,
            commands::pause_download,
            commands::resume_download,
            commands::cancel_download,
            commands::retry_download,
            commands::remove_download,
            commands::pause_all,
            commands::resume_all,
            commands::change_url,
            commands::move_in_queue,
            commands::set_speed_limit,
            commands::rename_download,
            commands::set_tags,
            commands::task_events,
            commands::insights,
            commands::reset_host_limits,
            commands::update_settings,
            commands::open_file,
            commands::reveal_file,
            commands::open_donate,
            commands::source_url,
            commands::media_list,
            commands::media_analyze,
            commands::media_add_download,
            commands::media_probe_file,
            commands::media_add_convert,
            commands::media_pause,
            commands::media_resume,
            commands::media_cancel,
            commands::media_remove,
            commands::media_open,
            commands::media_reveal,
            commands::media_tools,
            commands::media_update_ytdlp,
            commands::browser_status,
            commands::open_extension_folder,
            commands::power_action,
            commands::take_launch_target,
            commands::defaults_status,
            commands::make_default,
            commands::open_default_apps,
            commands::set_autostart,
            commands::move_download,
            commands::delete_file,
            commands::torrent_files,
            commands::set_torrent_files,
            commands::scan_file,
        ])
        .build(tauri::generate_context!())
        .expect("falha ao iniciar o SHIFT");

    app.run(move |handle, event| {
        // Fechar a janela: salva o progresso e encerra o aria2 e os processos de mídia antes de sair.
        if let RunEvent::ExitRequested { api, .. } = event
            && !shutting_down.swap(true, Ordering::SeqCst)
        {
            api.prevent_exit();
            let handle = handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Some(state) = handle.try_state::<AppState>() {
                    state.manager.shutdown().await;
                }
                if let Ok(data) = handle.path().app_data_dir() {
                    let _ = std::fs::remove_file(bridge::info_path(&data));
                }
                handle.exit(0);
            });
        }
    });
}
