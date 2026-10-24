# Referências consultadas e decisões para o ZittoDB

Consulta feita em 23/10/2026. Só entram aqui fontes efetivamente abertas ou retornadas por busca; onde a fonte foi um espelho/fork do README oficial ou um site de terceiros, isso está indicado. "A validar" = ainda precisa ser testado em implementação.

## Fontes e o que ensinaram

| Fonte | O que foi verificado | Aplicação no ZittoDB |
|---|---|---|
| [ADB (oficial)](https://developer.android.com/tools/adb) — lida na íntegra | Arquitetura cliente/servidor/daemon (porta 5037); estados `offline`/`device`/`no device`; `adb devices -l`; `adb server-status` (versão, mDNS); `adb mdns track-services`; Wi‑Fi debugging (Android 11+); desde Platform Tools 23 o adb trata argumentos como `ssh` (comandos com metacaracteres exigem aspas duplas); `am monitor` (crashes/ANRs); `pm list permissions -d` (permissões perigosas) | Scanner de ambiente usa `adb version`, `devices -l` e `server-status`; confirma a escolha atual de enviar UMA string de comando a `adb shell` com `quote_shell`; base para detecção de ANR/crash |
| [Logcat CLI (oficial)](https://developer.android.com/tools/logcat) — via busca | Buffers circulares do `logd` (`crash`, `main`, `system`, `all`…); formatos de saída (`threadtime` é o padrão); filtros `TAG:PRIORIDADE`; várias opções só funcionam como root | Logcat avançado: seleção de buffer, parsing de `threadtime` no backend, aviso quando uma opção exige root |
| [scrcpy (oficial)](https://github.com/Genymobile/scrcpy) — via busca (README também em forks) | Requer Android 5.0 (API 21); áudio só Android 11+; câmera Android 12+; modo OTG não precisa de depuração USB; nada fica instalado no aparelho | Scanner de dispositivo deve avisar "áudio indisponível" por versão de Android, em vez de oferecer e falhar |
| [JADX](https://github.com/skylot/jadx) — via espelhos do README | Decompila APK/DEX/AAR/AAB para Java e decodifica `AndroidManifest.xml`/`resources.arsc`; CLI com `--version`; **não decompila 100%**; licença Apache 2.0 | Integração **externa opcional**: detectar `jadx --version` e abrir a saída; nunca embutir |
| [Apktool](https://github.com/iBotPeaches/Apktool) — via forks e site de terceiros (appsecsanta) | Decodifica recursos e smali e permite rebuild; Apache 2.0 (segundo terceiros); README diz que não se destina a pirataria | Integração externa opcional somente para **decodificar/inspecionar**. Rebuild/patch fica fora do escopo (risco de uso para burlar licenças) |
| [MobSF](https://github.com/MobSF/Mobile-Security-Framework-MobSF) — repositório oficial via busca; licença por terceiros | Análise estática/dinâmica de APK/IPA/APPX, REST API, uso típico via Docker; **GPL-3.0** (fonte de terceiros) | Não embutir (a licença contaminaria o projeto). Possível integração opcional via REST contra uma instância local do usuário; prioridade baixa |
| [Tauri 2 — Capabilities](https://v2.tauri.app/security/capabilities/) | Capabilities mapeiam permissões a janelas; a API só é acessível a código empacotado; a doc lista "verificações de escopo incorretas na implementação do comando" como fronteira de risco | Reforça validar tudo no backend. Ver recomendações abaixo |
| Ferramentas do SDK (listadas na navegação da doc oficial do ADB): `aapt2`, `apkanalyzer`, `apksigner` | **Apenas a existência** foi confirmada nesta consulta | Caminho preferido para inspeção de APK: invocar essas ferramentas do SDK quando presentes (a validar: formato exato das saídas) |

## Decisões

1. **Nativo primeiro:** hash SHA-256 de APK, listagem de permissões declaradas via `pm`/`dumpsys` no aparelho, diagnóstico de ambiente e parsing de logcat são implementados em Rust, sem dependências pesadas.
2. **SDK tools antes de reimplementar:** metadados de APK e assinatura via `aapt2`/`apksigner`, se instalados; se ausentes, a UI diz "indisponível: instale as build-tools" em vez de inventar dados.
3. **JADX/Apktool/MobSF:** só integrações externas opcionais e desligadas por padrão; nenhuma é dependência obrigatória.
4. **Fora do escopo por política:** rebuild/patch de APK, bypass de licença, extração de credenciais.

## Comparativo

| Ferramenta | Pontos fortes | Oportunidade para o ZittoDB | Complexidade |
|---|---|---|---|
| Android Studio (Logcat/Profiler) | Logcat com filtros ricos, perfis de CPU/memória | Logcat com filtro por PID/nível/buffer e captura antes/depois de uma operação | Média |
| scrcpy | Espelhamento, gravação, câmera, OTG | Já integrado; adicionar checagem de requisitos por versão | Baixa |
| JADX | Leitura de código decompilado | Integração opcional "abrir no JADX" | Baixa |
| Apktool | Decodificação de recursos/smali | Inspeção opcional (sem rebuild) | Baixa |
| MobSF | Relatório de segurança completo | Integração REST opcional | Média/Alta |
| `aapt2`/`apksigner`/`apkanalyzer` | Metadados e assinatura oficiais | Base da inspeção de APK | Média |

## Recomendações de hardening do Tauri (ainda NÃO implementadas)

- **P2:** `src-tauri/build.rs` usa o `tauri_build::build()` padrão, sem `AppManifest`. Definir a lista de comandos do app (permissões por comando) evita que um comando novo fique exposto por omissão. A validação no backend continua sendo a barreira real. (A validar contra a doc de permissões do Tauri antes de implementar.)
- **P2:** `process:allow-restart` está concedido ao webview; avaliar se é necessário.
- **P3:** separar a CSP de desenvolvimento (`devCsp`) para que `localhost:1420` não conste no build de produção.
