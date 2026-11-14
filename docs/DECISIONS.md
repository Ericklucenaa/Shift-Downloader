# Decisões técnicas

Formato: contexto → decisão → consequência. Mais recentes no fim.

## D-001 · aria2 como motor, Rust como orquestrador (13/11/2026)

**Contexto.** Download segmentado, retomada e checkpoint são difíceis de fazer certo.
**Decisão.** Usar o aria2 1.37.0 (build oficial Windows) via JSON-RPC. O núcleo Rust só orquestra.
**Consequência.** Fase 1 entregue com retomada confiável. O aria2 está sem novas versões desde 11/2023; se isso virar risco (TLS, bugs), o adaptador `download-engine` isola a troca por um motor Rust próprio.

## D-002 · SQLite é a fonte do estado; o aria2 é a fonte dos bytes (13/11/2026)

**Decisão.** Não substituir o checkpoint do aria2 por tabelas. O banco guarda estado, GID e metadados para recriar a tarefa no motor.
**Consequência.** Recuperação funciona mesmo se o arquivo de sessão sumir, desde que o `.aria2` exista.

## D-003 · GID escolhido pelo SHIFT (13/11/2026)

**Decisão.** Derivar o GID do UUID da tarefa e gravá-lo antes de chamar `addUri`.
**Consequência.** Não há janela em que o motor tem uma tarefa que o banco não conhece.

## D-004 · Consulta própria antes de entregar ao aria2 (13/11/2026)

**Contexto.** O aria2 segue redirecionamentos sozinho, sem política de rede.
**Decisão.** O SHIFT consulta o link com `reqwest` (redirecionamento manual, cada salto validado, conexão presa aos IPs validados) e entrega ao aria2 a URL final.
**Consequência.** Bloqueia o caso comum de redirecionamento malicioso. Limitação documentada em SECURITY.md: se o servidor redirecionar *depois* (durante o download), o aria2 segue sem a validação.

## D-005 · Progresso não regride por leitura transitória (13/11/2026)

**Contexto.** Encontrado no teste manual: o aria2 relata 0 bytes para um download pausado restaurado da sessão até reabrir o arquivo de controle, e esse zero era gravado no banco.
**Decisão.** Para tarefas pausadas/na fila (ou leitura 0 em ativas), o progresso salvo prevalece. Recomeços deliberados (arquivo mudou) zeram o progresso explicitamente.
**Consequência.** Coberto pelo teste `ct004b`, que falha sem a correção.

## D-006 · Ordem estável na lista (13/11/2026)

**Decisão.** Mais recentes primeiro, sempre. Não reordenar por situação.
**Consequência.** A linha não pula de lugar quando pausa ou conclui; filtros cobrem "em andamento", "concluídos" e "com falha".

## D-007 · Diagnósticos no núcleo, tradução na interface (13/11/2026)

**Decisão.** O núcleo produz `Diagnostic { kind, title, action, detail }` em português. A interface traduz por `kind` para inglês e espanhol; mensagens específicas (`kind = unknown`) mantêm o texto do núcleo.

## D-008 · Estrutura do repositório (13/11/2026)

**Decisão.** Seguir a estrutura proposta, com ajustes: crates `media` e `browser-extension` só serão criados nas fases em que tiverem código (Fase 3). `packages/shared-types` foi substituído por `apps/desktop/src/lib/types.ts`, espelho dos tipos serde, enquanto só há um consumidor.
**Consequência.** Nenhum diretório vazio fingindo trabalho; a duplicação de tipos é pequena e verificada pela compilação do frontend contra o uso real.

## D-009 · Arquivos perigosos pedem confirmação no núcleo (13/11/2026)

**Decisão.** `open_file` recebe o ID da tarefa, busca o caminho no banco e devolve `needsConfirmation` para executáveis/scripts. A interface não tem permissão de abrir caminhos arbitrários (capability só com `core:default` e `dialog:allow-open`).

## D-010 · Mídia num gerenciador próprio, fora do aria2 (13/11/2026)

