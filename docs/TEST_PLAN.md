# Plano de testes

## Como rodar

```
node scripts/fetch-tools.mjs                # uma vez: yt-dlp e FFmpeg para os testes de mídia
npm test                                    # tudo
cargo test --workspace                      # Rust (unidade + aceitação)
cargo test -p shift-core --test acceptance  # aceitação: aria2 e FFmpeg reais + servidor HTTP local
npm test -w apps/desktop                    # Vitest
```

Os testes de aceitação sobem um servidor HTTP local controlável (`crates/core/tests/common/mod.rs`): com e sem Range, com limite de velocidade, status forçados, links que expiram, quedas de conexão e redirecionamentos. Cada teste roda um gerenciador real, com o `aria2c.exe` empacotado, numa pasta temporária própria.

## Critérios de aceitação (13/11/2026)

| ID | Cenário | Situação | Onde |
|---|---|---|---|
| CT-001 | Download HTTP salva conteúdo correto | ✅ | `ct001_…` (byte a byte) |
| CT-002 | Pausar e retomar continua do ponto | ✅ | `ct002_…` (Range a partir do meio) |
| CT-003 | Servidor sem Range | ✅ | `ct003_…` |
| CT-004 | Fechar o app no meio e reabrir | ✅ | `ct004_…`, `ct004b_…` + manual (1 GB real) |
| CT-005 | Queda de conexão / servidor fora | ✅ | `ct005_queda…` (cai no meio e continua do ponto), `ct005_servidor_fora…` |
| CT-006 | URL expirada informa link novo | ✅ | `ct006_…` + troca de link: `trocar_link_…` |
| CT-007 | Arquivo maior que o espaço livre | ✅ | `ct007_…` |
| CT-008 | Dois downloads com mesmo nome | ✅ | `ct008_…` |
| CT-009 | Hash incorreto não marca como íntegro | ✅ | `ct009_…` (também hash certo, hash em segundo plano e duplicados) |
| CT-010 | Converter mídia local | ✅ | `ct010_…` (MP3, WebM e MKV gerados de um vídeo real e lidos de volta pelo ffprobe) |
| CT-011 | Extensão com o app fechado | ✅ parcial | O host abre o app e espera a ponte (código em `native_host.rs`); testado com o app aberto via protocolo real (ver abaixo). Abertura a frio testada só manualmente pelo caminho do código |
| CT-012 | 100 tarefas na fila | ✅ | `ct012_…` (100 downloads; snapshot com 100 tarefas < 100 ms) |
| CT-013 | Reiniciar o PC durante download | ✅ parcial | Simulado com encerramento forçado (o aria2 morre junto); reinício físico não testado |
| CT-014 | Redirecionamento malicioso | ✅ | `ct014_…` |
| CT-015 | Falha no FFmpeg preserva o original | ✅ | `ct015_…` (original byte a byte igual, saída parcial apagada, arquivo inválido recusado) |

Outros cenários automatizados: organizar por tipo, limite por download, fila, etiquetas, renomear (`organizar_por_tipo_…`); agendamento fora da janela (`agendamento_…`); torrent local sem deixar cópias na pasta do usuário (`torrent_local_…`); cancelar apaga a parte baixada; Content-Disposition malicioso.

## Testes de unidade

- `shift-security` (11), `shift-persistence` (10), `shift-download-engine` (2), `shift-media` (8: escolhas de qualidade, argumentos, progresso, erros do yt-dlp; presets, ffprobe e estimativas do FFmpeg)
- `shift-core` (21): diagnósticos, Content-Disposition, categorias, .torrent (BEP 3) e magnet, agendamento (inclusive janelas que passam da meia-noite), SmartSpeed, SHA-256, assistente
- `shift-desktop` (1): protocolo do Native Messaging
- Frontend (11, Vitest): formatação, filtros e ordem, biblioteca, completude das traduções, tradução de diagnósticos

## Verificação manual no app real (13/11/2026)

Com capturas pelo DevTools do WebView2:

- **HTTP:** 100 MB e 1 GB (ash-speed.hetzner.com) a 6–14 MB/s; pausar, continuar, encerramento forçado e fechamento normal → continuou do ponto.
- **Torrent:** `debian-13.7.0-amd64-netinst.iso.torrent` oficial → 792 MB baixados; **SHA-256 igual ao SHA256SUMS oficial do Debian**. Arquivo de controle limpo depois de reiniciar.
- **Mídia:** archive.org (Big Buck Bunny): análise com formatos reais (720p/360p/áudio), download em 360p (H.264 + AAC, 596 s conferidos com ffprobe), extração para MP3.
- **Navegador:** o executável chamado como o Chrome chama (`chrome-extension://…`): `ping` → app rodando; `add` → a janela "Novo download" abriu no app; `file:///` recusado. Registro do host conferido para Chrome e Edge.
- **Acessibilidade:** axe-core injetado na janela real: 0 violações WCAG 2.1 A/AA em Visão geral, Todos, Torrents, Vídeo e áudio, Biblioteca, Agendamentos e Configurações, nos temas claro e escuro; diálogo de vários links também sem violações.
- **Aceleração de hardware:** NVENC detectado nesta máquina por codificação de teste.

## Lacunas conhecidas

- A extensão não foi carregada num Chrome/Edge real nesta sessão (o protocolo e o host foram testados diretamente).
- YouTube exige Deno para o yt-dlp atual (não incluído).
- Teste com leitor de tela feito por uma pessoa ainda não aconteceu.
- Memória ociosa: 195 MB medidos na build de release 0.2.0 (meta 180 MB); quase tudo é o WebView2. Abertura da janela abaixo de 1 s nesta máquina.
