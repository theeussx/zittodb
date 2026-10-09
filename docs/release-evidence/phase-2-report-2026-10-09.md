# Fase 2 — Toolchain e contrato inicial do P0

**Data:** 2026-10-09  
**Branch:** `feat/v2.0`  
**Escopo:** preparar o ambiente e implementar somente o contrato/formatadores puros do relatório diagnóstico.

## O que foi feito

- Instalados no ambiente:
  - Rust estável via `rustup`: `rustc 1.99.0`, `cargo 1.99.0`;
  - `adb` 1.0.41 / platform-tools 34.0.4;
  - `fastboot` 34.0.4;
  - bibliotecas nativas Tauri/WebKit/GTK usadas pelo projeto.
- Criado `src-tauri/src/reports.rs` com:
  - `REPORT_SCHEMA_VERSION = 1`;
  - estados `ok`, `unavailable` e `error` por seção;
  - representação de erro normalizado por seção;
  - contrato de dispositivo, ferramentas, limitações e seções;
  - mascaramento conservador de serial e rede;
  - serialização JSON estável;
  - formatação Markdown determinística;
  - escolha de nome incremental sem sobrescrita silenciosa.
- Registrado o módulo em `src-tauri/src/lib.rs`.
- Adicionados os tipos TypeScript equivalentes em `src/types/index.ts`.
- Nenhum comando Tauri, Bridge, coleta real, escrita em disco ou UI foi conectado ainda.
- Nenhuma dependência nova foi adicionada.

## Evidências

| Comando/artefato | Resultado | Observação |
|---|---|---|
| `cargo test --manifest-path src-tauri/Cargo.toml` | **PASSOU** | suíte completa: 86 testes passaram, incluindo 5 novos do relatório |
| `cargo test ... reports::tests` | **PASSOU** | schema, parcialidade, mascaramento, Markdown e colisão |
| `npm run check` | **PASSOU** | versão sincronizada, TypeScript/Vite e 49 testes frontend |
| `cargo fmt --check` | **FALHOU** | divergências de formatação preexistentes em arquivos não tocados pelo P0 |
| `cargo clippy -D warnings` | **FALHOU** | 16–18 lint errors preexistentes no baseline, fora do módulo novo |
| `npm audit --audit-level=high` | **FALHOU** | vulnerabilidade alta preexistente em `source-map-js` |
| `npm run check:deps` | **PASSOU** | `adb`, `scrcpy` e `fastboot` encontrados |
| `scripts/measure-performance.sh` | **PASSOU** | `adb devices`: 7 ms; nenhum dispositivo físico disponível para `get-state`; RSS do script: 3516 kB |

Logs completos:

- `docs/release-evidence/phase-2-toolchain-2026-10-09.log`
- `docs/release-evidence/phase-2-rust-native-2026-10-09.log`
- `docs/release-evidence/phase-2-contract-2026-10-09.log`

## Não verificado ou limitado

- O pacote Ubuntu fornece `scrcpy 1.25`, inferior ao mínimo documentado pelo projeto (`3.2`); não foi substituído silenciosamente.
- Não há aparelho físico/emulador ADB conectado; a medição de `get-state` e validação de coleta real continuam não verificadas.
- `cargo fmt --check` e Clippy refletem problemas anteriores à implementação deste contrato.
- A vulnerabilidade alta de `source-map-js` ainda precisa de decisão/revisão de dependência.
- O relatório ainda não é coletado nem exportado pelo caminho real da aplicação.

## Próximo passo

Implementar o orquestrador Rust de coleta parcial, reutilizando as operações existentes, com serial explícito/revalidado e conversão de cada resultado para `ReportSection`; somente depois conectar Bridge, MockBridge, escrita segura e UI.
