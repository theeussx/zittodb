# Arquitetura

## Princípios (do spec)

1. **Leve primeiro.** Sem Electron, sem bundle de ADB: usa `adb`/`scrcpy`/`fastboot` do sistema. Alvo: 2 GB de RAM.
2. **Backend dono da execução.** Rust é a única camada que executa processos. O frontend envia *operações tipadas*; nunca strings de comando.
3. **Local-first/offline.** Sem rede além da conversa com o aparelho; sem servidores, DBs ou telemetria.
4. **Sem re-implementar protocolo ADB.** O `adb` do sistema faz todo o trabalho.

## Camadas

```
┌────────────────────────────────────────────────────────────┐
│ Frontend (React + TS)                                      │
│  views → stores (zustand) → services/bridge.ts             │
│                     (única fronteira)                      │
└──────────────────────────┬─────────────────────────────────┘
                           │ invoke (comandos tipados) + events
┌──────────────────────────▼─────────────────────────────────┐
│ Backend (Rust, Tauri 2)                                    │
│  commands/* ──► adb/operations (allowlist) ──► processes/  │
│                 security/ (validação + risco)              │
│                 storage/ (settings, auditoria, histórico)  │
└──────────────────────────┬─────────────────────────────────┘
                           │ argv vetores (std::process)
              adb · scrcpy · fastboot (sistema)
```

## Contrato de frontend → backend

`src/services/bridge.ts` define a interface `Bridge` (~50 métodos). Duas implementações:

- **`TauriBridge`** — `invoke()` do `@tauri-apps/api/core`, um comando Rust por método.
- **`MockBridge`** (`src/services/mock.ts`) — mesma interface para modo demo no navegador e testes de UI.

**Erros** seguem um contrato único: `AppError { code: SCREAMING_SNAKE, details: string }`. Códigos: `NO_DEVICE`, `AMBIGUOUS_DEVICE`, `DEVICE_UNAUTHORIZED`, `DEVICE_OFFLINE`, `INVALID_SERIAL`, `INVALID_PATH`, `INVALID_PACKAGE`, `INVALID_ARGUMENT`, `CONFIRMATION_REQUIRED`, `FILE_EXISTS`, `ALREADY_RUNNING`, `PROCESS_FAILED`, `TIMEOUT`, `CANCELLED`, `UNSUPPORTED`, `UNEXPECTED`, `OPERATION_REJECTED`. O frontend mapeia `code → texto localizado` + "ver detalhes técnicos".

### Relatório diagnóstico P0

`collect_diagnostic_report` mantém a coleta no backend Rust e revalida o serial selecionado antes de cada leitura. Propriedades/memória, bateria, armazenamento e rede são seções independentes; uma falha vira `status: error` ou `unavailable` sem apagar as demais. `ReportPrivacy` mascara serial e rede por padrão. `export_diagnostic_report` serializa o DTO com `schemaVersion: 1` e grava JSON e Markdown com `OpenOptions::create_new` no `downloadDir` configurado ou em `~/Downloads/Zittodb`, usando nomes incrementais para colisões. O modo local não grava arquivos e retorna `UNSUPPORTED_LOCAL` para comandos não suportados.

## Fluxo de uma operação (execute_operation)

1. **Gate de confirmação** — operações destrutivas exigem a palavra digitada (`APAGAR`, `REMOVER`, `REINICIAR`…); senão `CONFIRMATION_REQUIRED`.
2. **Resolução de serial** — o serial vem da operação/seleção; nunca há chute de "qualquer dispositivo". Dispositivo ausente/autorização pendente → erro explícito.
3. **Validação + argv** — `DeviceOperation::to_adb_args()` valida cada campo (regex por tipo) e produz um **vetor de argumentos**; token com espaço vai entre aspas; nunca há `sh -c`.
4. **Lock por serial** — `Coordinator` serializa operações destrutivas do mesmo serial (uma por vez).
5. **Execução capturada** — `run_captured` com timeout por operação (ver tabela); `Reboot` tolera saída parcial do processo.
6. **Auditoria + undo** — entrada JSONL (limite 5.000) com `undo` (desativar→reativar, desinstalar→`pm install-existing`).

