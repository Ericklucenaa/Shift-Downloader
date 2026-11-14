//! Critérios de aceitação da Fase 1 com o aria2 real e um servidor HTTP local.
//! Os IDs (CT-xxx) seguem o plano em docs/TEST_PLAN.md.

mod common;

use std::time::Duration;

use common::*;
use shift_core::{AddRequest, DiagnosticKind, TaskStatus};

fn req(url: String) -> AddRequest {
    AddRequest { url, ..Default::default() }
}

#[tokio::test(flavor = "multi_thread")]
async fn ct001_download_http_salva_conteudo_correto() {
    let srv = TestServer::start(vec![("/pacote.bin", Route::file(3 * 1024 * 1024 + 17))]).await;
    let env = Env::new();
    let m = env.manager().await;
    let t = m.add(req(srv.url("/pacote.bin"))).await.unwrap();
    assert_eq!(t.file_name, "pacote.bin");
    assert_eq!(t.resumable, Some(true));
    let done = wait_status(&m, &t.id, TaskStatus::Completed, 30).await;
    assert_eq!(done.completed_bytes, 3 * 1024 * 1024 + 17);
    let bytes = std::fs::read(env.downloads().join("pacote.bin")).unwrap();
    assert!(bytes == content(3 * 1024 * 1024 + 17), "conteúdo difere");
    assert!(!env.downloads().join("pacote.bin.aria2").exists(), "arquivo de controle removido ao concluir");
    let kinds: Vec<String> = m.events(&t.id).unwrap().into_iter().filter_map(|e| e.to_status).collect();
    assert!(kinds.contains(&"verifying".to_string()) && kinds.contains(&"completed".to_string()), "{kinds:?}");
    m.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn ct002_pausar_e_retomar_continua_do_ponto() {
    let len = 2 * 1024 * 1024;
    let srv = TestServer::start(vec![("/lento.bin", Route::file(len).throttle(400 * 1024))]).await;
    let env = Env::new();
    let m = env.manager().await;
    let t = m.add(req(srv.url("/lento.bin"))).await.unwrap();
    wait_for(&m, &t.id, 20, "algum progresso", |t| t.completed_bytes > 200 * 1024).await;
    m.pause(&t.id).await.unwrap();
    let paused = wait_status(&m, &t.id, TaskStatus::Paused, 5).await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let still = task(&m, &t.id);
    assert_eq!(still.status, TaskStatus::Paused);
    assert!(still.completed_bytes <= paused.completed_bytes + 64 * 1024, "não deveria continuar baixando pausado");
    assert_eq!(still.speed, 0);
    m.resume(&t.id).await.unwrap();
    wait_status(&m, &t.id, TaskStatus::Completed, 30).await;
    assert!(std::fs::read(env.downloads().join("lento.bin")).unwrap() == content(len));
    let resumed_from: Vec<u64> = srv.range_starts("/lento.bin").into_iter().filter(|s| *s > 0).collect();
    assert!(!resumed_from.is_empty(), "a retomada pediu um Range a partir do meio do arquivo");
    m.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn ct003_servidor_sem_range_baixa_convencional_sem_corromper() {
    let len = 1024 * 1024 + 3;
    let srv = TestServer::start(vec![("/simples.bin", Route::file(len).no_ranges())]).await;
    let env = Env::new();
    let m = env.manager().await;
    let t = m.add(req(srv.url("/simples.bin"))).await.unwrap();
    assert_eq!(t.resumable, Some(false), "o servidor ignorou o Range: sem retomada");
    wait_status(&m, &t.id, TaskStatus::Completed, 30).await;
    assert!(std::fs::read(env.downloads().join("simples.bin")).unwrap() == content(len));
    m.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn ct004_fechar_o_app_no_meio_e_reabrir_recupera_a_tarefa() {
    let len = 3 * 1024 * 1024;
    let srv = TestServer::start(vec![("/grande.bin", Route::file(len).throttle(500 * 1024))]).await;
    let env = Env::new();
    let id;
    {
        let m = env.manager().await;
        let t = m.add(req(srv.url("/grande.bin"))).await.unwrap();
        id = t.id.clone();
        wait_for(&m, &id, 20, "progresso", |t| t.completed_bytes > 300 * 1024).await;
        m.shutdown().await;
    }
    // Reabre com o mesmo banco e a mesma sessão.
    let m = env.manager().await;
    let t = task(&m, &id);
    assert!(t.completed_bytes > 300 * 1024, "progresso salvo: {}", t.completed_bytes);
    assert!(!t.status.is_terminal());
    wait_status(&m, &id, TaskStatus::Completed, 40).await;
    assert!(std::fs::read(env.downloads().join("grande.bin")).unwrap() == content(len));
    assert!(srv.range_starts("/grande.bin").iter().any(|s| *s > 0), "continuou do ponto salvo");
    m.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn ct004b_tarefa_pausada_continua_pausada_depois_de_reabrir() {
    let srv = TestServer::start(vec![("/p.bin", Route::file(2 * 1024 * 1024).throttle(300 * 1024))]).await;
    let env = Env::new();
    let id;
    let saved;
    {
        let m = env.manager().await;
        id = m.add(req(srv.url("/p.bin"))).await.unwrap().id;
        wait_for(&m, &id, 20, "progresso", |t| t.completed_bytes > 100 * 1024).await;
        m.pause(&id).await.unwrap();
        tokio::time::sleep(Duration::from_millis(800)).await;
        saved = task(&m, &id).completed_bytes;
        m.shutdown().await;
    }
    let m = env.manager().await;
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let t = task(&m, &id);
    assert_eq!(t.status, TaskStatus::Paused);
    // O aria2 relata 0 bytes para download pausado restaurado da sessão: não pode regredir.
    assert!(t.completed_bytes >= saved && saved > 100 * 1024, "progresso regrediu: {} < {saved}", t.completed_bytes);
    m.resume(&id).await.unwrap();
    wait_status(&m, &id, TaskStatus::Completed, 30).await;
    m.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn ct005_servidor_fora_do_ar_da_erro_de_conexao_compreensivel() {
    // Porta fechada: pega uma porta livre e não escuta nela.
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let env = Env::new();
    let m = env.manager().await;
    let err = m.add(req(format!("http://127.0.0.1:{port}/x.zip"))).await.unwrap_err();
    assert_eq!(err.kind, DiagnosticKind::Connection, "{err:?}");
    assert!(err.retryable);
    assert!(m.snapshot().unwrap().tasks.is_empty(), "nada é criado se o link não responde");
    m.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn ct006_link_expirado_informa_que_precisa_de_link_novo() {
    let srv = TestServer::start(vec![
        ("/gone.zip", Route::status(410)),
        // Responde à consulta e expira antes do download começar (link assinado de CDN).
        ("/assinado.zip", Route { expire_after: Some((1, 403)), ..Route::file(1024) }),
    ])
    .await;
    let env = Env::new();
    let m = env.manager().await;
    let err = m.add(req(srv.url("/gone.zip"))).await.unwrap_err();
    assert_eq!(err.kind, DiagnosticKind::Expired);
    assert!(!err.retryable);

    let t = m.add(req(srv.url("/assinado.zip"))).await.unwrap();
    let failed = wait_status(&m, &t.id, TaskStatus::Failed, 30).await;
    let e = failed.error.expect("diagnóstico presente");
    assert_eq!(e.kind, DiagnosticKind::Forbidden, "{e:?}");
    assert!(e.action.contains("link novo"), "{}", e.action);
    assert_eq!(failed.next_retry_at, None, "403 não tenta de novo sozinho");
    m.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn ct007_arquivo_maior_que_o_espaco_livre_e_recusado_antes_de_gravar() {
    let srv = TestServer::start(vec![("/enorme.iso", Route { fake_length: Some(1u64 << 60), ..Route::file(0) })]).await;
    let env = Env::new();
    let m = env.manager().await;
    let err = m.add(req(srv.url("/enorme.iso"))).await.unwrap_err();
    assert_eq!(err.kind, DiagnosticKind::DiskFull);
    assert!(!env.downloads().join("enorme.iso").exists(), "nenhum arquivo criado");
    assert!(m.snapshot().unwrap().tasks.is_empty());
    m.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn ct008_dois_downloads_com_mesmo_nome_nao_se_sobrescrevem() {
    let srv = TestServer::start(vec![("/a/doc.pdf", Route::file(4000)), ("/b/doc.pdf", Route::file(9000))]).await;
    let env = Env::new();
    std::fs::create_dir_all(env.downloads()).unwrap();
    std::fs::write(env.downloads().join("doc.pdf"), b"arquivo do usuario").unwrap();
    let m = env.manager().await;
    let a = m.add(req(srv.url("/a/doc.pdf"))).await.unwrap();
    let b = m.add(req(srv.url("/b/doc.pdf"))).await.unwrap();
    assert_eq!(a.file_name, "doc (1).pdf");
    assert_eq!(b.file_name, "doc (2).pdf");
    wait_status(&m, &a.id, TaskStatus::Completed, 30).await;
    wait_status(&m, &b.id, TaskStatus::Completed, 30).await;
    assert_eq!(std::fs::read(env.downloads().join("doc.pdf")).unwrap(), b"arquivo do usuario");
    assert_eq!(std::fs::metadata(env.downloads().join("doc (2).pdf")).unwrap().len(), 9000);
    m.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn ct014_redirecionamento_para_endereco_interno_e_bloqueado() {
    let srv = TestServer::start(vec![("/pega", Route::redirect("http://169.254.169.254/latest/meta-data/"))]).await;
    let env = Env::new();
    let m = env.manager().await;
    let err = m.add(req(srv.url("/pega"))).await.unwrap_err();
    assert_eq!(err.kind, DiagnosticKind::RedirectBlocked, "{err:?}");
    m.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn cancelar_apaga_a_parte_baixada_e_remover_limpa_o_historico() {
    let srv = TestServer::start(vec![("/c.bin", Route::file(2 * 1024 * 1024).throttle(200 * 1024))]).await;
    let env = Env::new();
    let m = env.manager().await;
    let t = m.add(req(srv.url("/c.bin"))).await.unwrap();
    wait_for(&m, &t.id, 20, "progresso", |t| t.completed_bytes > 50 * 1024).await;
    m.cancel(&t.id).await.unwrap();
    assert_eq!(task(&m, &t.id).status, TaskStatus::Cancelled);
    assert!(!env.downloads().join("c.bin").exists());
    assert!(!env.downloads().join("c.bin.aria2").exists());
    m.remove(&t.id).await.unwrap();
    assert!(m.snapshot().unwrap().tasks.is_empty());
    m.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn nome_vem_do_content_disposition_e_e_saneado() {
    let srv = TestServer::start(vec![(
        "/dl",
        Route::file(10).header("Content-Disposition", "attachment; filename*=UTF-8''..%2F..%2Frelat%C3%B3rio%3F.txt"),
    )])
    .await;
    let env = Env::new();
    let m = env.manager().await;
    let p = m.probe_url(&srv.url("/dl")).await.unwrap();
    assert_eq!(p.file_name, "relatório_.txt");
    let t = m.add(AddRequest { probe_id: Some(p.probe_id), ..req(srv.url("/dl")) }).await.unwrap();
    wait_status(&m, &t.id, TaskStatus::Completed, 20).await;
    assert!(env.downloads().join("relatório_.txt").exists());
    m.shutdown().await;
}

// ---------------------------------------------------------------- Fase 2 a 5

fn sha256_hex(data: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(data).iter().map(|b| format!("{b:02x}")).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn ct009_hash_errado_nao_marca_como_integro_e_hash_certo_conclui() {
    let len = 300_000;
    let srv = TestServer::start(vec![("/ok.bin", Route::file(len)), ("/ruim.bin", Route::file(len))]).await;
    let env = Env::new();
    let m = env.manager().await;
    let good = sha256_hex(&content(len));
    let a = m.add(AddRequest { expected_sha256: Some(good.to_uppercase()), ..req(srv.url("/ok.bin")) }).await.unwrap();
    let b = m.add(AddRequest { expected_sha256: Some("0".repeat(64)), ..req(srv.url("/ruim.bin")) }).await.unwrap();
    let a = wait_status(&m, &a.id, TaskStatus::Completed, 30).await;
    assert_eq!(a.sha256.as_deref(), Some(good.as_str()));
    let b = wait_status(&m, &b.id, TaskStatus::Failed, 30).await;
    assert_eq!(b.error.unwrap().kind, DiagnosticKind::Checksum);
    assert!(b.next_retry_at.is_none(), "hash errado não tenta de novo sozinho");
    // Sem hash informado: o SHA-256 é calculado em segundo plano e duplicados aparecem.
    let c = m.add(req(srv.url("/ok.bin"))).await.unwrap();
    wait_status(&m, &c.id, TaskStatus::Completed, 30).await;
    let c = wait_for(&m, &c.id, 10, "sha256 calculado", |t| t.sha256.is_some()).await;
    assert!(c.duplicate, "mesmo conteúdo de ok.bin");
    m.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn ct005_queda_de_conexao_no_meio_continua_sem_corromper() {
    let len = 2 * 1024 * 1024;
    let srv = TestServer::start(vec![("/cai.bin", Route { drop_once_after: Some(700 * 1024), ..Route::file(len) })]).await;
    let env = Env::new();
    let m = env.manager().await;
    let t = m.add(req(srv.url("/cai.bin"))).await.unwrap();
    wait_status(&m, &t.id, TaskStatus::Completed, 40).await;
    assert!(std::fs::read(env.downloads().join("cai.bin")).unwrap() == content(len));
    assert!(srv.range_starts("/cai.bin").iter().any(|s| *s > 0), "retomou do ponto da queda");
    m.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn ct012_cem_downloads_na_fila_sem_travar() {
    let routes: Vec<(String, Route)> = (0..100).map(|i| (format!("/f{i}.bin"), Route::file(20_000 + i))).collect();
    let srv = TestServer::start(routes.iter().map(|(p, r)| (p.as_str(), r.clone())).collect()).await;
    let env = Env::new();
    let m = env.manager().await;
    let started = std::time::Instant::now();
    let urls: Vec<String> = (0..100).map(|i| srv.url(&format!("/f{i}.bin"))).collect();
    let res = m.add_batch(urls, None, false).await;
    assert_eq!(res.iter().filter(|r| r.task.is_some()).count(), 100, "{:?}", res.iter().find(|r| r.error.is_some()));
    // O snapshot (o que a interface pede) continua rápido com 100 tarefas.
    let t0 = std::time::Instant::now();
    for _ in 0..20 {
        assert_eq!(m.snapshot().unwrap().tasks.len(), 100);
    }
    let per = t0.elapsed() / 20;
    assert!(per < std::time::Duration::from_millis(100), "snapshot levou {per:?}");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
    loop {
        let done = m.snapshot().unwrap().tasks.iter().filter(|t| t.status == TaskStatus::Completed).count();
        if done == 100 {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "só {done}/100 concluídos");
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    eprintln!("100 downloads em {:?}; snapshot médio {per:?}", started.elapsed());
    m.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn trocar_link_mantem_o_progresso_quando_o_arquivo_e_o_mesmo() {
    let len = 2 * 1024 * 1024;
    let srv = TestServer::start(vec![
        ("/velho.bin", Route::file(len).throttle(300 * 1024)),
        ("/novo/velho.bin", Route::file(len)),
    ])
    .await;
    let env = Env::new();
    let m = env.manager().await;
    let t = m.add(req(srv.url("/velho.bin"))).await.unwrap();
    wait_for(&m, &t.id, 20, "progresso", |t| t.completed_bytes > 200 * 1024).await;
    m.pause(&t.id).await.unwrap();
    m.change_url(&t.id, &srv.url("/novo/velho.bin")).await.unwrap();
    m.resume(&t.id).await.unwrap();
    wait_status(&m, &t.id, TaskStatus::Completed, 30).await;
    assert!(std::fs::read(env.downloads().join("velho.bin")).unwrap() == content(len));
    assert!(srv.range_starts("/novo/velho.bin").iter().any(|s| *s > 0), "o link novo continuou do meio");
    m.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn organizar_por_tipo_limite_por_download_fila_e_etiquetas() {
    let srv = TestServer::start(vec![("/doc.pdf", Route::file(5000)), ("/v.mp4", Route::file(5000))]).await;
    let env = Env::new();
    let m = env.manager().await;
    let mut s = m.settings();
    s.organize_by_type = true;
    m.update_settings(s).await.unwrap();
    let a = m.add(AddRequest { speed_limit: Some(50_000), ..req(srv.url("/doc.pdf")) }).await.unwrap();
    assert!(a.directory.ends_with("Documentos"), "{}", a.directory);
    assert_eq!(a.speed_limit, 50_000);
    let b = m.add(req(srv.url("/v.mp4"))).await.unwrap();
    assert!(b.directory.ends_with("Vídeos"));
    m.move_in_queue(&b.id, "top").await.unwrap();
    wait_status(&m, &a.id, TaskStatus::Completed, 30).await;
    wait_status(&m, &b.id, TaskStatus::Completed, 30).await;
    assert!(task(&m, &b.id).priority > task(&m, &a.id).priority);
    let t = m.set_tags(&a.id, vec![" trabalho ".into(), "Trabalho".into(), "".into(), "pdf".into()]).await.unwrap();
    assert_eq!(t.tags, vec!["pdf".to_string(), "trabalho".to_string()], "sem duplicadas nem vazias, em ordem");
    let r = m.rename(&a.id, "relatório?.pdf").await.unwrap();
    assert_eq!(r.file_name, "relatório_.pdf");
    assert!(std::path::Path::new(&r.directory).join("relatório_.pdf").exists());
    m.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn agendamento_fora_da_janela_segura_downloads_novos() {
    let srv = TestServer::start(vec![("/x.bin", Route::file(5000))]).await;
    let env = Env::new();
    let m = env.manager().await;
    let mut s = m.settings();
    // Janela de um minuto que nunca é agora (dia nenhum).
    s.schedule = shift_core::schedule::Schedule { enabled: true, start: "03:00".into(), end: "03:01".into(), days: vec![] };
    m.update_settings(s.clone()).await.unwrap();
    let t = m.add(req(srv.url("/x.bin"))).await.unwrap();
    assert_eq!(t.status, TaskStatus::Paused);
    assert_eq!(t.paused_reason.as_deref(), Some("schedule"));
    s.schedule.enabled = false;
    m.update_settings(s).await.unwrap();
    wait_status(&m, &t.id, TaskStatus::Completed, 30).await;
    m.shutdown().await;
}

// ---------------------------------------------------------------- mídia (FFmpeg real)

async fn make_sample(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
    let out = dir.join(name);
    let ffmpeg = tools_dir().join("ffmpeg").join("ffmpeg.exe");
    let st = tokio::process::Command::new(ffmpeg)
        .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "testsrc=size=320x240:rate=25:duration=3", "-f", "lavfi", "-i", "sine=frequency=440:duration=3", "-c:v", "libx264", "-c:a", "aac", "-shortest"])
        .arg(&out)
        .status()
        .await
        .unwrap();
    assert!(st.success());
    out
}

async fn wait_media(m: &shift_core::Manager, id: &str, secs: u64) -> shift_core::MediaView {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(secs);
    loop {
        let j = m.media().list().unwrap().into_iter().find(|j| j.id == id).unwrap();
        if matches!(j.status.as_str(), "completed" | "failed" | "cancelled") {
            return j;
        }
        assert!(std::time::Instant::now() < deadline, "mídia presa em {}", j.status);
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn ct010_converter_midia_local_gera_arquivo_reproduzivel() {
    let env = Env::new();
    let m = env.manager().await;
    assert!(m.media().available(), "ferramentas em apps/desktop/src-tauri/tools (rode scripts/fetch-tools.mjs)");
    let input = make_sample(env.dir.path(), "amostra.mp4").await;
    let probe = m.media().probe_file(input.to_str().unwrap()).await.unwrap();
    assert!((probe.info.duration.unwrap() - 3.0).abs() < 0.2);
    assert!(probe.presets.iter().any(|p| p.id == "mp3" && p.approx_size.is_some()));
    for preset in ["mp3", "webm", "mkv-remux"] {
        let j = m.media().add_convert(shift_core::ConvertRequest { path: input.to_string_lossy().to_string(), preset: preset.into(), options: Default::default() }).unwrap();
        let done = wait_media(&m, &j.id, 120).await;
        assert_eq!(done.status, "completed", "{preset}: {:?}", done.error);
        let out = done.output_path.unwrap();
        let again = m.media().probe_file(&out).await.unwrap();
        assert!((again.info.duration.unwrap() - 3.0).abs() < 0.3, "{preset}: duração {:?}", again.info.duration);
    }
    assert!(input.exists(), "original preservado");
    m.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn ct015_falha_no_ffmpeg_preserva_o_original_e_informa() {
    let env = Env::new();
    let m = env.manager().await;
    let input = make_sample(env.dir.path(), "audio.mp4").await;
    let before = std::fs::read(&input).unwrap();
    // Corte que começa depois do fim: o FFmpeg não gera nada.
    let j = m
        .media()
        .add_convert(shift_core::ConvertRequest {
            path: input.to_string_lossy().to_string(),
            preset: "mp4-remux".into(),
            options: shift_media::ffmpeg::ConvertOptions { start: Some(50.0), end: Some(60.0), ..Default::default() },
        })
        .unwrap();
    let done = wait_media(&m, &j.id, 60).await;
    assert_eq!(done.status, "failed");
    assert_eq!(done.error.unwrap().kind, DiagnosticKind::MediaFailed);
    assert_eq!(std::fs::read(&input).unwrap(), before, "original intacto");
    let leftovers: Vec<_> = std::fs::read_dir(env.dir.path()).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().contains("convertido")).collect();
    assert!(leftovers.is_empty(), "saída parcial apagada");
    // Arquivo que não é mídia: recusado na análise.
    let junk = env.dir.path().join("lixo.mp4");
    std::fs::write(&junk, b"isto nao e video").unwrap();
    assert!(m.media().probe_file(junk.to_str().unwrap()).await.is_err());
    m.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn torrent_local_entra_na_fila_sem_deixar_copias_na_pasta_do_usuario() {
    // .torrent mínimo (um arquivo de 5 bytes, sem rastreadores): só testa o caminho do SHIFT.
    let info = "d6:lengthi5e4:name9:iso.x.iso12:piece lengthi16384e6:pieces20:aaaaaaaaaaaaaaaaaaaae";
    let data = format!("d4:info{info}e");
    let env = Env::new();
    let m = env.manager().await;
    let file = env.dir.path().join("teste.torrent");
    std::fs::write(&file, data).unwrap();
    let preview = m.torrent_preview(file.to_str().unwrap()).unwrap();
    assert_eq!((preview.name.as_str(), preview.total_bytes), ("iso.x.iso", 5));
    let t = m
        .add_torrent(shift_core::TorrentRequest { source: file.to_string_lossy().to_string(), selected: None, directory: None, start_paused: true })
        .await
        .unwrap();
    assert_eq!(t.kind, "torrent");
    assert_eq!(t.category, "disk");
    tokio::time::sleep(Duration::from_millis(1500)).await;
    assert_eq!(task(&m, &t.id).status, TaskStatus::Paused);
    let torrents_in_downloads: Vec<_> =
        std::fs::read_dir(env.downloads()).unwrap().flatten().filter(|e| e.file_name().to_string_lossy().ends_with(".torrent")).collect();
    assert!(torrents_in_downloads.is_empty(), "nenhuma cópia de .torrent na pasta do usuário");
    m.cancel(&t.id).await.unwrap();
    assert_eq!(task(&m, &t.id).status, TaskStatus::Cancelled);
    m.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn mover_para_outra_pasta_e_excluir_manda_para_a_lixeira() {
    let srv = TestServer::start(vec![("/m.bin", Route::file(4000)), ("/x.bin", Route::file(3000))]).await;
    let env = Env::new();
    let m = env.manager().await;
    let a = m.add(req(srv.url("/m.bin"))).await.unwrap();
    wait_status(&m, &a.id, TaskStatus::Completed, 20).await;
    let dest = env.dir.path().join("Arquivo morto");
    std::fs::create_dir_all(&dest).unwrap();
    std::fs::write(dest.join("m.bin"), b"ja existia").unwrap();
    let moved = m.move_to(&a.id, dest.to_str().unwrap()).await.unwrap();
    assert_eq!(moved.file_name, "m (1).bin", "não sobrescreve o que já existe no destino");
    assert_eq!(std::fs::read(dest.join("m.bin")).unwrap(), b"ja existia");
    assert_eq!(std::fs::metadata(dest.join("m (1).bin")).unwrap().len(), 4000);
    assert!(!env.downloads().join("m.bin").exists());
    let b = m.add(req(srv.url("/x.bin"))).await.unwrap();
    wait_status(&m, &b.id, TaskStatus::Completed, 20).await;
    m.delete_file(&b.id).await.unwrap();
    assert!(!env.downloads().join("x.bin").exists(), "foi para a Lixeira");
    assert!(m.snapshot().unwrap().tasks.iter().all(|t| t.id != b.id));
    m.shutdown().await;
}
