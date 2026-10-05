# Desenvolvimento

## Toolchain

- **Node** 20.19+ ou 22.12+, **Rust** estável (edition 2021). Instale o Rust pelo [rustup](https://rustup.rs/) e abra um novo terminal para carregar o `cargo` no `PATH`. O `tauri-cli` v2 é instalado como dependência de desenvolvimento pelo `npm install`.
- **Dependências de sistema do Tauri 2 (Debian/Ubuntu):**
  ```bash
  sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
    libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
  ```
- **Ferramentas Android (aplicativo, não do toolchain):**
  ```bash
  sudo apt install adb scrcpy        # ou baixe platform-tools da Android
  ```

## Setup

```bash
git clone https://github.com/theeussx/zittodb.git && cd wadb
npm install
```

## Marca e nomes técnicos

O nome do produto é **Zittodb** (*Zitto* + **db** de **D**ebug **B**ridge — não é um banco
de dados). A marca vive em poucos lugares; nada de nome antigo deve voltar:

| Onde | Arquivo | Valor |
|---|---|---|
| Identidade exibida no frontend | `src/config/app.ts` | `name`, `tagline`, `version` |
| i18n do tagline | `src/i18n/pt-BR.ts`, `src/i18n/en-US.ts` | `app.tagline` |
| Título/meta do navegador | `index.html` | `<title>`, description, theme-color |
| Produto e janela do Tauri | `src-tauri/tauri.conf.json` | `productName`, `title`, `identifier`, descrições do bundle |
| Crate Rust | `src-tauri/Cargo.toml` | `name = "zittodb"`, `[lib] name = "zittodb_lib"` |
| Pacote npm | `package.json` | `name`, `description` |
| Diretórios locais | `src-tauri/src/storage/mod.rs` | `ProjectDirs::from("app", "zittodb", "desktop")` |
| Log e pastas de mídia | `src-tauri/src/applog.rs`, `commands/media.rs` | `zittodb.log`, `~/Pictures/Zittodb` |
| Eventos internos | `src/App.tsx`, `src/hooks/useShortcuts.ts` | `zittodb:*` |
| Cabeçalho do serviço local | `src/services/bridge.ts`, `server/local.ts` | `X-Zittodb-Client: local` |
| Ícones | `scripts/generate-icons.mjs` | `npm run icons` regenera tudo |
| Assets do README | `docs/assets/*.svg` | lockup e marca |

Ao trocar o identificador (`app.zittodb.desktop`), **os dados mudam de lugar**: as
pastas antigas de configuração deixam de ser lidas (settings, auditoria e histórico
não migram automaticamente). Se isso precisar acontecer, faça uma migração explícita
em `storage::app_dirs()` — nunca deixe duas pastas crescendo em paralelo sem avisar.

O repositório aparece em `REPO_SLUG` (`src/services/updateService.ts`) e em
`package.json → repository`; ao renomear no GitHub, atualize esses dois lugares.

## Rodar

| Comando | O que faz |
|---|---|
| `npm run dev` | Frontend e API ADB local em `http://localhost:1420`; suporte inicial a dispositivos, informações, lista de apps, shell e logs. Ver limites no README. |
| `npm run dev:demo` | Demonstração no navegador, sem ADB. |
| `npm run check` | TypeScript, build e testes JavaScript/TypeScript. |
| `npm run tauri:dev` | App desktop completo (frontend + Rust), porta 1420. |
| `npm run tauri:build` | Bundle release: **AppImage** e **.deb** em `src-tauri/target/release/bundle/`. |
| `npm test` | Testes do frontend (vitest): i18n, presets, MockBridge, diálogo de confirmação. |
| `make test-rust` | Testes do backend: `cargo test` em `src-tauri/` (unitários + integração com `fake-adb`). |
| `make check` | `tsc --noEmit` + `vitest` + `cargo test` (o que for disponível). |

## Scripts

- `scripts/check-deps.sh` — verifica se `adb`, `scrcpy` e `fastboot` existem e imprime versões (idêntico ao que o app faz em "Configurações").
- `scripts/measure-performance.sh` — mede o que a spec §15 pede sem telemetria: tempo de `adb devices`, `adb get-state`, e consumo de memória do próprio processo (sem coletar métricas contínuas).

## Flatpak (alvo adicional)

O bundle principal é AppImage + .deb (`tauri.conf.json → bundle.targets`). Para Flatpak:

1. Crie um `.flatpakref`/manifest (ex.: `flatpak/zittodb.json`) herdando a runtime `org.freedesktop.Platform` 24.08 com SDK webkitgtk-4.1.
2. Empacote `adb`/`scrcpy` como extensão do Flatpak **ou** permita o host via `talk-name` — o app detecta as ferramentas no PATH, então basta expor os binários no sandbox.
3. `flatpak build` com `npm ci && npm run build` e o binário release do Tauri.

(Isso está fora do escopo de build automático deste repo; a configuração de bundle Tauri já cobre AppImage/.deb.)

## Estrutura de commits

- Mensagens em pt-BR ou en-US, imperativo ("adiciona", "adiciona flag…").
- Separe mudanças de segurança (validação, confirmações) para revisão dedicada.

## Debug

- **Log do app:** `~/.local/share/app.zittodb.desktop/logs/zittodb.log` (limitado a 2 MB).
- **Auditoria:** `~/.config/app.zittodb.desktop/audit.jsonl` (JSONL, 5.000 linhas) — é a fonte da aba Histórico → Reverter.
- **Modo demo no navegador** é o caminho mais rápido para iterar na UI sem Rust: `npm run dev:demo`.
- Para testar sem aparelho real, a suíte de integração Rust usa um `fake-adb` (shell script) em `src-tauri/tests/integration.rs` — veja os testes de segurança (serial malicioso, injeção de argv) lá.

## Performance

- Alvo: 2 GB de RAM. Se o app pesar, use **Configurações → Modo de desempenho**:
  - `low`: menos animações, refresh 15 s, buffer logcat 3.000.
  - `ultra`: zero animações/sombras/backdrop-filter, sem auto-refresh, buffer 1.500.
- O frontend não coleta métricas contínuas; `scripts/measure-performance.sh` faz medições pontuais sob demanda.
