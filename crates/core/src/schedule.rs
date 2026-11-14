//! Agendamento: janela de horário em que os downloads podem rodar, e ação ao terminar tudo.

use chrono::{Datelike, NaiveTime, Timelike};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Schedule {
    pub enabled: bool,
    /// "HH:MM" local.
    pub start: String,
    pub end: String,
    /// 0 = domingo … 6 = sábado.
    pub days: Vec<u8>,
}

impl Default for Schedule {
    fn default() -> Self {
        Self { enabled: false, start: "01:00".into(), end: "07:00".into(), days: vec![0, 1, 2, 3, 4, 5, 6] }
    }
}

pub fn parse_hhmm(s: &str) -> Option<NaiveTime> {
    NaiveTime::parse_from_str(s.trim(), "%H:%M").ok()
}

impl Schedule {
    pub fn is_valid(&self) -> bool {
        parse_hhmm(&self.start).is_some() && parse_hhmm(&self.end).is_some() && self.days.iter().all(|d| *d <= 6)
    }

    /// Dentro da janela? Janelas que passam da meia-noite (23:00–06:00) contam para o dia em que começam.
    pub fn allows(&self, weekday_from_sunday: u8, minutes_of_day: u32) -> bool {
        if !self.enabled {
            return true;
        }
        let (Some(a), Some(b)) = (parse_hhmm(&self.start), parse_hhmm(&self.end)) else { return true };
        let (a, b) = (a.hour() * 60 + a.minute(), b.hour() * 60 + b.minute());
        let day = |d: u8| self.days.contains(&d);
        let yesterday = (weekday_from_sunday + 6) % 7;
        if a == b {
            return day(weekday_from_sunday);
        }
        if a < b {
            day(weekday_from_sunday) && (a..b).contains(&minutes_of_day)
        } else {
            (day(weekday_from_sunday) && minutes_of_day >= a) || (day(yesterday) && minutes_of_day < b)
        }
    }

    pub fn allows_now(&self) -> bool {
        let now = chrono::Local::now();
        self.allows(now.weekday().num_days_from_sunday() as u8, now.hour() * 60 + now.minute())
    }
}

/// O que fazer quando todos os downloads terminarem.
pub const AFTER_ALL: [&str; 4] = ["nothing", "sleep", "hibernate", "shutdown"];

#[cfg(test)]
mod tests {
    use super::*;

    fn s(start: &str, end: &str, days: &[u8]) -> Schedule {
        Schedule { enabled: true, start: start.into(), end: end.into(), days: days.to_vec() }
    }

    #[test]
    fn janela_no_mesmo_dia() {
        let x = s("08:00", "18:00", &[1, 2, 3, 4, 5]);
        assert!(x.allows(1, 8 * 60));
        assert!(!x.allows(1, 18 * 60));
        assert!(!x.allows(0, 12 * 60), "domingo fora");
    }

    #[test]
    fn janela_que_passa_da_meia_noite_conta_para_o_dia_inicial() {
        let x = s("23:00", "06:00", &[5]); // sexta à noite
        assert!(x.allows(5, 23 * 60 + 30));
        assert!(x.allows(6, 2 * 60), "madrugada de sábado ainda é a janela de sexta");
        assert!(!x.allows(6, 23 * 60 + 30));
        assert!(!x.allows(5, 2 * 60));
    }

    #[test]
    fn desligado_ou_invalido_permite_sempre() {
        assert!(Schedule::default().allows(3, 0));
        assert!(!s("25:00", "x", &[]).is_valid());
    }
}
