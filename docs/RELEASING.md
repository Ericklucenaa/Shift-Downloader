# Publicar uma versão

O app instalado procura atualizações em
`https://github.com/Ericklucenaa/Shift-Downloader/releases/latest/download/latest.json`,
confere a assinatura do instalador com a chave pública de `tauri.conf.json` e instala.

## Chave de assinatura

- A chave privada fica **fora do repositório**, em `%USERPROFILE%\.tauri\shift-downloader.key`.
- Guarde uma cópia em lugar seguro. Sem ela, as instalações existentes não aceitam novas versões
  (seria preciso publicar uma chave nova e reinstalar manualmente).
- Nunca faça commit da chave nem a cole em issues ou no chat.

## Passos

1. Suba a versão em `apps/desktop/src-tauri/tauri.conf.json`, `apps/desktop/package.json` e no
   `Cargo.toml` do workspace.
2. Gere o instalador assinado (PowerShell):

   ```powershell
   $env:TAURI_SIGNING_PRIVATE_KEY = Get-Content -Raw "$env:USERPROFILE\.tauri\shift-downloader.key"
   $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = ""
   npm run build
   ```

3. Gere o manifesto: `node scripts/release-manifest.mjs notas.md`
4. Crie a release `vX.Y.Z` com o instalador (nome com pontos no lugar dos espaços) e o `latest.json`.
   O script mostra os dois caminhos prontos para o `gh release create`.

A release precisa ficar marcada como **Latest**: é dela que o link `releases/latest` lê.
