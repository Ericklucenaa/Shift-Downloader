//! SmartSpeed: escolhe quantas conexões usar de forma conservadora.
//!
//! - Modo: econômico (2), equilibrado (configuração do usuário) ou máximo (16).
//! - Por servidor: se um host responde "ocupado", derruba conexões ou fica lento
//!   demais com várias conexões, o limite dele cai pela metade (mínimo 1) e fica
//!   salvo. Nunca sobe sozinho: o usuário pode zerar os limites nas configurações.

use std::collections::BTreeMap;

use crate::diagnostics::DiagnosticKind;

pub fn mode_connections(mode: &str, configured: u32) -> u32 {
    match mode {
        "economy" => 2.min(configured.max(1)),
        "max" => 16,
        _ => configured.clamp(1, 16),
    }
}

/// Conexões para um download novo neste host.
pub fn connections_for(host: &str, mode: &str, configured: u32, limits: &BTreeMap<String, u32>) -> u32 {
    let base = mode_connections(mode, configured);
    limits.get(&host.to_ascii_lowercase()).map_or(base, |l| base.min(*l).max(1))
}

/// Depois de uma falha: devolve o novo limite do host, se a falha indica excesso de conexões.
pub fn after_failure(kind: DiagnosticKind, used_connections: u32) -> Option<u32> {
    let suspicious = matches!(kind, DiagnosticKind::ServerBusy | DiagnosticKind::Connection | DiagnosticKind::TooSlow);
    (suspicious && used_connections > 1).then(|| (used_connections / 2).max(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modos() {
        assert_eq!(mode_connections("economy", 8), 2);
        assert_eq!(mode_connections("balanced", 8), 8);
        assert_eq!(mode_connections("max", 4), 16);
        assert_eq!(mode_connections("balanced", 99), 16);
    }

    #[test]
    fn limite_por_host_so_reduz() {
        let mut l = BTreeMap::new();
        l.insert("cdn.x.com".to_string(), 2);
        assert_eq!(connections_for("CDN.x.com", "balanced", 8, &l), 2);
        assert_eq!(connections_for("outro.com", "balanced", 8, &l), 8);
        assert_eq!(connections_for("cdn.x.com", "economy", 8, &l), 2);
        assert_eq!(after_failure(DiagnosticKind::ServerBusy, 8), Some(4));
        assert_eq!(after_failure(DiagnosticKind::ServerBusy, 1), None);
        assert_eq!(after_failure(DiagnosticKind::NotFound, 8), None, "404 não tem a ver com conexões");
    }
}
