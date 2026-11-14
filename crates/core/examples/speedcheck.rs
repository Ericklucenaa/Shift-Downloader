//! Medição manual: velocidade exibida pelo SHIFT × bytes reais, segundo a segundo.
//! Uso: cargo run -p shift-core --example speedcheck -- <url> [segundos]

use std::sync::Arc;
use std::time::{Duration, Instant};

use shift_core::{AddRequest, EngineState, EventSink, Manager, ManagerConfig, Stats, TaskView};

struct Null;
impl EventSink for Null {
    fn tasks_changed(&self, _: &[TaskView]) {}
    fn tasks_removed(&self, _: &[String]) {}
    fn engine_changed(&self, _: &EngineState) {}
    fn stats(&self, _: &Stats) {}
}

#[tokio::main]
async fn main() {
    let url = std::env::args().nth(1).expect("url");
    let secs: u64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(12);
    let dir = tempfile::tempdir().unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let m = Manager::start(
        ManagerConfig {
            db_path: dir.path().join("d/shift.db"),
            aria2_binary: root.join("apps/desktop/src-tauri/binaries/aria2c-x86_64-pc-windows-msvc.exe"),
            session_file: dir.path().join("d/aria2.session"),
            default_download_dir: dir.path().join("dl"),
            user_agent: "SHIFT-speedcheck".into(),
            tools_dir: None,
        },
        Arc::new(Null),
    )
    .await
    .unwrap();
    while !matches!(m.snapshot().unwrap().engine, EngineState::Ready { .. }) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    std::fs::create_dir_all(dir.path().join("dl")).unwrap();
    let t = m.add(AddRequest { url, ..Default::default() }).await.unwrap();
    let t0 = Instant::now();
    let mut last = (t0, 0i64);
    println!("  t   | exibido pelo SHIFT | real (bytes/s)  | conexões");
    while t0.elapsed() < Duration::from_secs(secs) {
        tokio::time::sleep(Duration::from_millis(1000)).await;
        let v = m.snapshot().unwrap().tasks.into_iter().find(|x| x.id == t.id).unwrap();
        let now = Instant::now();
        let real = (v.completed_bytes - last.1) as f64 / now.duration_since(last.0).as_secs_f64();
        last = (now, v.completed_bytes);
        println!("{:5.1} | {:>12.0} Mbit/s | {:>9.0} Mbit/s | {}", t0.elapsed().as_secs_f64(), v.speed as f64 * 8.0 / 1e6, real * 8.0 / 1e6, v.connections);
    }
    let _ = m.cancel(&t.id).await;
    m.shutdown().await;
}
