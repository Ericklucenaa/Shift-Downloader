//! Leitura de arquivos .torrent (BEP 3) e de links magnet (BEP 9), só o necessário
//! para mostrar nome, arquivos e tamanho antes de entregar ao aria2.

use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};

use crate::diagnostics::{Diagnostic, DiagnosticKind};

const MAX_TORRENT_BYTES: usize = 16 * 1024 * 1024;

/// Rastreadores públicos abertos somados a magnets e torrents (opção "rastreadores extras").
/// Medido: magnet sem rastreador → sem metadados em 60 s só com DHT; com esta lista, 6,7 s.
pub const PUBLIC_TRACKERS: &[&str] = &[
    "udp://tracker.opentrackr.org:1337/announce",
    "udp://open.stealth.si:80/announce",
    "udp://tracker.torrent.eu.org:451/announce",
    "udp://exodus.desync.com:6969/announce",
    "udp://explodie.org:6969/announce",
    "udp://open.demonii.com:1337/announce",
    "udp://tracker.tiny-vps.com:6969/announce",
    "https://tracker.gbitt.info:443/announce",
];

#[derive(Debug, Clone, PartialEq, Eq)]
enum B<'a> {
    Int(i64),
    Bytes(&'a [u8]),
    List(Vec<B<'a>>),
    Dict(Vec<(&'a [u8], B<'a>)>),
}

struct Parser<'a> {
    data: &'a [u8],
    pos: usize,
    depth: usize,
    /// Faixa de bytes do dicionário `info` (o info hash é o SHA-1 desses bytes).
    info_span: Option<(usize, usize)>,
}

impl<'a> Parser<'a> {
    fn err(&self, what: &str) -> String {
        format!("{what} na posição {}", self.pos)
    }

    fn peek(&self) -> Result<u8, String> {
        self.data.get(self.pos).copied().ok_or_else(|| self.err("fim inesperado"))
    }

    fn value(&mut self) -> Result<B<'a>, String> {
        self.depth += 1;
        if self.depth > 64 {
            return Err(self.err("aninhamento demais"));
        }
        let v = match self.peek()? {
            b'i' => {
                self.pos += 1;
                let end = self.find(b'e')?;
                let s = std::str::from_utf8(&self.data[self.pos..end]).map_err(|_| self.err("inteiro inválido"))?;
                let n = s.parse::<i64>().map_err(|_| self.err("inteiro inválido"))?;
                self.pos = end + 1;
                B::Int(n)
            }
            b'l' => {
                self.pos += 1;
                let mut items = Vec::new();
                while self.peek()? != b'e' {
                    items.push(self.value()?);
                }
                self.pos += 1;
                B::List(items)
            }
            b'd' => {
                self.pos += 1;
                let mut items = Vec::new();
                while self.peek()? != b'e' {
                    let B::Bytes(k) = self.value()? else { return Err(self.err("chave inválida")) };
                    let start = self.pos;
                    let v = self.value()?;
                    if k == b"info" && self.depth == 1 {
                        self.info_span = Some((start, self.pos));
                    }
                    items.push((k, v));
                }
                self.pos += 1;
                B::Dict(items)
            }
            b'0'..=b'9' => {
                let colon = self.find(b':')?;
                let len: usize = std::str::from_utf8(&self.data[self.pos..colon])
                    .ok()
                    .and_then(|s| s.parse().ok())
                    .ok_or_else(|| self.err("tamanho inválido"))?;
                let start = colon + 1;
                let end = start.checked_add(len).filter(|e| *e <= self.data.len()).ok_or_else(|| self.err("texto cortado"))?;
                self.pos = end;
                B::Bytes(&self.data[start..end])
            }
            _ => return Err(self.err("byte inesperado")),
        };
        self.depth -= 1;
        Ok(v)
    }

    fn find(&self, byte: u8) -> Result<usize, String> {
        self.data[self.pos..].iter().position(|b| *b == byte).map(|i| self.pos + i).ok_or_else(|| self.err("fim inesperado"))
    }
}

fn get<'a, 'b>(d: &'b B<'a>, key: &str) -> Option<&'b B<'a>> {
    match d {
        B::Dict(items) => items.iter().find(|(k, _)| *k == key.as_bytes()).map(|(_, v)| v),
        _ => None,
    }
}

fn text(v: Option<&B<'_>>) -> Option<String> {
    match v {
        Some(B::Bytes(b)) => Some(String::from_utf8_lossy(b).to_string()),
        _ => None,
    }
}

