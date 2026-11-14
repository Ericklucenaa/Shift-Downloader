# Roadmap

O diferencial não é a quantidade de recursos, e sim cinco coisas bem feitas: velocidade real, retomada confiável, integração simples, interface excelente e diagnóstico claro.

| Marco | Objetivo | Fases |
|---|---|---|
| **M1 — Substituir o download do navegador** | Baixar qualquer link HTTP(S) com mais confiabilidade que o navegador | 1 (feita), 2, extensão da 3 |
| **M2 — Central de mídia e automação** | Vídeo/áudio, conversão, torrents, agendamentos | 3 (mídia), 4 |
| **M3 — Lançamento** | Assinatura, atualização, acessibilidade, benchmarks | 5 |

## Ordem e critério de cada fase

1. **Fundação** ✅: app real com aria2, SQLite e HTTP/HTTPS; pausar, continuar, fechar e reabrir sem perder progresso.
2. **Filas, recuperação, integridade e organização**: prioridade, SHA-256, troca de link expirado, lote, categorias, biblioteca. Critério: CT-009 e CT-012 passando.
3. **Navegadores e mídia**: extensão com Native Messaging, yt-dlp, FFmpeg. Critério: CT-010, CT-011, CT-015.
4. **Torrents e automação**: magnet/.torrent, agendador, SmartSpeed.
5. **Lançamento**: assinatura de código, atualização segura, acessibilidade, benchmarks medidos em hardware documentado.

A meta de "superar as alternativas" será validada com testes comparativos na Fase 5, não pela lista de recursos.
