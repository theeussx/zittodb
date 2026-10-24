# Segurança

Modelo de ameaças e controles do Zittodb. A premissa central: **a UI é confiável o suficiente para exibir dados, mas nunca é confiável para executar comandos**.

## 1. Execução de processos

| Ameaça | Controle |
|---|---|
| Injeção de comando via valores do usuário (serial, path, pacote…) | Cada valor passa por validação por regex em `security/` **antes** de virar argv; aspas só são colocadas no `describe()` para exibição, nunca interpretadas por shell. |
| `sh -c "…"` com string montada | **Não existe.** `std::process::Command` recebe um `Vec<String>`; não há flag `shell: true` em lugar nenhum do código. |
| Alvo errado do comando | Toda operação de dispositivo carrega `-s SERIAL`; serial é validado (`validate_serial`); quando há mais de um dispositivo e a serial não é explícita → `AMBIGUOUS_DEVICE` (o app nunca "chuta" o dispositivo). |
| Injeção via serial (`$(rm -rf ~)`) | `validate_serial` restringe o charset; o serial vem do parse de `adb devices -l`, e a operação exige o mesmo serial do parse. |
| Argumentos com `-` (opção de flag) | Validadores rejeitam tokens iniciando com `-` onde o valor é um caminho/pacote (ex.: `getvar -v` rejeitado, testado). |
| Processo órfão travando o aparelho | `ProcessRegistry` com `Drop`; `SIGTERM` → 3 s → `SIGKILL`; `on_exit` mata tudo; operações com timeout explícito por tipo. |

## 2. Autorização do ADB (RSA)

- O app **nunca** burla a autorização: não importa `adbkey` por conta própria, não usa `adb root`, não tenta `unauthorized` em loop.
- Dispositivo `unauthorized` é listado com estado e **dica** (aceite a caixinha no aparelho). Operações contra ele falham com `DEVICE_UNAUTHORIZED`.
- Se o aparelho exige root (`adb root`) para alguma ação, o app mostra a falha do comando — não tenta elevar privilégio.

## 3. Confirmações destrutivas

Operações destrutivas exigem **digitar a palavra exata** (maiúsculas, sem tolerância). Um clique nunca é suficiente; o botão fica desabilitado até o texto bater:

| Operação | Palavra |
|---|---|
| Apagar arquivo remoto | `APAGAR` |
| Limpar dados do pacote | `APAGAR` |
| Desinstalar (usuário 0) | `REMOVER` |
| Reboot (qualquer alvo) | `REINICIAR` |
| fastboot flash | `FLASHAR` |
| fastboot erase / unlock / lock | `APAGAR` |

Desativar pacote **não** é destrutiva (é reversível e o app oferece Reverter no Histórico) — segue a política de risco, não a de confirmação digitada. Lotes de debloat usam **uma única** confirmação para o conjunto. Instalar APK (`install -r`) também não exige palavra: é reversível via desinstalação por usuário e o caminho local é validado (`validate_local_path`).

## 3.1. Shell: classificação de risco (heurística)

`shell_write` recebe **uma linha** (sem `\n`; o backend acrescenta o terminador e rejeita quebras de linha, bytes de controle e linhas > 10.000 bytes). Antes de enviá-la ao stdin, `security::shell_risk` classifica a linha:

| Risco | Exemplos | Efeito |
|---|---|---|
| `read` | `ls`, `getprop`, `dumpsys`, `pm list`, `settings get` | executa |
| `modify` | `settings put`, `pm disable-user`, `am start`, `input`, `mv`, redirecionamentos `>` | executa e é auditado |
| `destructive` | `rm`, `reboot`, `dd`, `pm uninstall/clear`, `find -delete`, `mount …` e **qualquer wrapper opaco** (`sh -c`, `su`, `eval`, `$(…)`, aspas desbalanceadas) | exige digitar `EXECUTAR`; sem a palavra o backend responde `CONFIRMATION_REQUIRED` |

Regras: comando desconhecido é `modify`, nunca `read`; o segmento mais arriscado de uma linha composta (`;`, `&&`, `||`, `|`) decide.

**Limite explícito:** isto é uma proteção contra acidentes, **não uma fronteira de segurança**. Um shell sempre pode esconder a intenção (variáveis, scripts já presentes no aparelho). A fronteira real são as permissões do usuário `shell` do ADB. O texto dos comandos continua **fora** da auditoria (pode conter segredos); linhas `modify`/`destructive` geram uma entrada `shell_command` com apenas o risco e os códigos de motivo.

## 3.2. Scanner Center

- Somente leitura: usa apenas operações da allowlist (`GetProps`, `GetBattery`, `GetDiskUsage`, `GetSelinuxMode`) e probes do próprio adb (`version`, `server-status`). Nenhuma varredura altera o computador ou o aparelho.
- O alvo é sempre explícito: varredura de dispositivo exige serial; um serial que não está listado devolve `NO_DEVICE` (nunca é tratado como "offline" nem redirecionado a outro aparelho).
- Tudo o que o aparelho informa é o que ele diz de si mesmo; um sistema comprometido pode mentir (limitação registrada em cada relatório).
- Histórico em `<data>/scans/` com permissão 0600; ids são validados (`scan-<unix>-<4 hex>`) antes de virarem caminho.

## 4. Debloat e risco

