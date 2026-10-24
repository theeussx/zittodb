## [1.0.0] - 2026-10-09

### Corrigido

- A gravação do scrcpy e o armazenamento dos arquivos de mídia.
- A seleção explícita do dispositivo e a consulta de variáveis no Fastboot.
- A proteção de pacotes críticos e a desinstalação no Debloat.

## [1.1.0] - 2026-10-24

### Adicionado

- Relatório diagnóstico P0 com schemaVersion estável, coleta parcial por seção, mascaramento conservador de serial/rede e preview na tela de Diagnóstico.
- Exportação backend em JSON e Markdown no diretório seguro de downloads, sem sobrescrita silenciosa e com nomes incrementais.
- Scanner Center com verificações somente leitura do ambiente (ADB, Fastboot e scrcpy) ou de um dispositivo Android selecionado, evidências por achado e limitações explícitas.
- Histórico local de varreduras, comparação entre relatórios, filtros por severidade/categoria e exportação em JSON ou Markdown.
- Classificação de risco no Shell interativo, com indicação de comandos de leitura, alteração e destrutivos.

### Segurança

- Comandos destrutivos no Shell exigem a confirmação digitada `EXECUTAR`; a validação também é aplicada no backend, que limita a entrada a uma linha.
- Exclusão e renomeação no dispositivo bloqueiam caminhos críticos, mesmo quando há confirmação.

### Melhorado

- Auditoria de transferências de arquivos e extração de APKs registra resultado e nome do arquivo, sem incluir o caminho local completo.

## [0.1.6] - 2026-10-06

### Adicionado

- Recuperação do servidor ADB pela interface, incluindo versão, inicialização e reinício protegido por confirmação.
- Alias, tags e favoritos persistentes para dispositivos no histórico local.
- Validação de identidade e metadados de dispositivos no backend Rust e no modo local/mock.
- Verificação automática de sincronização de versões integrada ao `npm run check`.

### Segurança

- Reiniciar o daemon exige digitar `REINICIAR_ADB` e informa que sessões ADB locais serão interrompidas.
- Metadados locais recebem limites de tamanho e continuam fora de qualquer serviço remoto.

## [0.1.5] - 2026-10-05

### Adicionado

- Atualização automática do aplicativo via Tauri Updater.
- Releases assinadas no GitHub.
- Pipeline de CI para frontend e Rust.
- Geração automática de AppImage e pacote `.deb`.

### Corrigido

- Dependências Linux ausentes no job Rust do CI.
- Versões do frontend, backend e Tauri alinhadas.
- Publicação do manifesto `latest.json`.
- Fixture `fake-adb` dos testes de integração agora usa diretórios temporários únicos, evitando falhas paralelas de `Text file busy`.
