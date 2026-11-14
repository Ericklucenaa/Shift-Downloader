//! SHA-256 de arquivos, em blocos, fora da thread da interface.

use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Aceita o hash como o usuário cola: com espaços, maiúsculas ou prefixo "sha256:".
pub fn normalize_sha256(input: &str) -> Option<String> {
    let s: String = input.trim().trim_start_matches("sha256:").trim_start_matches("SHA256:").chars().filter(|c| !c.is_whitespace()).collect();
    (s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())).then(|| s.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_de_arquivo_e_normalizacao() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.txt");
        std::fs::write(&p, b"abc").unwrap();
        assert_eq!(sha256_file(&p).unwrap(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(
            normalize_sha256(" SHA256:BA7816BF 8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD ").as_deref(),
            Some("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
        assert_eq!(normalize_sha256("abc"), None);
    }
}
