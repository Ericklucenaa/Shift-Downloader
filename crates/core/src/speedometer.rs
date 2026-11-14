//! Velocidade instantânea a partir dos bytes que realmente chegaram.
//!
//! O `downloadSpeed` do aria2 é uma média de vários segundos: na partida ele leva
//! de 7 a 14 s para mostrar a velocidade real (medido). Aqui a velocidade sai da
//! diferença de bytes entre amostras (a cada ~500 ms), suavizada por uma média
//! exponencial com constante de tempo de ~1 s: responde rápido sem "pular".

use std::time::Instant;

const TAU_SECS: f64 = 1.0;

#[derive(Debug, Default, Clone)]
pub struct Speedometer {
    last: Option<(Instant, i64)>,
    ema: f64,
}

impl Speedometer {
    /// Registra o total baixado agora e devolve a velocidade em bytes/s.
    pub fn sample(&mut self, at: Instant, done: i64) -> u64 {
        let Some((t0, d0)) = self.last else {
            self.last = Some((at, done));
            return 0;
        };
        let dt = at.saturating_duration_since(t0).as_secs_f64();
        if dt < 0.05 {
            return self.ema as u64;
        }
        if done < d0 {
            // Recomeçou (arquivo mudou no servidor, nova tentativa): zera a medição.
            self.reset();
            self.last = Some((at, done));
            return 0;
        }
        let instant = (done - d0) as f64 / dt;
        let alpha = 1.0 - (-dt / TAU_SECS).exp();
        self.ema = if self.ema == 0.0 { instant } else { self.ema + alpha * (instant - self.ema) };
        self.last = Some((at, done));
        self.ema.max(0.0) as u64
    }

    pub fn reset(&mut self) {
        self.last = None;
        self.ema = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn mostra_a_velocidade_real_em_menos_de_dois_segundos() {
        let t0 = Instant::now();
        let mut s = Speedometer::default();
        let rate = 50_000_000.0; // 50 MB/s constantes desde o início
        let mut shown = 0;
        for i in 0..=3 {
            let t = t0 + Duration::from_millis(500 * i);
            shown = s.sample(t, (rate * 0.5 * i as f64) as i64);
        }
        // Em 1,5 s já mostra a taxa real (o aria2 leva 7–14 s).
        assert!((shown as f64 - rate).abs() / rate < 0.02, "{shown}");
    }

    #[test]
    fn suaviza_oscilacoes_e_cai_quando_para() {
        let t0 = Instant::now();
        let mut s = Speedometer::default();
        let mut done = 0i64;
        s.sample(t0, 0);
        let mut vals = vec![];
        for i in 1..=8 {
            // alterna 40 e 60 MB/s a cada amostra: média 50
            done += if i % 2 == 0 { 30_000_000 } else { 20_000_000 };
            vals.push(s.sample(t0 + Duration::from_millis(500 * i), done));
        }
        let last = *vals.last().unwrap() as f64;
        assert!((last - 50e6).abs() < 6e6, "{last}");
        // parou de chegar byte: em 2 s a velocidade mostrada cai bastante
        let mut v = 0;
        for i in 9..=12 {
            v = s.sample(t0 + Duration::from_millis(500 * i), done);
        }
        assert!((v as f64) < last * 0.2, "{v}");
    }

    #[test]
    fn recomeco_zera() {
        let t0 = Instant::now();
        let mut s = Speedometer::default();
        s.sample(t0, 1000);
        s.sample(t0 + Duration::from_millis(500), 2_000_000);
        assert_eq!(s.sample(t0 + Duration::from_millis(1000), 10), 0);
    }
}
