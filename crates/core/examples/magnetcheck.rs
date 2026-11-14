//! Medição manual: um magnet (de preferência sem rastreadores) chega aos metadados e começa a baixar?
//! Uso: cargo run -p shift-core --example magnetcheck -- "<magnet>" [segundos]

use std::sync::Arc;
use std::time::{Duration, Instant};

use shift_core::{EngineState, EventSink, Manager, ManagerConfig, Stats, TaskView, TorrentRequest};

struct Null;
impl EventSink for Null {
    fn tasks_changed(&self, _: &[TaskView]) {}
    fn tasks_removed(&self, _: &[String]) {}
    fn engine_changed(&self, _: &EngineState) {}
    fn stats(&self, _: &Stats) {}
}

#[tokio::main]
async fn main() {
    let magnet = std::env::args().nth(1).expect("magnet");
    let secs: u64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(90);
    let dir = tempfile::tempdir().unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::fs::create_dir_all(dir.path().join("dl")).unwrap();
    let m = Manager::start(
        ManagerConfig {
            db_path: dir.path().join("d/shift.db"),
            aria2_binary: root.join("apps/desktop/src-tauri/binaries/aria2c-x86_64-pc-windows-msvc.exe"),
            session_file: dir.path().join("d/aria2.session"),
            default_download_dir: dir.path().join("dl"),
            user_agent: "SHIFT-magnetcheck".into(),
            tools_dir: None,
        },
        Arc::new(Null),
    )
    .await
    .unwrap();
    while !matches!(m.snapshot().unwrap().engine, EngineState::Ready { .. }) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    let t = m.add_torrent(TorrentRequest { source: magnet, selected: None, directory: None, start_paused: false }).await.unwrap();
    println!("adicionado como: {}", t.file_name);
    let t0 = Instant::now();
    let mut meta_at = None;
    while t0.elapsed() < Duration::from_secs(secs) {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let v = m.snapshot().unwrap().tasks.into_iter().find(|x| x.id == t.id).unwrap();
        if meta_at.is_none() && !v.file_name.starts_with("magnet-") {
            meta_at = Some(t0.elapsed());
            println!("metadados em {:.1} s → {} ({} MB)", t0.elapsed().as_secs_f64(), v.file_name, v.total_bytes.unwrap_or(0) / 1_048_576);
        }
        if v.completed_bytes > 20 * 1_048_576 {
            println!("baixando: {} MB em {:.1} s, {} fontes, {:.0} Mbit/s", v.completed_bytes / 1_048_576, t0.elapsed().as_secs_f64(), v.seeders, v.speed as f64 * 8.0 / 1e6);
            break;
        }
    }
    if meta_at.is_none() {
        println!("NÃO obteve metadados em {secs} s");
    }
    let _ = m.cancel(&t.id).await;
    m.shutdown().await;
}