**Decisão.** Downloads de vídeo/áudio (yt-dlp) e conversões (FFmpeg) ficam na tabela `media_jobs`, com fila e limite de concorrência próprios (`MediaManager`). O crate `shift-media` só monta argumentos e interpreta saídas; o núcleo executa os processos.
**Consequência.** Uma conversão pesada não ocupa vaga de download, e o código de processos fica num lugar só (inclui encerrar a árvore de processos, porque o yt-dlp chama o FFmpeg).

## D-011 · Ferramentas de mídia baixadas por script, com hash fixo (13/11/2026)

**Decisão.** yt-dlp e FFmpeg (~200 MB) não vão para o git. `scripts/fetch-tools.mjs` baixa versões fixas e confere o SHA-256 antes de instalar em `src-tauri/tools/`. O instalador os inclui como recursos.
**Consequência.** Builds reproduzíveis; trocar de versão é mudar URL e hash num lugar só. O yt-dlp pode se atualizar sozinho pelo botão nas configurações (sites mudam com frequência).

## D-012 · Conversão sempre conferida pelo ffprobe (13/11/2026)

**Contexto.** Encontrado pelo CT-015: um corte fora da duração fazia o FFmpeg sair com sucesso gerando só o cabeçalho do arquivo.
**Decisão.** Toda saída é lida de volta pelo ffprobe; sem duração ou sem fluxos, conta como falha e o arquivo é apagado.

## D-013 · O próprio SHIFT é o native messaging host (13/11/2026)

**Decisão.** O Chrome/Edge chama `shift-desktop.exe chrome-extension://<id>/`; nesse modo o executável não abre janela e repassa pedidos a uma ponte HTTP em 127.0.0.1 com token aleatório gravado em `%APPDATA%\app.shift.downloads\bridge.json`. Se o app estiver fechado, o host o abre e espera a ponte.
**Consequência.** Nenhum executável extra para manter; páginas web não conseguem falar com a ponte (não leem o token e pedidos com `Origin` são recusados). O ID da extensão é fixo pela chave pública no manifesto.

## D-014 · Torrents: categoria pelo maior arquivo, conflito por pasta-invólucro (13/11/2026)

**Decisão.** Se já existe algo com o nome do torrent na pasta, o download vai para `<pasta>/<nome> (1)/`. A categoria é a do maior arquivo escolhido. O aria2 não grava cópias do .torrent na pasta do usuário (`--rpc-save-upload-metadata=false`); o SHIFT guarda a sua em `%APPDATA%`.

## D-015 · Agendamento pausa com motivo (13/11/2026)

**Decisão.** Pausas têm motivo (`user`, `schedule`, `override`). O agendamento só retoma o que ele mesmo pausou; continuar manualmente fora da janela marca `override` e é respeitado até a janela abrir.

## D-016 · Contraste ajustado pela auditoria (13/11/2026)

**Decisão.** Os tons de texto secundário (`--muted`, `--faint`) foram escurecidos/clareados até passar 4,5:1 nos dois temas, e a lista de downloads deixou de usar `role=option` com botões dentro (aninhamento interativo). Resultado: 0 violações no axe-core.

## D-017 · Bandeja do sistema e fechar ≠ sair (13/11/2026)

**Decisão.** Fechar a janela esconde o SHIFT na área de notificação (padrão, desligável em Configurações); o ícone mostra a velocidade, abre a janela com um clique e tem menu com Pausar/Continuar tudo e "Sair do SHIFT", que faz o encerramento ordenado (salva progresso, para o aria2 e a mídia). Iniciado com o Windows (`--minimized`), o SHIFT sobe só na bandeja.

## D-018 · Programa padrão do Windows sem sequestrar escolhas (13/11/2026)

**Contexto.** O Windows não tem "gerenciador de downloads padrão"; downloads de sites passam pelo navegador (por isso a extensão). O que o Windows associa são protocolos e tipos de arquivo.
**Decisão.** A cada abertura o SHIFT só se registra como opção (ProgIDs `SHIFT.magnet`/`SHIFT.torrent`, `Capabilities`, `RegisteredApplications`, "Abrir com"). Assumir magnet/.torrent só acontece quando o usuário pede (instalador, boas-vindas ou Configurações); se outro programa já é a escolha do usuário, o Windows 10/11 exige confirmação na tela de Aplicativos padrão, que o SHIFT abre já filtrada. O desinstalador remove só o que é do SHIFT.
**Consequência.** Builds de desenvolvimento não registram nada (a menos que `SHIFT_REGISTER=1`), e `npm run dev:isolado` roda com outro identificador, banco e instância — para não interferir no SHIFT instalado.

