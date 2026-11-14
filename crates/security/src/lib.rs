//! Regras de segurança puras (sem E/S de rede): validação de URLs, política de
//! destinos de rede, nomes de arquivo seguros e redação de dados sensíveis em logs.

pub mod filename;
pub mod network;
pub mod url_policy;

pub use filename::{is_dangerous_extension, resolve_conflict, sanitize_filename};
pub use network::{IpClass, RedirectVerdict, check_initial_target, check_redirect, classify_ip};
pub use url_policy::{UrlError, parse_download_url, redact_url};
