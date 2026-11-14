//! Nomes de arquivo seguros no Windows (e aceitáveis em Linux/macOS).

use std::path::Path;

const RESERVED: &[&str] = &[
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Limite em bytes UTF-8 do nome (NTFS aceita 255 unidades UTF-16; deixamos folga
/// para o sufixo de conflito " (123)" e para a extensão `.aria2` de controle).
const MAX_NAME_BYTES: usize = 200;

/// Transforma um nome vindo da rede (cabeçalho, URL) num nome de arquivo seguro:
/// sem diretórios, sem caracteres proibidos, sem nomes reservados do Windows.
pub fn sanitize_filename(raw: &str) -> String {
    // Só o último componente: impede "../../x" e "C:\Windows\x".
    let last = raw.rsplit(['/', '\\']).next().unwrap_or("");
    let mut name: String = last
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    name = name.trim().trim_end_matches(['.', ' ']).to_string();
    if name.is_empty() || name.chars().all(|c| c == '.' || c == '_') {
        return "download".into();
    }
    let stem_upper = name.split('.').next().unwrap_or("").trim().to_ascii_uppercase();
    if RESERVED.contains(&stem_upper.as_str()) {
        name.insert(0, '_');
    }
    truncate_keeping_extension(&name, MAX_NAME_BYTES)
}

fn truncate_keeping_extension(name: &str, max: usize) -> String {
    if name.len() <= max {
        return name.to_string();
    }
    let (stem, ext) = split_ext(name);
    let ext = if ext.len() > 20 { "" } else { ext };
    let budget = max.saturating_sub(ext.len());
    let mut cut = budget.min(stem.len());
    while !stem.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}{}", stem[..cut].trim_end(), ext)
}

/// ("arquivo.tar", ".gz") para "arquivo.tar.gz"; (".bashrc", "") para dotfiles.
fn split_ext(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 && i < name.len() - 1 => (&name[..i], &name[i..]),
        _ => (name, ""),
    }
}

/// Política de conflito: se `name` já existe (no disco ou reservado por outra
/// tarefa), devolve "nome (1).ext", "nome (2).ext"… Nunca sobrescreve.
pub fn resolve_conflict(name: &str, taken: impl Fn(&str) -> bool) -> String {
    if !taken(name) {
        return name.to_string();
    }
    let (stem, ext) = split_ext(name);
    for n in 1.. {
        let candidate = format!("{stem} ({n}){ext}");
        if !taken(&candidate) {
            return candidate;
        }
    }
    unreachable!()
}

const DANGEROUS: &[&str] = &[
    "exe", "msi", "msix", "msixbundle", "appx", "bat", "cmd", "com", "scr", "pif", "ps1", "psm1",
    "vbs", "vbe", "js", "jse", "wsf", "wsh", "hta", "cpl", "jar", "lnk", "reg", "dll", "sys",
    "application", "gadget", "inf", "scf", "url", "iso", "img", "vhd", "vhdx",
];

/// Arquivos que executam código ao serem abertos: a interface pede confirmação.
pub fn is_dangerous_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| DANGEROUS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_diretorios_e_caracteres_proibidos() {
        assert_eq!(sanitize_filename("../../etc/passwd"), "passwd");
        assert_eq!(sanitize_filename(r"C:\Windows\system32\evil.dll"), "evil.dll");
        assert_eq!(sanitize_filename("relatório: final?.pdf"), "relatório_ final_.pdf");
        assert_eq!(sanitize_filename("nome\u{0}com\ncontrole.txt"), "nome_com_controle.txt");
    }

    #[test]
    fn trata_nomes_vazios_reservados_e_pontos() {
        assert_eq!(sanitize_filename(""), "download");
        assert_eq!(sanitize_filename(".."), "download");
        assert_eq!(sanitize_filename("arquivo.txt. . "), "arquivo.txt");
        assert_eq!(sanitize_filename("CON"), "_CON");
        assert_eq!(sanitize_filename("nul.tar.gz"), "_nul.tar.gz");
        assert_eq!(sanitize_filename("console.log"), "console.log");
    }

    #[test]
    fn preserva_unicode_e_corta_nomes_longos_mantendo_extensao() {
        assert_eq!(sanitize_filename("日本語のファイル.zip"), "日本語のファイル.zip");
        let long = format!("{}.mp4", "ç".repeat(300));
        let out = sanitize_filename(&long);
        assert!(out.len() <= MAX_NAME_BYTES, "{}", out.len());
        assert!(out.ends_with(".mp4"));
    }

    #[test]
    fn conflito_gera_sufixo_sem_sobrescrever() {
        let existing = ["video.mp4", "video (1).mp4"];
        assert_eq!(resolve_conflict("video.mp4", |n| existing.contains(&n)), "video (2).mp4");
        assert_eq!(resolve_conflict("novo.mp4", |n| existing.contains(&n)), "novo.mp4");
        assert_eq!(resolve_conflict("LEIAME", |n| n == "LEIAME"), "LEIAME (1)");
    }

    #[test]
    fn identifica_executaveis() {
        assert!(is_dangerous_extension(Path::new("setup.EXE")));
        assert!(is_dangerous_extension(Path::new("a/b/script.ps1")));
        assert!(!is_dangerous_extension(Path::new("foto.jpg")));
        assert!(!is_dangerous_extension(Path::new("sem_extensao")));
    }
}
