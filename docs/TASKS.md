# Tarefas

Estado real em 13/11/2026 (versão 0.3.0). Só está marcado o que foi executado e testado.

## 0.3.0 — Integração com o Windows e acabamento ✅

- [x] Ícone na bandeja: fechar a janela mantém o SHIFT baixando; menu com Pausar/Continuar tudo e Sair; velocidade no ícone
- [x] Iniciar com o Windows (só na bandeja)
- [x] SHIFT como programa para links magnet e arquivos .torrent (registro em HKCU, tela de Aplicativos padrão)
- [x] Instalador pergunta sobre o programa padrão; desinstalador remove o que é do SHIFT
- [x] Boas-vindas na primeira abertura: padrão, iniciar com o Windows, extensão
- [x] Menu de botão direito nos downloads: abrir, pasta, escolher arquivos do torrent, prioridade, mover, excluir (Lixeira), converter para MP3/MP4, verificar vírus (Defender), etiquetas, trocar link
- [x] Botões "líquidos" (opcional)
- [ ] Adicionar rastreadores / ignorar razão em torrent já em andamento (o aria2 não permite mudar essas opções numa tarefa ativa)

## Fase 1 — Fundação ✅

- [x] Tauri 2 + React + Rust, SQLite com migrations, tema claro/escuro, pt-BR/en/es
- [x] aria2 supervisionado por RPC; HTTP/HTTPS com progresso real
- [x] Pausar, continuar, cancelar, tentar de novo, remover; recuperação depois de fechar/derrubar o app
- [x] Diagnóstico de erros; novas tentativas com espera crescente; conflito de nomes; espaço em disco
- [x] Bloqueio de redirecionamento para rede interna

## Fase 2 — Filas, recuperação, integridade e organização ✅

- [x] Prioridade: mover para o topo/fim da fila
- [x] Limite de velocidade por download
- [x] SHA-256 de todo arquivo concluído; verificação contra hash informado (CT-009)
- [x] Trocar link (mantém o progresso quando é o mesmo arquivo; recomeça se mudou)
- [x] Vários links de uma vez (até 500; colar várias linhas também abre o lote)
- [x] Renomear depois de concluído
- [x] Categorias e pastas automáticas por tipo
- [x] Biblioteca: busca, categorias, etiquetas, duplicados por hash
- [x] Notificações do sistema (concluído/falhou, com a janela em segundo plano)
- [ ] Credenciais HTTP (usuário/senha): **não feito** — o aria2 grava as opções de cada download no arquivo de sessão, então a senha iria para o disco em texto. Precisa de outra abordagem (ver SECURITY.md).

## Fase 3 — Navegadores e mídia ✅ (com ressalvas)

- [x] Extensão Chrome/Edge (MV3): menu de contexto (link, imagem, vídeo/áudio, página), captura opcional de downloads
- [x] Native Messaging autenticado (o próprio SHIFT é o host; ponte local com token); abre o app se estiver fechado
- [x] yt-dlp: análise com formatos reais, escolhas por resolução, só áudio, legendas, playlists, pausar/continuar/cancelar
- [x] FFmpeg: 10 presets, corte, resolução, estimativa de tamanho, aceleração de hardware detectada por teste real
- [ ] Firefox: não feito (a extensão é MV3 de Chromium; Firefox precisa de manifesto e registro próprios)
- [ ] Extensão publicada na Chrome Web Store/Edge Add-ons: não feito (hoje é instalada sem compactação)
- [ ] YouTube: o yt-dlp atual exige um interpretador de JavaScript externo (Deno) para muitos vídeos do YouTube, que não vem com o SHIFT. Outros sites suportados funcionam (testado com archive.org).

## Fase 4 — Torrents e automação ✅

- [x] Magnet e .torrent (escolha de arquivos, pasta-invólucro em caso de conflito, compartilhamento até a razão)
- [x] Agendamento por janela de horário e dias da semana; ação ao terminar tudo com contagem regressiva cancelável
- [x] SmartSpeed: modos e redução automática de conexões por servidor
- [x] Estatísticas por site e assistente local de sugestões
- [ ] Controle remoto pelo celular (módulo 11): não feito
- [ ] Sistema de plugins (módulo 12): não feito

## Fase 5 — Lançamento (parcial)

- [x] Testes de carga: 100 downloads simultâneos (CT-012)
- [x] Auditoria de acessibilidade automatizada (axe-core, WCAG 2.1 A/AA): 0 violações em 7 telas × 2 temas
- [x] Instalador NSIS com aria2, yt-dlp, FFmpeg e a extensão
- [ ] **Assinatura de código:** depende de um certificado de assinatura (compra/validação em seu nome)
- [ ] **Atualização automática:** depende de um lugar público para publicar as versões (ex.: repositório no GitHub) e de uma chave de assinatura das atualizações
- [ ] Memória ociosa ≤ 180 MB: medida em 195 MB na build 0.2.0 (WebView2 186 MB, aria2 4 MB, SHIFT 6 MB)
- [ ] Teste com leitor de tela (NVDA/Narrador) feito por uma pessoa
