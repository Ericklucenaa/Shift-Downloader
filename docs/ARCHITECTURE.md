# Arquitetura

## Camadas

```
┌──────────────────────────── apps/desktop (React) ────────────────────────────┐
│ Sidebar · Barra de link · Lista · Painel de detalhes · Visão geral · Ajustes │
│ lib/ipc.ts (única porta para o núcleo) · lib/store.ts (zustand)              │
└───────────────▲ eventos shift://tasks|removed|engine|stats    │ invoke(...)  ┘
                │                                                ▼
┌──────────── apps/desktop/src-tauri (camada de comandos) ─────────────────────┐
│ commands.rs: um comando por ação, recebe IDs (nunca caminhos)                │
│ sink.rs: EventSink → app.emit                                                │
└───────────────▲──────────────────────────────────────────────▼───────────────┘
┌──────────────────────────── crates/core ─────────────────────────────────────┐
│ Manager: estado das tarefas (SQLite) ⇄ motor (aria2)                         │
│   op_lock serializa: ciclo de acompanhamento, reconciliação, comandos        │
│ probe.rs: consulta o link, valida cada redirecionamento                      │
│ diagnostics.rs: erro → causa + ação                                          │
└──────┬──────────────────────┬─────────────────────────────┬──────────────────┘
       ▼                      ▼                             ▼
 crates/persistence     crates/download-engine        crates/security
 SQLite (WAL)           aria2 supervisionado          nomes, URLs, redes
                        JSON-RPC em 127.0.0.1
```

## Fonte da verdade

- **Estado da tarefa** (situação, nome, pasta, URL, erro, tentativas): SQLite, tabela `downloads`. Toda mudança passa por `Db::transition`, que valida a transição e grava um evento em `download_events` na mesma transação.
- **Bytes já baixados**: o aria2 (arquivo `.aria2` ao lado do download + sessão `aria2.session`). O SHIFT não tenta reimplementar o checkpoint; guarda o progresso só para exibir e para conferir.
- **GID**: o SHIFT escolhe o GID do aria2 (derivado do UUID da tarefa) e o grava antes de adicionar. Assim, se a sessão do aria2 se perder, a tarefa é recriada com o mesmo GID e o aria2 continua do arquivo `.aria2`.

## Máquina de estados

```
probing ─► queued ◄──────────────┐
             │  ▲                │
             ▼  │                │ (retomar / nova tentativa)
        downloading ◄─► paused   │
             │                   │
             ▼                   │
         verifying ─► completed  │
             │                   │
             └──────► failed ────┘
  (qualquer não final) ─► cancelled
```

`TaskStatus::can_transition` (crates/persistence/src/model.rs) é a regra; transições fora dela são recusadas.

## Ciclo de acompanhamento

A cada 500 ms, com `op_lock`:

1. Lê as tarefas que deveriam estar no motor (`queued`, `downloading`, `paused`…).
2. Um `system.multicall` com `tellStatus` de todas.
3. Mapeia a situação do aria2 para a do SHIFT, guarda velocidade e bytes ao vivo em memória e grava o progresso no banco a cada 2 s (ou na hora, se a situação mudou).
4. `complete` → `verifying` (confere o tamanho no disco) → `completed`.
5. `error` → diagnóstico; falhas passageiras agendam nova tentativa (5, 10, 20, 40, 80 s).
6. Emite para a interface só as tarefas cuja visão mudou.

Uma vez por segundo, emite estatísticas (velocidade total e histórico de 5 min).

## Reconciliação (SmartRecovery, parte 1)

Sempre que o aria2 (re)inicia:

1. Aplica as opções globais atuais (simultâneos, limite de velocidade).
2. Remove do aria2 o que não pertence a uma tarefa ativa no banco (ex.: falha que voltou pela sessão).
3. Recria no aria2 as tarefas que o banco tem e o motor não, com o mesmo GID (e pausadas, se estavam pausadas).

Ao recriar ou retomar uma falha, o link é consultado de novo. Se ETag, tamanho ou Last-Modified mudaram, a parte baixada é descartada e o download recomeça (evita juntar pedaços de versões diferentes).