## D-019 · Menu de contexto = mesma lista do "⋯" (13/11/2026)

**Decisão.** Uma única função monta as ações do download, renderizada pelo Radix tanto no botão "⋯" quanto no botão direito. Itens que o motor não suporta com segurança (adicionar rastreadores ou ignorar a razão de um torrent em andamento, abrir a página de origem que não é guardada) ficam de fora em vez de aparecerem sem funcionar. "Excluir arquivo" usa a Lixeira; "Verificar vírus" usa o Microsoft Defender (MpCmdRun) do próprio Windows.

## D-020 · Botões "líquidos" como opção (13/11/2026)

**Decisão.** A pedido do usuário, um estilo alternativo de botões em vidro (cápsula, reflexo, botão principal aceso com brilho), escolhido em Configurações → Aparência. O padrão continua o clássico. O miolo do botão aceso é escuro o bastante para o texto branco manter 4,5:1.

## D-021 · Velocidade exibida pelos bytes reais; blocos de 1 MiB (13/11/2026)

**Contexto.** Comparado lado a lado com o Free Download Manager, o SHIFT "demorava a atingir a banda". Medido (`crates/core/examples/speedcheck.rs`, bench com o aria2 puro): o `downloadSpeed` do aria2 é uma média que leva 7–14 s para mostrar a velocidade real, enquanto os bytes já chegavam rápido; HTTP e HTTPS deram o mesmo resultado (a criptografia não é o gargalo); 8 × 16 conexões empataram em servidor estável (539 MB × 520 MB em 3 rodadas de 15 s), e 16 conexões fizeram um servidor (OVH) bloquear o IP e outro limitar a 2 conexões.
**Decisão.** A velocidade exibida passa a sair da diferença de bytes a cada ~500 ms com média exponencial de ~1 s (`speedometer.rs`); `--min-split-size` cai de 4 MiB para 1 MiB, para arquivos médios usarem todas as conexões. O padrão continua 8 conexões; o modo "Máximo" (16, o limite do aria2) fica disponível, com o SmartSpeed recuando sozinho quando o servidor reclama.

## D-022 · Material líquido como padrão (13/11/2026)

**Decisão.** A pedido do usuário, a interface ganhou um material de acrílico/gel (`src/liquid.css`, `components/ui/liquid.tsx`): cápsula com espessura, reflexo especular, borda luminosa e luz interna que surge ao selecionar (laranja no claro, vermelho no escuro), afundando ao pressionar. Aplicado só onde há função: ação principal, botões secundários, grupos de opção (uma acesa por vez, setas do teclado), chaves e item ativo da barra lateral; superfícies e listas continuam sóbrias. Passa a ser o padrão; "Clássico" continua disponível. Auditoria axe-core: 0 violações com o material ligado.

## D-023 · Magnets como no qBittorrent: DHT pronta + rastreadores públicos (13/11/2026)

**Contexto.** Medido com um magnet só com o hash (Ubuntu 24.04.3, legal): a configuração anterior e a DHT com ponto de entrada não obtiveram os metadados em 60 s; com rastreadores públicos abertos somados, 6,7 s. Pelo gerenciador completo, o mesmo magnet baixou 20 MB em 15 s de 6 fontes.
**Decisão.** aria2 com DHT IPv4/IPv6, pontos de entrada (`router.bittorrent.com`), tabela salva em `%APPDATA%` (cada abertura fica mais rápida), troca de participantes, descoberta local e até 100 participantes; lista de rastreadores públicos somada a magnets/torrents (opção "Rastreadores públicos extras", ligada por padrão). A extensão captura cliques em links `magnet:` e manda para o SHIFT; se ele não responder, o link segue para o programa padrão do Windows.
