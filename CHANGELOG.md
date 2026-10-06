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
