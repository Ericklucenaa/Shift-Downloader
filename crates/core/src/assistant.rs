//! Assistente local, baseado em regras: olha o histórico e sugere o que fazer.
//! Não usa rede nem IA; cada regra é testável e explicável.

use std::collections::BTreeMap;

use shift_persistence::{DownloadRecord, TaskStatus};

use crate::views::Suggestion;

pub struct Context<'a> {
    pub records: &'a [DownloadRecord],
    pub free_space: Option<u64>,
    pub engine_restarts: u32,
    pub host_limits: &'a BTreeMap<String, u32>,
    pub schedule_blocked: bool,
}

fn host(r: &DownloadRecord) -> String {
    url::Url::parse(&r.source_url).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_default()
}

pub fn suggestions(c: &Context<'_>) -> Vec<Suggestion> {
    let mut out = Vec::new();
    let mut push = |id: &str, level: &str, title: String, body: String| {
        out.push(Suggestion { id: id.into(), level: level.into(), title, body });
    };

    let failed: Vec<&DownloadRecord> = c.records.iter().filter(|r| r.status == TaskStatus::Failed).collect();
    let expired = failed.iter().filter(|r| matches!(r.error_kind.as_deref(), Some("expired" | "forbidden" | "not_found"))).count();
    if expired > 0 {
        push(
            "expired-links",
            "warn",
            format!("{expired} download(s) com link expirado ou removido"),
            "Copie o link de novo na página de origem e use \"Trocar link\" no download: o progresso é mantido quando o arquivo é o mesmo.".into(),
        );
    }

    if let Some(free) = c.free_space {
        let pending: i64 = c
            .records
            .iter()
            .filter(|r| r.status.is_engine_bound())
            .map(|r| r.total_bytes.unwrap_or(0) - r.completed_bytes)
            .filter(|n| *n > 0)
            .sum();
        if pending as u64 > free {
            push("disk", "warn", "Os downloads pendentes não cabem no disco".into(), "Libere espaço ou mude a pasta de destino antes que eles falhem.".into());
        } else if free < 5 * 1024 * 1024 * 1024 {
            push("disk-low", "info", "Pouco espaço livre na pasta de downloads".into(), "Menos de 5 GB livres. Downloads grandes podem falhar por falta de espaço.".into());
        }
    }

    let fragile = c
        .records
        .iter()
        .filter(|r| r.status == TaskStatus::Downloading && r.resumable == Some(false) && r.total_bytes.unwrap_or(0) > 1 << 30)
        .count();
    if fragile > 0 {
        push(
            "no-resume",
            "info",
            "Download grande sem retomada em andamento".into(),
            "O servidor não permite continuar de onde parou. Evite pausar ou fechar o SHIFT até terminar.".into(),
        );
    }

    if !c.host_limits.is_empty() {
        let hosts: Vec<&str> = c.host_limits.keys().map(String::as_str).take(3).collect();
        push(
            "smartspeed",
            "info",
            "O SmartSpeed reduziu conexões em alguns servidores".into(),
            format!("{} recusaram muitas conexões. Os próximos downloads deles usam menos conexões. Dá para zerar isso nas configurações.", hosts.join(", ")),
        );
    }

    if c.engine_restarts > 0 {
        push(
            "engine-restarts",
            "warn",
            "O motor de downloads reiniciou nesta sessão".into(),
            "Os downloads continuaram sozinhos. Se acontecer de novo, verifique se um antivírus está bloqueando o aria2c.exe.".into(),
        );
    }

    if c.schedule_blocked && c.records.iter().any(|r| r.paused_reason.as_deref() == Some("schedule")) {
        push("schedule", "info", "Downloads aguardando o horário agendado".into(), "Eles começam sozinhos quando a janela do agendamento abrir. Para baixar agora, desligue o agendamento ou continue o download manualmente.".into());
    }

    // Falhas repetidas no mesmo servidor.
    let mut by_host: BTreeMap<String, u32> = BTreeMap::new();
    for r in &failed {
        *by_host.entry(host(r)).or_default() += 1;
    }
    if let Some((h, n)) = by_host.into_iter().filter(|(h, n)| !h.is_empty() && *n >= 3).max_by_key(|(_, n)| *n) {
        push("host-failures", "warn", format!("{n} falhas em {h}"), "O servidor pode estar fora do ar ou bloqueando downloads. Tente mais tarde ou use outro espelho.".into());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(status: TaskStatus, err: Option<&str>, url: &str) -> DownloadRecord {
        DownloadRecord { status, error_kind: err.map(str::to_string), source_url: url.into(), ..Default::default() }
    }

    #[test]
    fn regras_basicas() {
        let recs = vec![
            rec(TaskStatus::Failed, Some("expired"), "https://a.com/1"),
            rec(TaskStatus::Failed, Some("connection"), "https://b.com/1"),
            rec(TaskStatus::Failed, Some("connection"), "https://b.com/2"),
            rec(TaskStatus::Failed, Some("connection"), "https://b.com/3"),
        ];
        let limits = BTreeMap::new();
        let s = suggestions(&Context { records: &recs, free_space: Some(1 << 40), engine_restarts: 0, host_limits: &limits, schedule_blocked: false });
        let ids: Vec<&str> = s.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, vec!["expired-links", "host-failures"]);
        assert!(s[1].title.contains("b.com"));
    }

    #[test]
    fn sem_problemas_sem_sugestoes() {
        let limits = BTreeMap::new();
        let s = suggestions(&Context { records: &[], free_space: Some(1 << 40), engine_restarts: 0, host_limits: &limits, schedule_blocked: false });
        assert!(s.is_empty());
    }

    #[test]
    fn disco_insuficiente_para_pendentes() {
        let mut r = rec(TaskStatus::Queued, None, "https://a.com/x");
        r.total_bytes = Some(10 << 30);
        let limits = BTreeMap::new();
        let s = suggestions(&Context { records: &[r], free_space: Some(1 << 30), engine_restarts: 2, host_limits: &limits, schedule_blocked: false });
        let ids: Vec<&str> = s.iter().map(|x| x.id.as_str()).collect();
        assert_eq!(ids, vec!["disk", "engine-restarts"]);
    }
}
