# Como contribuir

## Antes de abrir um PR

```
npm run lint   # tsc + clippy -D warnings
npm test       # Vitest + cargo test --workspace (inclui aceitação com aria2 real)
```

Os dois precisam passar. Mudanças de interface: anexe capturas nos temas claro e escuro e na largura mínima (960 × 620).

## Regras do projeto

- **Regra de negócio fica no `shift-core`.** A camada Tauri só traduz comandos e eventos; a interface só fala com o núcleo por `src/lib/ipc.ts`.
- **Toda mudança de situação passa por `Db::transition`.** Não atualize a coluna `status` direto.
- **Nada fictício na interface.** Recurso que não existe aparece desabilitado com a fase prevista, nunca como botão que não faz nada. Números exibidos vêm do núcleo.
- **Subprocessos sem shell**, com argumentos estruturados.
- **Logs sem URLs completas, tokens ou senhas.** Use `redact_url` e `diagnostics::scrub`.
- **Nunca sobrescrever arquivos do usuário.**
- **Textos novos nos três idiomas** (`src/lib/locales/`). Há um teste que falha se faltar chave.
- **Decisão técnica relevante vai para `docs/DECISIONS.md`**, e o estado real das tarefas para `docs/TASKS.md`.
- **Nunca marque como testado o que não foi executado.**

## Estilo

- Rust: `cargo fmt`, Clippy sem avisos, erros com `thiserror`, comentários em português explicando o porquê.
- TypeScript: estrito, componentes pequenos, sem `any`.
- Visual: tokens de cor em `styles.css` (`--ink`, `--line`, `--accent`…); nada de cor solta em componente. Números em `.num` (IBM Plex Mono, tabular).
