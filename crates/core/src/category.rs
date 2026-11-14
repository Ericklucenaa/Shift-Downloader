//! Categorias por tipo de arquivo e pastas automáticas.

pub const CATEGORIES: [&str; 8] = ["video", "audio", "image", "document", "archive", "disk", "program", "other"];

const TABLE: &[(&str, &[&str])] = &[
    ("video", &["mp4", "mkv", "webm", "mov", "avi", "m4v", "ts", "wmv", "flv", "mpg", "mpeg"]),
    ("audio", &["mp3", "flac", "wav", "m4a", "ogg", "opus", "aac", "wma"]),
    ("image", &["png", "jpg", "jpeg", "gif", "webp", "svg", "avif", "heic", "bmp", "tif", "tiff"]),
    ("document", &["pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "csv", "epub", "md", "odt", "ods", "rtf"]),
    ("archive", &["zip", "7z", "rar", "tar", "gz", "bz2", "xz", "zst", "tgz"]),
    ("disk", &["iso", "img", "vhd", "vhdx", "dmg"]),
    ("program", &["exe", "msi", "msix", "appx", "apk", "deb", "rpm", "appimage", "bat", "ps1"]),
];

pub fn category_of(file_name: &str) -> &'static str {
    let ext = file_name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    TABLE.iter().find(|(_, exts)| exts.contains(&ext.as_str())).map(|(c, _)| *c).unwrap_or("other")
}

/// Nome da subpasta quando "organizar por tipo" está ligado.
pub fn folder_name(category: &str, language: &str) -> &'static str {
    let pt = [("video", "Vídeos"), ("audio", "Músicas"), ("image", "Imagens"), ("document", "Documentos"), ("archive", "Compactados"), ("disk", "Imagens de disco"), ("program", "Programas"), ("other", "Outros")];
    let en = [("video", "Videos"), ("audio", "Music"), ("image", "Pictures"), ("document", "Documents"), ("archive", "Archives"), ("disk", "Disk images"), ("program", "Programs"), ("other", "Other")];
    let es = [("video", "Videos"), ("audio", "Música"), ("image", "Imágenes"), ("document", "Documentos"), ("archive", "Comprimidos"), ("disk", "Imágenes de disco"), ("program", "Programas"), ("other", "Otros")];
    let table = match language {
        "en" => &en,
        "es" => &es,
        _ => &pt,
    };
    table.iter().find(|(c, _)| *c == category).map(|(_, n)| *n).unwrap_or("Outros")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categoria_pela_extensao() {
        assert_eq!(category_of("Filme.MKV"), "video");
        assert_eq!(category_of("a.tar.gz"), "archive");
        assert_eq!(category_of("setup.exe"), "program");
        assert_eq!(category_of("LEIAME"), "other");
        assert_eq!(folder_name("audio", "pt-BR"), "Músicas");
        assert_eq!(folder_name("audio", "en"), "Music");
        for c in CATEGORIES {
            assert!(!folder_name(c, "es").is_empty());
        }
    }
}
