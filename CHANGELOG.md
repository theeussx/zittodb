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
