<p align="center"><img src="apps/desktop/src-tauri/icons/128x128@2x.png" width="128" height="128" alt="Ícone do Shift Downloader"></p>

# Shift Downloader

[![Apoie no Ko-fi](https://img.shields.io/badge/Ko--fi-apoiar%20o%20projeto-2BB3A3?logo=ko-fi&logoColor=white)](https://ko-fi.com/ericklucena)

Gerenciador de downloads para Windows (antes chamado SHIFT): links HTTP/HTTPS, torrents, vídeo e áudio de sites compatíveis, conversão de mídia, agendamento e integração com Chrome/Edge. Núcleo em Rust (Tauri 2), motor aria2, interface em React.

**Versão 0.5.1.** O que está pronto, o que foi testado e o que ficou de fora está em [docs/TASKS.md](docs/TASKS.md) e [docs/TEST_PLAN.md](docs/TEST_PLAN.md).

## O que ele faz

- **Downloads HTTP/HTTPS** com várias conexões, pausar/continuar, recuperação depois de fechar ou derrubar o app, fila com prioridade, limite de velocidade global e por download, vários links de uma vez, troca de link expirado mantendo o progresso.
- **Integridade:** SHA-256 de todo arquivo concluído; se você informar o hash da página, o arquivo só é marcado como íntegro se bater. Duplicados aparecem na biblioteca.
- **Diagnóstico claro:** cada falha diz a causa e o que fazer (link expirado, servidor fora, disco cheio, redirecionamento bloqueado…). Falhas passageiras tentam de novo sozinhas.
- **Torrents:** links magnet e arquivos .torrent, com escolha de arquivos e compartilhamento até a razão configurada.
- **Vídeo e áudio (yt-dlp):** mostra as qualidades que o site oferece de verdade, playlists, legendas e só áudio (M4A/MP3). Não contorna DRM, paywall nem login.
- **Conversão (FFmpeg):** MP4, WebM, MKV, MP3, M4A, FLAC, WAV e Opus, com corte, resolução, estimativa de tamanho e aceleração de hardware (NVENC/QSV/AMF) quando disponível. O original nunca é alterado.
- **Biblioteca:** tudo que terminou, por categoria, etiqueta, duplicados e busca.
- **Agendamento:** janela de horário por dia da semana e ação ao terminar tudo (suspender, hibernar, desligar), sempre com 60 s para cancelar.
- **SmartSpeed e assistente:** modos econômico/equilibrado/máximo; servidores que recusam conexões em excesso passam a receber menos; sugestões baseadas no histórico.
- **Navegador:** extensão para Chrome e Edge (menu de contexto, captura opcional de downloads) via Native Messaging autenticado.
- **Interface:** português, inglês e espanhol; tema claro/escuro; teclado completo; sem violações WCAG 2.1 A/AA no axe-core.

## Instalar para testar

Rode `Shift Downloader_0.5.1_x64-setup.exe` (gerado em `target/release/bundle/nsis/`). Instala só para o usuário atual, sem pedir administrador. O instalador ainda **não é assinado**, então o Windows SmartScreen pode avisar ("Mais informações" → "Executar assim mesmo").

**Atualizações:** a partir da 0.5.1 o app procura versões novas sozinho (ao abrir e a cada 6 horas), avisa na barra lateral e instala com um clique em Configurações → Atualizações. Cada instalador é conferido por assinatura antes de rodar. Quem está na 0.5.0 ou anterior precisa instalar a 0.5.1 manualmente uma vez.

**Extensão do navegador:** depois de abrir o Shift Downloader uma vez, vá em Configurações → Navegador, clique em "Abrir pasta da extensão" e siga as instruções da tela (chrome://extensions ou edge://extensions → Modo do desenvolvedor → Carregar sem compactação).

## Desenvolver

Pré-requisitos (Windows 10/11 x64): Node.js 22+, Rust estável (`winget install Rustlang.Rustup`), Visual Studio Build Tools com C++, WebView2.

```
npm install
node scripts/fetch-tools.mjs   # baixa yt-dlp e FFmpeg (versões fixas, SHA-256 conferido)
npm run dev                    # app em modo de desenvolvimento
npm test                       # Vitest + todos os testes Rust (aceitação com aria2 e FFmpeg reais)
npm run lint                   # TypeScript + Clippy com avisos como erro
npm run build                  # instalador NSIS
```

## Estrutura

```
apps/desktop/            interface React + TypeScript (Vite, Tailwind 4, Radix)
apps/desktop/src-tauri/  camada Tauri: comandos, eventos, ponte do navegador, native host
apps/browser-extension/  extensão Manifest V3 (Chrome/Edge)
crates/core/             gerenciador de downloads, mídia, torrents, agendamento, assistente
crates/download-engine/  supervisão do aria2 + cliente JSON-RPC
crates/media/            argumentos e leitura de saída do yt-dlp e do FFmpeg
crates/persistence/      SQLite: tarefas, eventos, etiquetas, mídia, migrations
crates/security/         nomes de arquivo, URLs, política de redes internas
docs/                    arquitetura, decisões, testes, segurança, tarefas, roadmap
```

Documentos: [Arquitetura](docs/ARCHITECTURE.md) · [Decisões](docs/DECISIONS.md) · [Plano de testes](docs/TEST_PLAN.md) · [Segurança](docs/SECURITY.md) · [Roadmap](docs/ROADMAP.md) · [Tarefas](docs/TASKS.md) · [Como contribuir](docs/CONTRIBUTING.md)

## Licenças de terceiros

aria2 (GPL-2.0+), FFmpeg (build GPL) e yt-dlp (Unlicense) rodam como programas separados. Detalhes em [docs/third-party/NOTICE.txt](docs/third-party/NOTICE.txt).

## Apoie o projeto

O Shift Downloader é gratuito e de código aberto. Se ele te ajuda, considere apoiar no **[Ko-fi](https://ko-fi.com/ericklucena)**: o botão também fica na barra lateral do app, em "Apoiar o projeto".
