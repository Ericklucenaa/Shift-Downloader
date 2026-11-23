//! Ícone na área de notificação (perto do relógio): o Shift Downloader continua baixando com a
//! janela fechada. Clique abre a janela; botão direito mostra o menu (inclusive Sair).

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::AppState;

pub const TRAY_ID: &str = "main";
static HINT_SHOWN: AtomicBool = AtomicBool::new(false);

fn labels(lang: &str) -> [&'static str; 5] {
    match lang {
        "en" => ["Open Shift Downloader", "Pause all", "Resume all", "Quit Shift Downloader", "Shift Downloader keeps running here. Right-click the icon to quit."],
        "es" => ["Abrir Shift Downloader", "Pausar todo", "Continuar todo", "Salir de Shift Downloader", "Shift Downloader sigue funcionando aquí. Clic derecho en el icono para salir."],
        _ => ["Abrir o Shift Downloader", "Pausar tudo", "Continuar tudo", "Sair do Shift Downloader", "O Shift Downloader continua rodando aqui. Clique com o botão direito no ícone para sair."],
    }
}

pub fn create(app: &AppHandle, lang: &str) -> tauri::Result<()> {
    let l = labels(lang);
    let open = MenuItem::with_id(app, "open", l[0], true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause_all", l[1], true, None::<&str>)?;
    let resume = MenuItem::with_id(app, "resume_all", l[2], true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", l[3], true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &pause, &resume, &PredefinedMenuItem::separator(app)?, &quit])?;
    let mut b = TrayIconBuilder::with_id(TRAY_ID).tooltip("Shift Downloader").menu(&menu).show_menu_on_left_click(false);
    if let Some(icon) = app.default_window_icon() {
        b = b.icon(icon.clone());
    }
    b.on_menu_event(|app, ev| match ev.id().as_ref() {
        "open" => crate::bridge::focus_main(app),
        "pause_all" | "resume_all" => {
            let app = app.clone();
            let pause = ev.id().as_ref() == "pause_all";
            tauri::async_runtime::spawn(async move {
                if let Some(s) = app.try_state::<AppState>() {
                    let _ = if pause { s.manager.pause_all().await } else { s.manager.resume_all().await };
                }
            });
        }
        "quit" => app.exit(0),
        _ => {}
    })
    .on_tray_icon_event(|tray, ev| {
        if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
            crate::bridge::focus_main(tray.app_handle());
        }
    })
    .build(app)?;
    Ok(())
}

/// Esconde a janela na bandeja; na primeira vez da sessão explica onde o Shift Downloader ficou.
pub fn hide_to_tray(window: &tauri::Window, lang: &str) {
    let _ = window.hide();
    if !HINT_SHOWN.swap(true, Ordering::Relaxed) {
        let _ = window.app_handle().notification().builder().title("Shift Downloader").body(labels(lang)[4]).show();
    }
}

/// Velocidade no texto do ícone (atualizado uma vez por segundo).
pub fn set_speed(app: &AppHandle, down: u64, active: u32) {
    if let Some(t) = app.tray_by_id(TRAY_ID) {
        let text = if active == 0 { "Shift Downloader".to_string() } else { format!("Shift Downloader — {active} ↓ {}", human(down)) };
        let _ = t.set_tooltip(Some(text));
    }
}

fn human(bps: u64) -> String {
    let mut v = bps as f64;
    let mut unit = "B/s";
    for u in ["KB/s", "MB/s", "GB/s"] {
        if v < 1024.0 {
            break;
        }
        v /= 1024.0;
        unit = u;
    }
    if unit == "B/s" { format!("{bps} B/s") } else { format!("{v:.1} {unit}") }
}