- Risco é calculado por heurística (`security/risk.rs`): `com.android.providers.*` → **Crítico**; `com.miui.*`/`com.xiaomi.*` → **Baixo**; GMS/GSF → **Atenção**; pacotes de sistema genéricos → **Perigoso**; **desconhecido nunca é "Seguro"** (`UNKNOWN`).
- O app **nunca afirma que um pacote é seguro por nome** — a UI mostra o risco e deixa a decisão (e a reversibilidade) com o usuário.
- Perfis (conservador → avançado) só **incluem** pacotes até um teto de risco; o usuário marca/desmarca à vontade.

## 5. Arquivos e mídia

- **Locais protegidos no dispositivo**: `Delete` (`rm -rf`) e a origem de `Rename` (`mv`) rejeitam com `OPERATION_REJECTED` as raízes críticas (`/`, `/sdcard`, `/storage/emulated/0`, `/data`, `/data/data`, `/system`, `/vendor`, `/sdcard/Android`, etc.) e qualquer coisa sob `/proc`, `/sys`, `/dev`, mesmo com a palavra de confirmação. O conteúdo dessas pastas continua removível. A checagem normaliza `//`, `/./` e `/` final e vive em `security::validate_mutable_device_path`, ou seja, vale no backend independentemente da UI.
- **Exportação de texto (`save_log_file`)**: só cria arquivo NOVO (`create_new`/O_EXCL: nunca sobrescreve nem segue symlink), exige caminho absoluto, diretório-pai existente, extensão em `log, txt, md, json, html` e no máximo 64 MiB.

- **Screenshots** auto-nomeados `screenshot-YYYY-MM-DD-HHMMSS.png`; se o nome já existe, erro `FILE_EXISTS` e a UI pergunta — com `force`, usa sufixo `-1`..`-N`, nunca sobrescreve silenciosamente.
- **Gravação scrcpy**: `--record <caminho>` validado; se o arquivo existe, sufixo `-1`..`-99`.
- **Pull/push**: caminhos de dispositivo validados por `validate_device_path` (absolutos, sem `..`, sem bytes de controle); caminhos locais por `validate_local_path` (que também recusa valores iniciados por `-`, para nunca virarem opção do adb); a UI parte de `/sdcard`. Push, pull e extração de APK geram entrada de auditoria no fim (`ok` ou `error:<CÓDIGO>`), registrando só o nome do arquivo local, não o diretório. O caminho do APK informado pelo aparelho em `package_extract` também é validado.
- **Salvamento de logcat**: nome do arquivo validado (sem `/` embutido além da pasta de destino).

## 6. Superfície de rede e dados

- **Zero** de telemetria/analytics e nenhum update automático. Ao abrir o app, ele pode consultar somente os metadados públicos do último release no GitHub para avisar sobre uma nova versão; nenhum arquivo é baixado ou instalado.
- Sem servidor local, sem DB, sem arquivos temporários compartilhados.
- Persistência: `~/.config/app.zittodb.desktop/settings.json` (atômica), `audit.jsonl` (5.000 linhas), histórico de dispositivos (metadados), log do app (2 MB). Nada de token, cookie ou segredo.

## 7. Tauri / webview

- Sem plugin de shell (não há como o frontend executar algo direto do sistema).
- CSP restritiva definida em `tauri.conf.json` (`default-src 'self'`, `script-src 'self'`, sem `eval`); o `connect-src` inclui `localhost:1420` (porta do Vite) **também nos builds de produção**, porque há uma única CSP; separar via `devCsp` está listado como melhoria em `docs/REFERENCES.md`. Em produção a webview carrega arquivos locais (`frontendDist`).
- `freezePrototype: true` (hardening do prototype do webview).
- Diálogo de arquivos usa `tauri-plugin-dialog` (native), não `<input type=file>` com upload.

## 8. Fastboot

- Flash/erase/unlock/lock só com confirmação digitada **e** partição/arquivo validados (charset de partição, caminho local).
- A UI avisa explicitamente que flash incorreto pode deixar o aparelho sem boot.

## Limitações conhecidas

- O `adb` do sistema continua sujeito a seus próprios bugs; o app não corrige o binário.
- O modelo de risco é heurística: **não** garante que um pacote classificado como "Baixo" seja inofensivo em todos os aparelhos.
- Em modo demo (navegador), nenhuma operação real é executada — é o mesmo contrato, com resultados simulados.

## Serviço de desenvolvimento no navegador

`npm run dev` inclui uma API Node no próprio Vite. Não é um serviço de produção.
O serviço exige Host loopback, Origin correspondente, POST JSON e cabeçalho
`X-Zittodb-Client: local`; não habilita CORS. Isso bloqueia chamadas de sites externos
usuais e DNS rebinding, mas não protege contra código malicioso executado na própria
origem, extensões privilegiadas ou usuários/processos locais. Não exponha o Vite
por túnel nem use proxy que reescreva esses cabeçalhos.

Os comandos são selecionados por allowlist e executados por `execFile`/`spawn`,
sem shell no computador. Seriais e filtros de logcat são validados. O terminal é
uma exceção intencional: o usuário envia comandos arbitrários ao shell **Android**
selecionado, com os privilégios concedidos pelo ADB. A autorização RSA continua
obrigatória. Os logs ficam em memória limitada; o download usa o navegador, não
uma escrita arbitrária em caminhos do computador. Sessões inativas expiram.
O modo demo não registra essa API. Operações ainda não implementadas retornam
`UNSUPPORTED_LOCAL`, sem fallback para dados simulados.
