# Releases e atualização automática

O Zittodb não precisa ser recompilado no computador do usuário. O aplicativo instalado verifica o endpoint assinado do GitHub Releases e oferece a instalação da nova versão dentro do próprio app.

## Publicar uma versão

1. Atualize a versão em `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock` e `src-tauri/tauri.conf.json`; atualize também o badge no README.
2. Registre as alterações e a data no `CHANGELOG.md`.
3. Rode `npm ci && npm run check`.
4. Crie e envie a tag semântica correspondente à versão:

```bash
git tag v1.0.0
git push origin v1.0.0
```

O workflow `Release` compila o AppImage e o `.deb`, cria as assinaturas e publica `latest.json` no release.

## Gerar a chave do updater

Gere o par no seu ambiente de desenvolvimento e não compartilhe a chave privada:

```bash
npx tauri signer generate -w ~/.config/zittodb/updater.key
cat ~/.config/zittodb/updater.key.pub
```

Substitua `REPLACE_WITH_TAURI_UPDATER_PUBLIC_KEY` em `src-tauri/tauri.conf.json` pelo conteúdo da chave pública. Guarde a chave privada em um gerenciador de segredos.

## Segredos do GitHub

Configure no repositório:

- `TAURI_SIGNING_PRIVATE_KEY`: conteúdo da chave privada do updater.
- `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`: senha da chave, se houver.

A chave privada nunca deve entrar no Git. A chave pública fica em `src-tauri/tauri.conf.json` e permite que o app rejeite artefatos adulterados.

## Atualização no aplicativo

No desktop, o Zittodb verifica a versão no endpoint oficial do updater Tauri. Quando há uma versão nova, o usuário vê o banner e escolhe **Instalar agora**. O download é validado pela assinatura antes da instalação e o app é reiniciado pelo plugin oficial.

No navegador/demo, a checagem continua sendo apenas informativa, porque o navegador não pode instalar um binário do desktop.

## Rollback

Não reutilize uma chave de assinatura. Para corrigir uma release, publique uma nova versão sem apagar a chave atual. Perder a chave privada impede todas as atualizações futuras para instalações existentes.
