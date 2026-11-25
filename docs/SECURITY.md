# Segurança

## Princípios aplicados

| Regra | Onde |
|---|---|
| A interface não executa comandos nem abre caminhos arbitrários | capability `default.json` (só `core:default`, `dialog:allow-open`, `notification:default`); abrir/mostrar arquivos recebe o ID e busca o caminho no banco |
| Subprocessos sem shell, argumentos estruturados | aria2 (`process.rs`), yt-dlp e FFmpeg (`core/media.rs`); a URL vai depois de `--` para não virar opção |
| RPC do aria2 só local, com segredo | `--rpc-listen-all=false`, porta aleatória, segredo de 256 bits por execução, fora dos logs |
| Nenhum processo filho sobrevive ao SHIFT | aria2 com `--stop-with-process`; yt-dlp/FFmpeg encerrados em árvore (`taskkill /T`) ao fechar ou cancelar |
| Nunca sobrescrever arquivos | aria2 com `--allow-overwrite=false`; FFmpeg com `-n`; política de conflito própria (`nome (1).ext`, pasta-invólucro em torrents) |
| Nomes vindos da rede são saneados | `security/filename.rs` (vale também para nomes de torrents e magnets) |
| Redirecionamento para rede interna é bloqueado | `security/network.rs` + `core/probe.rs`; a análise de mídia aplica a mesma checagem ao destino |
| Executáveis pedem confirmação antes de abrir | downloads e resultados de mídia |
| Logs e eventos sem credenciais | `redact_url` e `diagnostics::scrub` (também nas mensagens do yt-dlp e do FFmpeg) |
| Integridade | SHA-256 de todo download HTTP concluído; verificação contra hash informado; torrents conferidos peça a peça pelo protocolo |
| Ferramentas distribuídas com hash fixo | `scripts/fetch-tools.mjs` recusa yt-dlp/FFmpeg com SHA-256 diferente do registrado |
| Ação de energia só a configurada | `power_action` recusa qualquer ação diferente da escolhida em Agendamento; sempre precedida de contagem de 60 s cancelável |
| CSP restritiva | só `'self'` e IPC; imagens externas permitidas apenas por HTTPS (miniaturas de vídeo) |

## Integração com o navegador

- O navegador só inicia o host para a extensão com o ID fixo (`allowed_origins` no manifesto do host).
- O host fala com o app por uma ponte em 127.0.0.1 que exige o token de `%APPDATA%\app.shift.downloads\bridge.json` (gerado a cada abertura, apagado ao fechar). Pedidos com cabeçalho `Origin` (vindos de páginas web) são recusados.
- Só `http:`, `https:` e `magnet:` são aceitos. Com "Confirmar links vindos do navegador" ligado (padrão), nada começa sem o usuário ver a janela de novo download.
- A captura de downloads da extensão vem desligada; quando ligada, só cancela o download do navegador **depois** que o SHIFT confirma que recebeu o link.

## Política de destinos de rede

Referência: OWASP SSRF Prevention Cheat Sheet.

- **Link digitado:** público, privado (NAS) e loopback aceitos; link-local (`169.254.0.0/16`, `fe80::/10`) e endereços especiais nunca.
- **Redirecionamento:** não pode sair de público para privado/loopback, ir para link-local/especial ou trocar de protocolo. Cada salto é resolvido, validado e a conexão é presa aos IPs validados.

### Limitações conhecidas

1. A validação vale para a consulta inicial e cada nova tentativa. Se o servidor redirecionar de novo **durante** o download, o aria2 segue sem essa política; o yt-dlp segue redirecionamentos e busca recursos (manifestos, fragmentos) sem ela.
2. Torrents conversam com participantes arbitrários por definição; o aria2 também escuta conexões de entrada (o Windows pode pedir permissão de firewall).
3. **Credenciais HTTP não são suportadas** de propósito: o aria2 grava as opções de cada download no arquivo de sessão, então usuário/senha iriam para o disco em texto. Uma solução exige mudar de motor para esses casos ou um proxy local de autenticação.
4. Releases ainda não são assinadas e não há atualização automática (dependem de certificado e de um canal público de publicação).

## Registro do Windows (só HKCU)

| Chave | Quando |
|---|---|
| `Software\Classes\SHIFT.magnet`, `SHIFT.torrent`, `.torrent\OpenWithProgids`, `Software\SHIFT\Capabilities`, `Software\RegisteredApplications\SHIFT` | a cada abertura (só oferece o SHIFT como opção) |
| `Software\Classes\magnet`, `Software\Classes\.torrent` (padrão) | só quando o usuário pede "Tornar padrão" |
| `Software\Microsoft\Windows\CurrentVersion\Run\SHIFT` | só quando o usuário liga "Iniciar com o Windows" |
| `Software\{Google\Chrome,Microsoft\Edge}\NativeMessagingHosts\app.shift.downloads` | a cada abertura (integração com o navegador) |

O desinstalador remove essas chaves; `magnet` e `.torrent` só são apagados se ainda apontarem para o SHIFT. Builds de desenvolvimento não escrevem nada disso (a menos que `SHIFT_REGISTER=1`).

## Dados e privacidade

Tudo fica em `%APPDATA%\app.shift.downloads\` (banco, sessão do aria2, cópias de .torrent, manifesto do host e o token da ponte). Nada é enviado a servidores do SHIFT; não há conta nem telemetria. As miniaturas de vídeo são carregadas do site que você analisou.

## Dependências distribuídas

| Componente | Versão | Origem | SHA-256 |
|---|---|---|---|
| aria2c.exe | 1.37.0 | github.com/aria2/aria2 (release oficial) | `be2099c214f63a3cb4954b09a0becd6e2e34660b886d4c898d260febfe9d70c2` |
| yt-dlp.exe | 2026.11.23 | github.com/yt-dlp/yt-dlp | `622ed48023cac1daa52a3bf3a6ac38f03514de214b5f0fa069389fd0f6d17e5c` |
| FFmpeg (zip) | 9.0.2 gpl-shared | github.com/BtbN/FFmpeg-Builds | `f4e068477787394da18a44b27d3e2cb19e05b21495b836a4fd5a31958b608932` |

Licenças em `docs/third-party/NOTICE.txt`.

## Reportar um problema

Abra uma issue privada (security advisory) no repositório.