## Processo aria2

- Iniciado sem shell e sem janela, com argumentos estruturados (`process.rs::build_args`).
- RPC só em 127.0.0.1, porta livre aleatória, segredo aleatório de 256 bits por execução.
- `--stop-with-process=<pid do SHIFT>`: se o SHIFT morrer, o aria2 encerra sozinho.
- Supervisor reinicia com espera crescente (1–16 s) e desiste depois de 5 falhas rápidas seguidas, avisando a interface.
- Ao fechar a janela: `saveSession` + `shutdown`, espera até 5 s, depois encerra à força.

## Interface

- `lib/ipc.ts` concentra todos os comandos e eventos. Os componentes não chamam `invoke` diretamente.
- O snapshot é carregado depois de assinar os eventos, para nada se perder no meio.
- Números em IBM Plex Mono com algarismos tabulares (não "dançam" quando mudam).
- Lista com container queries: em larguras menores, velocidade e tempo restante saem da linha (continuam no painel de detalhes).

## Mídia (Fase 3)

```
Interface ──invoke──► MediaManager (core/media.rs) ──► yt-dlp.exe / ffmpeg.exe / ffprobe.exe
                         │  fila própria (media_jobs), limite configurável (1–4)
                         └─ shift-media: argumentos + leitura de progresso/erros (funções puras)
```

- **Análise:** `yt-dlp -J --flat-playlist` → formatos reais → escolhas (melhor, por resolução, só áudio). Tamanhos aproximados só quando o site informa.
- **Download:** progresso por `--progress-template` (vídeo e áudio separados contam como duas etapas; pós-processamento à parte); caminho final por `--print after_move`.
- **Conversão:** presets em `shift-media::ffmpeg`; progresso por `-progress pipe:1`; saída sempre conferida pelo ffprobe. Aceleração de hardware detectada por uma codificação de teste (NVENC → QSV → AMF).
- **Pausar/cancelar:** encerra a árvore de processos; o yt-dlp retoma dos `.part`. Ao reabrir o app, trabalhos interrompidos voltam para a fila.

## Torrents (Fase 4)

Pelo mesmo aria2. `.torrent` é lido pelo SHIFT (`core/torrent.rs`, BEP 3: nome, arquivos, info hash) para mostrar e escolher os arquivos; uma cópia fica em `%APPDATA%\…\torrents\<id>.torrent` para recriar a tarefa se a sessão se perder. Magnets: o aria2 baixa os metadados num GID e continua em outro (`followedBy`); o SHIFT acompanha a troca. Ao concluir, a tarefa vira "concluída" e o aria2 segue compartilhando até a razão configurada.

## Navegador (Fase 3)

```
Extensão MV3 ──sendNativeMessage──► shift-desktop.exe chrome-extension://… (modo host, sem janela)
                                        │ lê bridge.json (porta + token)
                                        ▼
                         Ponte HTTP 127.0.0.1 (bridge.rs) ──emit "shift://external-add"──► Interface
```

O registro (`HKCU\Software\{Google\Chrome,Microsoft\Edge}\NativeMessagingHosts\app.shift.downloads`) é refeito a cada abertura do app. Se o app estiver fechado, o host o abre e espera a ponte; o plugin de instância única evita duas janelas.

## Automação (Fase 4)

- **Agendamento:** a cada 15 s, fora da janela, pausa (motivo `schedule`); dentro, retoma só o que ele pausou.
- **Ao terminar tudo:** quando não há downloads nem mídia ativos depois de alguma atividade, o núcleo emite `shift://after-all`; a interface conta 60 s (cancelável) e chama `power_action`, que só executa a ação configurada.
- **SmartSpeed:** conexões por modo; falhas do tipo "servidor ocupado/conexão caiu/lento" com várias conexões reduzem o limite daquele host pela metade (persistido em `settings.host_limits`).
- **Assistente:** regras puras em `core/assistant.rs` sobre o histórico (links expirados, disco, servidores com falhas repetidas, reinícios do motor…).
