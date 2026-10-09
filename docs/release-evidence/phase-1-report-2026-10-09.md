# Fase 1 — Baseline e estabilização

**Projeto:** `theeussx/zittodb`  
**Data:** 2026-10-09  
**Branch:** `feat/v2.0`  
**Base:** `734aacd` (`v0.1.6`, `origin/main`)  
**Escopo executado:** somente auditoria/baseline; nenhum recurso P0/P1 foi implementado.

## Estado do checkout

- Checkout confirmado no repositório `theeussx/zittodb`.
- Branch `feat/v2.0` criada localmente a partir de `main` limpa.
- Não havia alterações locais antes da criação da branch.
- Ao final, as únicas alterações são os artefatos de evidência desta pasta.
- Nenhuma tag, release, secret ou branch remota foi alterada.

## Evidências de comandos

A saída bruta está em:

- `docs/release-evidence/phase-1-baseline-2026-10-09.log`
- `docs/release-evidence/phase-1-status-2026-10-09.log` — inclui códigos de saída confiáveis

| Comando/artefato | Resultado | Observação |
|---|---|---|
| `git status --short --branch` | **PASSOU** (0) | `feat/v2.0`; checkout limpo antes dos artefatos de evidência |
| `node --version` | **PASSOU** (0) | `v22.13.0` |
| `npm --version` | **PASSOU** (0) | `10.9.2` |
| `rustc --version` | **NÃO VERIFICADO** (127) | `rustc` não encontrado no PATH |
| `cargo --version` | **NÃO VERIFICADO** (127) | `cargo` não encontrado no PATH |
| `npm ci` | **PASSOU** (0) | 184 pacotes adicionados; aviso de pacote deprecated `whatwg-encoding@3.1.1` |
| `npm run check` | **PASSOU** (0) | versão sincronizada `v0.1.6`; build Vite passou; 7 arquivos de teste e 49 testes passaram |
| `make check` | **PASSOU** (0) | TypeScript e 49 testes passaram; testes Rust foram pulados por ausência de Cargo |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | **NÃO VERIFICADO** (127) | Cargo indisponível |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features -- -D warnings` | **NÃO VERIFICADO** (127) | Cargo indisponível |
| `npm audit --audit-level=high` | **FALHOU** (1) | 1 vulnerabilidade alta em `source-map-js` 1.0.0–1.2.1; correção sugerida pelo npm |
| `npm run check:deps` | **FALHOU** (1) | `adb`, `scrcpy` e `fastboot` ausentes |
| `bash scripts/measure-performance.sh` | **FALHOU** (1) | não mediu: `adb` ausente no PATH |

## Divergências e observações verificadas

1. O código está alinhado em `v0.1.6` nas fontes checadas pelo script (`package.json`, `Cargo.toml`, `tauri.conf.json` e badge do README).
2. O README ainda apresenta o roadmap como série `v0.1`, o que é esperado na base atual e deverá ser atualizado somente na fase de release.
3. A CI executa build/testes separados, mas não chama `npm run check` nem `npm run check:version` como comando único; isso deve ser avaliado no hardening de CI, sem fazer parte do baseline.
4. A fronteira existente é `src/services/bridge.ts`; `TauriBridge`, `LocalBridge` e `MockBridge` são os pontos obrigatórios para qualquer contrato novo.
5. `DiagnosticsView.tsx` atualmente coleta cinco operações e exibe texto bruto; ainda não há exportação de relatório.
6. `Settings.downloadDir` já existe e a persistência local possui diretórios privados; a escrita do P0 deverá permanecer no backend Rust.
7. O modo local já retorna `UNSUPPORTED_LOCAL` para operações não implementadas; o P0 não deve introduzir download/escrita local no navegador sem decisão deliberada.
8. A validação atual de `InstallApk` é apenas a validação genérica de caminho; nenhuma alteração foi feita nesta fase.

## Critérios de aceite P0

O P0 será considerado entregue somente quando todos os itens abaixo tiverem teste e evidência:

### Contrato e coleta

- [ ] Relatório referente ao serial selecionado, com serial explicitamente revalidado antes da coleta e conforme estratégia documentada durante coleta longa.
- [ ] Seções independentes para propriedades, memória, armazenamento, bateria, rede, estado ADB, versões de `adb`/`scrcpy`/`fastboot`, versão do ZittoDB e metadados.
- [ ] Cada seção contém status (`ok`/indisponível/erro normalizado), dados quando disponíveis e erro localizado sem inventar valores.
- [ ] Falha de uma seção não interrompe as demais.
- [ ] Dispositivo completo, parcial e offline/ausente cobertos pelo MockBridge e/ou fake ADB.

### Schema e privacidade

- [ ] JSON estável com `schemaVersion` explícito e documentado.
- [ ] Markdown e JSON gerados a partir de representação comum, sem duplicar regras de coleta.
- [ ] Serial e dados de rede ocultos por padrão para compartilhamento; opção explícita para revelar.
- [ ] Exportações não incluem serial, IP, MAC ou DNS por padrão.
- [ ] Relatório registra timestamp, versão do app, ferramentas detectadas e limitações sem segredos.

### Escrita segura

- [ ] Escrita realizada somente pelo backend Rust.
- [ ] `downloadDir` usado apenas quando validado; fallback para diretório seguro do app.
- [ ] Diretório inexistente, caminho não regular e ausência de permissão geram erro acionável.
- [ ] Colisão de nome nunca sobrescreve silenciosamente; comportamento incremental existente é reutilizado ou documentado.
- [ ] Nenhuma escrita é adicionada ao modo local do navegador sem implementação segura deliberada.

### Testes e documentação

- [ ] Unitários Rust para masking, normalização, schema, serialização, formatadores e nomes incrementais.
- [ ] Integração Rust/fake ADB para serial explícito, indisponibilidade e falha parcial.
- [ ] Testes Vitest com MockBridge para sucesso, parcial, falha, mascaramento, colisão e estados de carregamento.
- [ ] Chaves novas de i18n presentes em `pt-BR` e `en-US`.
- [ ] README, arquitetura, segurança, desenvolvimento, release e changelog atualizados quando afetados.

## Proposta curta de implementação P0

1. **Contrato puro:** adicionar tipos Rust/TypeScript para `DiagnosticReport`, `ReportSection`, `schemaVersion`, opções de mascaramento e resultado de exportação.
2. **Coleta:** criar um orquestrador Rust que reutilize as operações/serviços existentes, fixe e revalide o serial e capture falhas por seção sem abortar o conjunto.
3. **Formatação:** implementar serialização determinística para JSON e Markdown a partir do mesmo DTO; mascarar identificadores na última etapa antes da formatação.
4. **Escrita:** adicionar comando Tauri de exportação com validação de `downloadDir`, nomes incrementais e erros normalizados; manter o caminho local como `UNSUPPORTED_LOCAL` até existir suporte seguro específico.
5. **Bridge/UI:** refletir o contrato em `Bridge`, `TauriBridge`, `MockBridge` e na tela de diagnósticos; adicionar revisão pré-exportação, seleção de privacidade, loading, estados parcial/offline e download.
6. **Testes/documentação:** implementar a matriz de aceite acima, atualizar i18n/docs e executar a suíte disponível; instalar Rust/ferramentas Android em ambiente apropriado antes de declarar a validação completa.

## Decisões pendentes que podem alterar o contrato

- Nenhuma decisão é necessária para iniciar o desenho mínimo do P0.
- Antes de implementar a escrita, o mantenedor deverá aceitar o formato exato de nome de arquivo/extensão e o diretório fallback, caso o comportamento incremental existente não seja suficiente.
- A resolução da vulnerabilidade alta de `source-map-js` deve ser feita antes do release; não foi aplicado `npm audit fix` automaticamente para evitar alteração de dependências sem revisão.

## Próximo portão

A Fase 1 está encerrada. O próximo passo autorizado pelo prompt é obter aprovação do plano P0 e, em um ambiente com Rust/Cargo disponível, iniciar o contrato/DTO do P0. Não implementar inspetor de APK nem fila nesta etapa.
