//! Integração com o Windows: o SHIFT como programa para links magnet e arquivos .torrent,
//! e início junto com o Windows. Tudo em HKCU (só o usuário atual, sem administrador).
//!
//! - A cada abertura: registra os "ProgIDs" e as capacidades (o SHIFT aparece em
//!   "Abrir com" e em Configurações → Aplicativos padrão). Não muda o padrão de ninguém.
//! - Só quando o usuário pede ("Tornar padrão"): assume magnet/.torrent onde não há
//!   escolha feita pelo usuário e abre a tela de Aplicativos padrão para ele confirmar
//!   (o Windows 10/11 não deixa programas trocarem a escolha do usuário sozinhos).

use serde::Serialize;

pub const APP_KEY: &str = r"Software\SHIFT";
const CAPABILITIES: &str = r"Software\SHIFT\Capabilities";
const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DefaultsStatus {
    /// O SHIFT abre links magnet ao clicar.
    pub magnet_default: bool,
    /// O SHIFT abre arquivos .torrent com dois cliques.
    pub torrent_default: bool,
    pub autostart: bool,
    /// O instalador perguntou e a pessoa respondeu "sim".
    pub installer_requested: bool,
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::path::Path;
    use winreg::RegKey;
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};

    fn hkcu() -> RegKey {
        RegKey::predef(HKEY_CURRENT_USER)
    }

    fn command(exe: &Path) -> String {
        format!("\"{}\" \"%1\"", exe.display())
    }

    fn set(path: &str, name: &str, value: &str) -> std::io::Result<()> {
        let (k, _) = hkcu().create_subkey(path)?;
        k.set_value(name, &value.to_string())
    }

    fn get(path: &str, name: &str) -> Option<String> {
        hkcu().open_subkey_with_flags(path, KEY_READ).ok()?.get_value::<String, _>(name).ok()
    }

    fn notify_shell() {
        use windows_sys::Win32::UI::Shell::{SHCNE_ASSOCCHANGED, SHCNF_IDLIST, SHChangeNotify};
        unsafe { SHChangeNotify(SHCNE_ASSOCCHANGED as i32, SHCNF_IDLIST, std::ptr::null(), std::ptr::null()) };
    }

    /// ProgIDs e capacidades: não invasivo, roda a cada abertura.
    pub fn register_capabilities(exe: &Path) -> std::io::Result<()> {
        let icon = format!("\"{}\",0", exe.display());
        for (progid, desc) in [("SHIFT.magnet", "Link magnet (SHIFT)"), ("SHIFT.torrent", "Arquivo torrent (SHIFT)")] {
            let base = format!(r"Software\Classes\{progid}");
            set(&base, "", desc)?;
            set(&format!(r"{base}\DefaultIcon"), "", &icon)?;
            set(&format!(r"{base}\shell\open\command"), "", &command(exe))?;
        }
        set(r"Software\Classes\SHIFT.magnet", "URL Protocol", "")?;
        set(r"Software\Classes\.torrent\OpenWithProgids", "SHIFT.torrent", "")?;
        set(CAPABILITIES, "ApplicationName", "SHIFT")?;
        set(CAPABILITIES, "ApplicationDescription", "Gerenciador de downloads: links, torrents, vídeo e áudio.")?;
        set(&format!(r"{CAPABILITIES}\URLAssociations"), "magnet", "SHIFT.magnet")?;
        set(&format!(r"{CAPABILITIES}\FileAssociations"), ".torrent", "SHIFT.torrent")?;
        set(r"Software\RegisteredApplications", "SHIFT", CAPABILITIES)?;
        Ok(())
    }

    /// Assume magnet e .torrent onde o usuário não escolheu outro programa no Windows.
    pub fn claim(exe: &Path) -> std::io::Result<()> {
        register_capabilities(exe)?;
        // magnet: protocolo em HKCU\Software\Classes\magnet (usado quando não há UserChoice).
        set(r"Software\Classes\magnet", "", "URL:Magnet")?;
        set(r"Software\Classes\magnet", "URL Protocol", "")?;
        set(r"Software\Classes\magnet\DefaultIcon", "", &format!("\"{}\",0", exe.display()))?;
        set(r"Software\Classes\magnet\shell\open\command", "", &command(exe))?;
        set(r"Software\Classes\.torrent", "", "SHIFT.torrent")?;
        notify_shell();
        Ok(())
    }

    fn user_choice(path: &str) -> Option<String> {
        get(path, "ProgId")
    }

    pub fn status(exe: &Path) -> DefaultsStatus {
        let ours = command(exe).to_lowercase();
        let magnet_choice = user_choice(r"Software\Microsoft\Windows\Shell\Associations\UrlAssociations\magnet\UserChoice");
        let magnet_default = match magnet_choice {
            Some(p) => p == "SHIFT.magnet",
            None => get(r"Software\Classes\magnet\shell\open\command", "").is_some_and(|c| c.to_lowercase() == ours),
        };
        let torrent_choice = user_choice(r"Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts\.torrent\UserChoice");
        let torrent_default = match torrent_choice {
            Some(p) => p == "SHIFT.torrent",
            None => get(r"Software\Classes\.torrent", "").as_deref() == Some("SHIFT.torrent"),
        };
        let installer_requested = hkcu()
            .open_subkey_with_flags(APP_KEY, KEY_READ)
            .ok()
            .and_then(|k| k.get_value::<u32, _>("MakeDefault").ok())
            == Some(1);
        DefaultsStatus { magnet_default, torrent_default, autostart: get(RUN_KEY, "SHIFT").is_some(), installer_requested }
    }

    pub fn set_autostart(exe: &Path, on: bool) -> std::io::Result<()> {
        if on {
            set(RUN_KEY, "SHIFT", &format!("\"{}\" --minimized", exe.display()))
        } else {
            match hkcu().open_subkey_with_flags(RUN_KEY, winreg::enums::KEY_SET_VALUE) {
                Ok(k) => match k.delete_value("SHIFT") {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                    _ => Ok(()),
                },
                Err(_) => Ok(()),
            }
        }
    }

    pub fn clear_installer_flag() {
        if let Ok(k) = hkcu().open_subkey_with_flags(APP_KEY, winreg::enums::KEY_SET_VALUE) {
            let _ = k.delete_value("MakeDefault");
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    use std::path::Path;
    pub fn register_capabilities(_: &Path) -> std::io::Result<()> {
        Ok(())
    }
    pub fn claim(_: &Path) -> std::io::Result<()> {
        Err(std::io::Error::other("só no Windows"))
    }
    pub fn status(_: &Path) -> DefaultsStatus {
        DefaultsStatus::default()
    }
    pub fn set_autostart(_: &Path, _: bool) -> std::io::Result<()> {
        Err(std::io::Error::other("só no Windows"))
    }
    pub fn clear_installer_flag() {}
}

pub use imp::{claim, clear_installer_flag, register_capabilities, set_autostart, status};

/// Argumentos que o Windows passa ao abrir um magnet ou um .torrent com o SHIFT.
pub fn launch_target(args: &[String]) -> Option<crate::bridge::ExternalAdd> {
    for a in args.iter().skip(1) {
        let a = a.trim().trim_matches('"');
        if a.starts_with("magnet:?") {
            return Some(crate::bridge::ExternalAdd { url: a.to_string(), kind: Some("file".into()), file_name: None, referrer: None });
        }
        if a.to_ascii_lowercase().ends_with(".torrent") && std::path::Path::new(a).is_file() {
            return Some(crate::bridge::ExternalAdd { url: a.to_string(), kind: Some("torrent-file".into()), file_name: None, referrer: None });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconhece_magnet_e_torrent_nos_argumentos() {
        let dir = std::env::temp_dir().join(format!("shift-arg-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let t = dir.join("x.torrent");
        std::fs::write(&t, b"d").unwrap();
        let args = vec!["shift.exe".to_string(), format!("\"{}\"", t.display())];
        assert_eq!(launch_target(&args).unwrap().kind.as_deref(), Some("torrent-file"));
        let m = vec!["shift.exe".into(), "magnet:?xt=urn:btih:abc".into()];
        assert_eq!(launch_target(&m).unwrap().url, "magnet:?xt=urn:btih:abc");
        assert!(launch_target(&["shift.exe".into(), "--minimized".into()]).is_none());
        let _ = std::fs::remove_dir_all(dir);
    }
}
