# Fase 2 — P0 relatório diagnóstico — 2026-10-09

## Status

**PASSOU** para a vertical de contrato, coleta, mascaramento, preview e exportação JSON/Markdown. A integração fake ADB passou isoladamente e com `--test-threads=1`; execução paralela apresentou uma falha transitória `Text file busy` do fixture legado.

## Entrega

- `src-tauri/src/commands/reports.rs`
  - `collect_diagnostic_report`: coleta ferramentas, conexão ADB, propriedades, memória, bateria, armazenamento e rede.
  - Cada seção tem status independente; falha parcial não aborta o relatório.
  - Revalida o serial e o estado conectado antes de cada leitura.
  - `export_diagnostic_report`: grava JSON e Markdown no `downloadDir` configurado ou `~/Downloads/Zittodb`.
  - `OpenOptions::create_new` impede sobrescrita silenciosa; colisões usam nome incremental.
- `src-tauri/src/reports.rs`
  - DTO JSON camelCase, `schemaVersion`, estados de seção e mascaramento conservador.
- `src/services/bridge.ts`, `src/services/mock.ts`, `src/types/index.ts`
  - contrato real Tauri, mock de demo/teste e tipos compartilhados.
- `src/features/diagnostics/DiagnosticsView.tsx`
  - opções para incluir serial e rede, preview JSON e exportação dos dois formatos.
- `src/i18n/pt-BR.ts`, `src/i18n/en-US.ts`
  - chaves do fluxo P0 nos dois idiomas.

## Evidências

| Comando/artefato | Resultado | Observação |
|---|---|---|
| `cargo test --manifest-path src-tauri/Cargo.toml` | PASSOU | 88 testes unitários; a primeira suíte completa também executou 7 integrações, com uma falha transitória no fixture fake ADB. |
| `cargo test --manifest-path src-tauri/Cargo.toml --test integration ... --exact --test-threads=1` | PASSOU | Descoberta fake ADB reproduzível isoladamente. |
| `npm run check` | PASSOU | Versões sincronizadas, build Vite e 51 testes Vitest. |
| `src-tauri/src/commands/reports.rs` | PASSOU | Teste `write_new_file_refuses_overwrite`. |
| `tests/mock-bridge.test.ts` | PASSOU | Sucesso completo mascarado, exportação, parcial e offline. |
| `src-tauri/src/reports.rs` | PASSOU | Schema, masking, Markdown, nomes incrementais e seções parciais. |

## Critérios P0 cobertos

- caso completo: coberto pelo MockBridge e coleta Rust;
- caso parcial: seção de rede indisponível no demo e seções independentes no Rust;
- offline/ausente: erro explícito `DEVICE_OFFLINE`/`NO_DEVICE`;
- serial/rede mascarados por padrão: coberto em Rust e Vitest;
- colisão: teste de nome incremental e `create_new`;
- schemaVersion: `1`, documentado no DTO compartilhado;
- diretório inválido/permissão: mapeamento para `INVALID_PATH`/`OPERATION_REJECTED` no backend;
- modo local: não escreve relatório; comando desconhecido no serviço local retorna `UNSUPPORTED_LOCAL`.

## Não verificado

- coleta contra aparelho físico nesta sandbox;
- permissão real de diretório configurado pelo usuário;
- bundle AppImage/.deb e instalação limpa;
- assinatura/updater;
- `cargo clippy`, `npm audit`, medição de desempenho e `make check` desta etapa.

## Riscos e decisões

- A integração fake ADB existente é sensível à execução paralela: a descoberta passa com `--test-threads=1`, mas uma execução paralela apresentou `Text file busy`. Não foi alterada nesta etapa para evitar misturar estabilização do fixture com o P0.
- O export recebe o DTO já coletado, mas a coleta real permanece exclusivamente no backend Rust; o modo local não cria arquivos.
- P1 inspetor/fila de APK ainda não foi iniciado; deve ser vertical separado após revisão do P0.