fn int(v: Option<&B<'_>>) -> Option<i64> {
    match v {
        Some(B::Int(n)) => Some(*n),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentFile {
    /// Índice do aria2 (começa em 1).
    pub index: usize,
    pub path: String,
    pub length: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TorrentInfo {
    pub name: String,
    pub info_hash: String,
    pub files: Vec<TorrentFile>,
    pub total_bytes: u64,
    /// Um único arquivo (sem pasta) ou vários dentro da pasta `name`.
    pub single_file: bool,
}

pub fn parse_torrent(data: &[u8]) -> Result<TorrentInfo, Diagnostic> {
    let bad = |d: String| {
        let mut x = Diagnostic::new(DiagnosticKind::Unknown, format!("arquivo .torrent inválido: {d}"));
        x.title = "Este arquivo .torrent é inválido ou está corrompido".into();
        x.action = "Baixe o .torrent de novo na página de origem.".into();
        x
    };
    if data.len() > MAX_TORRENT_BYTES {
        return Err(bad("grande demais".into()));
    }
    let mut p = Parser { data, pos: 0, depth: 0, info_span: None };
    let root = p.value().map_err(bad)?;
    let info = get(&root, "info").ok_or_else(|| bad("sem dicionário info".into()))?;
    let (s, e) = p.info_span.ok_or_else(|| bad("sem dicionário info".into()))?;
    let info_hash = Sha1::digest(&data[s..e]).iter().map(|b| format!("{b:02x}")).collect::<String>();
    let name = text(get(info, "name.utf-8")).or_else(|| text(get(info, "name"))).ok_or_else(|| bad("sem nome".into()))?;
    let mut files = Vec::new();
    let single_file;
    if let Some(B::List(list)) = get(info, "files") {
        single_file = false;
        for (i, f) in list.iter().enumerate() {
            let parts = match get(f, "path.utf-8").or_else(|| get(f, "path")) {
                Some(B::List(ps)) => ps.iter().filter_map(|x| text(Some(x))).collect::<Vec<_>>(),
                _ => return Err(bad("arquivo sem caminho".into())),
            };
            let length = int(get(f, "length")).filter(|n| *n >= 0).ok_or_else(|| bad("arquivo sem tamanho".into()))?;
            files.push(TorrentFile { index: i + 1, path: parts.join("/"), length: length as u64 });
        }
    } else {
        single_file = true;
        let length = int(get(info, "length")).filter(|n| *n >= 0).ok_or_else(|| bad("sem tamanho".into()))?;
        files.push(TorrentFile { index: 1, path: name.clone(), length: length as u64 });
    }
    if files.is_empty() {
        return Err(bad("nenhum arquivo".into()));
    }
    let total_bytes = files.iter().map(|f| f.length).sum();
    Ok(TorrentInfo { name: shift_security::sanitize_filename(&name), info_hash, files, total_bytes, single_file })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Magnet {
    pub info_hash: String,
    pub name: Option<String>,
}

/// `magnet:?xt=urn:btih:<hash>&dn=<nome>` (hash em hex de 40 ou base32 de 32).
pub fn parse_magnet(s: &str) -> Result<Magnet, Diagnostic> {
    let bad = || Diagnostic::new(DiagnosticKind::InvalidUrl, "link magnet inválido: falta xt=urn:btih:");
    let q = s.trim().strip_prefix("magnet:?").ok_or_else(bad)?;
    let mut hash = None;
    let mut name = None;
    for pair in q.split('&') {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        let v = percent_encoding::percent_decode_str(&v.replace('+', " ")).decode_utf8_lossy().to_string();
        match k {
            "xt" => {
                if let Some(h) = v.strip_prefix("urn:btih:")
                    && ((h.len() == 40 && h.bytes().all(|b| b.is_ascii_hexdigit())) || (h.len() == 32 && h.bytes().all(|b| b.is_ascii_alphanumeric())))
                {
                    hash = Some(h.to_ascii_lowercase());
                }
            }
            "dn" if !v.is_empty() => name = Some(shift_security::sanitize_filename(&v)),
            _ => {}
        }
    }
    Ok(Magnet { info_hash: hash.ok_or_else(bad)?, name })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bstr(s: &str) -> String {
        format!("{}:{}", s.len(), s)
    }

    fn sample_multi() -> Vec<u8> {
        let info = format!(
            "d5:filesld6:lengthi100e4:pathl{}{}eed6:lengthi50e4:pathl{}eee4:name{}12:piece lengthi16384e6:pieces0:e",
            bstr("sub"),
            bstr("a.txt"),
            bstr("b.bin"),
            bstr("Pacote")
        );
        format!("d8:announce{}4:info{}e", bstr("http://t/announce"), info).into_bytes()
    }

    #[test]
    fn le_torrent_com_varios_arquivos_e_calcula_info_hash() {
        let data = sample_multi();
        let t = parse_torrent(&data).unwrap();
        assert_eq!(t.name, "Pacote");
        assert!(!t.single_file);
        assert_eq!(t.total_bytes, 150);
        assert_eq!(t.files[0], TorrentFile { index: 1, path: "sub/a.txt".into(), length: 100 });
        assert_eq!(t.files[1].index, 2);
        // info hash = SHA-1 dos bytes exatos do dicionário info
        let s = String::from_utf8(data.clone()).unwrap();
        let start = s.find("4:info").unwrap() + 6;
        let expected: String = Sha1::digest(&data[start..data.len() - 1]).iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(t.info_hash, expected);
    }

    #[test]
    fn le_torrent_de_um_arquivo() {
        let data = format!("d4:infod6:lengthi7e4:name{}12:piece lengthi1e6:pieces0:ee", bstr("../x.iso")).into_bytes();
        let t = parse_torrent(&data).unwrap();
        assert!(t.single_file);
        assert_eq!(t.name, "x.iso", "nome saneado");
        assert_eq!(t.total_bytes, 7);
    }

    #[test]
    fn recusa_lixo_sem_entrar_em_panico() {
        for bad in [&b""[..], b"d", b"i12", b"d4:infoi1ee", b"99999999999:x", b"lllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllll"] {
            assert!(parse_torrent(bad).is_err(), "{:?}", String::from_utf8_lossy(bad));
        }
    }

    #[test]
    fn magnet() {
        let m = parse_magnet("magnet:?xt=urn:btih:C12FE1C06BBA254A9DC9F519B335AA7C1367A88A&dn=Ubuntu+24.04%20Desktop&tr=udp://t").unwrap();
        assert_eq!(m.info_hash, "c12fe1c06bba254a9dc9f519b335aa7c1367a88a");
        assert_eq!(m.name.as_deref(), Some("Ubuntu 24.04 Desktop"));
        assert!(parse_magnet("magnet:?dn=x").is_err());
        assert!(parse_magnet("https://x").is_err());
    }
}
