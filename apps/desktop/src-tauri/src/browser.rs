//! Registro do native messaging host para Chrome e Edge (só para o usuário atual).

use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::json;

pub const HOST_NAME: &str = "app.shift.downloads";
/// ID fixo da extensão (derivado da chave pública em apps/browser-extension/manifest.json).
pub const EXTENSION_ID: &str = "blodjnacfjoicagdklhnmhknnnhbimeb";

const REG_KEYS: [(&str, &str); 2] = [
    ("Chrome", r"Software\Google\Chrome\NativeMessagingHosts\app.shift.downloads"),
    ("Edge", r"Software\Microsoft\Edge\NativeMessagingHosts\app.shift.downloads"),
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserStatus {
    pub registered: Vec<String>,
    pub extension_dir: Option<String>,
    pub extension_id: String,
    pub error: Option<String>,
}

/// Escreve o manifesto do host e as chaves do registro. Idempotente; roda a cada abertura
/// (o caminho do executável pode mudar depois de uma atualização).
pub fn register(data_dir: &Path) -> Result<Vec<String>, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let manifest = data_dir.join("native-host.json");
    let body = json!({
        "name": HOST_NAME,
        "description": "SHIFT Download Manager",
        "path": exe.to_string_lossy(),
        "type": "stdio",
        "allowed_origins": [format!("chrome-extension://{EXTENSION_ID}/")]
    });
    std::fs::write(&manifest, serde_json::to_vec_pretty(&body).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let mut ok = Vec::new();
    #[cfg(windows)]
    {
        use winreg::RegKey;
        use winreg::enums::HKEY_CURRENT_USER;
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        for (name, key) in REG_KEYS {
            if let Ok((k, _)) = hkcu.create_subkey(key)
                && k.set_value("", &manifest.to_string_lossy().to_string()).is_ok()
            {
                ok.push(name.to_string());
            }
        }
    }
    Ok(ok)
}

pub fn extension_dir(resource_dir: Option<PathBuf>) -> Option<PathBuf> {
    let candidates = [
        resource_dir.map(|r| r.join("browser-extension")),
        Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../browser-extension")),
    ];
    candidates.into_iter().flatten().find(|p| p.join("manifest.json").is_file()).map(|p| p.canonicalize().unwrap_or(p))
}