Exemplos de argv produzidos:

| Operação | argv (após `adb`) |
|---|---|
| get_props | `-s S shell getprop` |
| disable_package | `-s S shell pm disable-user --user 0 'com.x.y'` |
| screenshot | `-s S exec-out screencap -p` |
| install_apk | `-s S install -r /caminho/app.apk` |
| connect Wi-Fi | `connect 192.168.1.50:5555` (sem serial) |
| fastboot flash | `flash boot /caminho/boot.img` |

## Processos (src-tauri/src/processes/)

- **`ProcessRegistry`** — mapa `id → Child`; `Drop` mata o processo; `on_exit` do app derruba tudo (sem órfãos).
- **Matar:** `SIGTERM` → espera 3 s → `SIGKILL` (via `libc`).
- **Timeouts:** dispositivos 15 s · props 30 s · dump 60 s · mídia 30 s · install 600 s · transfer 3.600 s.
- **Shell:** sessão persistente por dispositivo; escrita via stdin; saída em eventos; nunca executa comando por conta própria.
- **Logcat:** `adb -s S logcat -v threadtime [TAG:PRIORITY]` (spec validado); linhas em evento; buffer limitado no frontend (padrão 5.000).
- **Transfers:** `pull` tem progresso real (poll do tamanho local a cada 300 ms vs `stat -c %s` remoto); `push` é indeterminado (o adb não expõe progresso); cancelar mata o child.

## scrcpy

- Registro próprio de processo (id fixo `scrcpy`); um por vez (`ALREADY_RUNNING`).
- Antes de iniciar, o backend executa `scrcpy --version` e rejeita versões anteriores a **3.2**, que não suportam corretamente as mudanças do Android 15 (`SCRCPY_INCOMPATIBLE`). O Zittodb não reimplementa nem substitui o `scrcpy-server`; a correção é usar o release oficial atualizado.
- **Presets preenchem lacunas** — se o usuário personaliza, vira `custom` e o app não sobrescreve.
- Flags: `--max-size`, `--max-fps`, `--video-bit-rate`, `--turn-screen-off`, `--always-on-top`, `--audio`, `--record` (orientação é seguida do aparelho; scrcpy não tem flag de orientação estável). O alias antigo `--bit-rate` foi removido no scrcpy 3.3.
- Gravação **nunca sobrescreve**: sufixo `-1`..`-99` se o arquivo existir.

## Eventos (backend → frontend)

| Evento | Payload |
|---|---|
| `shell-output` | `{ session, line }` |
| `logcat-line` | `{ session, line }` |
| `scrcpy-log` | `{ serial, line }` |
| `file-progress` | `{ id, status: running\|done\|error\|cancelled, doneBytes, total, message }` |

## Storage (src-tauri/src/storage/)

- **Settings** — `~/.config/app.zittodb.desktop/settings.json`; escrita **atômica** (`.tmp` + rename); corrompido → defaults.
- **Auditoria** — `audit.jsonl`, 5.000 linhas, cada linha `{ts, device, action, command, result, undo}`.
- **Histórico de dispositivos** — apenas metadados de último estado/visto.
- **Log do app** — arquivo com limite de 2 MB (rotação simples).

## Frontend (src/)

- **Estado:** um store zustand mínimo (settings, dispositivos, navegação, toasts). Nada de Redux.
- **i18n:** dicionários `pt-BR.ts`/`en-US.ts` (chaves planas); `t(key, params)`; componente nunca traz string de UI hardcoded.
- **CSS:** um único `global.css` com variáveis; tema via `data-theme` no `<html>`; **modos de desempenho** via `data-perf="low|ultra"` (desligam transições, animações, sombras, backdrop-filter).
- **Componentes:** primitivos pequenos (Button, Badge, Modal, ConfirmDialog, Toasts) + views por feature.

## Bundle

- `tauri.conf.json`: janela 1120×720 (mín 880×560); `freezePrototype: true`; sem CSP restritiva (o app não carrega fontes remotas nem eval).
- Alvos: **AppImage** e **.deb** (`bundle.targets`). Flatpak: configuração documentada em `docs/DEVELOPMENT.md`.
