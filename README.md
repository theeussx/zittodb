<p align="center">
  <img src="docs/assets/zittodb-logo.svg" alt="ZittoDB — Android Debug Bridge, direto e local" width="520" />
</p>

<p align="center">
  <a href="https://github.com/theeussx/zittodb/releases"><img alt="Release" src="https://img.shields.io/badge/release-v1.0.0-3ddc84?style=flat-square&labelColor=0e1116" /></a>
  <a href="LICENSE"><img alt="Licença MIT" src="https://img.shields.io/badge/license-MIT-2aa862?style=flat-square&labelColor=0e1116" /></a>
  <img alt="Linux" src="https://img.shields.io/badge/platform-Linux%20(amd64)-4da3ff?style=flat-square&labelColor=0e1116" />
  <img alt="Tauri 2 · Rust · React" src="https://img.shields.io/badge/stack-Tauri%202%20%C2%B7%20Rust%20%C2%B7%20React-f5a623?style=flat-square&labelColor=0e1116" />
  <img alt="Local-first" src="https://img.shields.io/badge/local--first-sem%20telemetria-2aa862?style=flat-square&labelColor=0e1116" />
</p>

<h3 align="center">Interface gráfica leve e local-first para <strong>ADB</strong>, <strong>scrcpy</strong> e <strong>fastboot</strong> no Linux.</h3>
<p align="center"><em>Android Debug Bridge, direto e local.</em></p>

<p align="center">
  Dispositivos, shell, screenshots, espelhamento, aplicativos, arquivos, logcat,
  diagnóstico, auditoria e histórico — <strong>sem nuvem, sem conta e sem telemetria</strong>.
</p>

<p align="center">
  <a href="https://github.com/theeussx/zittodb/releases">Baixar release</a> ·
  <a href="docs-site/">Ler a documentação</a> ·
  <a href="docs/ARCHITECTURE.md">Arquitetura</a> ·
  <a href="docs/SECURITY.md">Segurança</a>
</p>

> **ZittoDB não é um banco de dados.** O nome é Zitto + `db` de **D**ebug **B**ridge: uma GUI para as ferramentas de desenvolvimento Android que você já usa.

## O que é

O ZittoDB é uma camada visual para o Android. O `adb`, o `scrcpy` e o `fastboot` continuam sendo as ferramentas que fazem o trabalho; o ZittoDB constrói o `argv`, valida os parâmetros, mostra o resultado e registra as operações quando aplicável.

```text
Você clica → a UI envia uma operação tipada → Rust valida e monta o argv
           → adb/scrcpy/fastboot executa → o resultado volta para a interface
           → a ação entra na auditoria, com reversão quando fizer sentido
```

### Por que usar

- **Local-first:** não há conta, servidor, banco remoto ou telemetria.
- **Leve:** Tauri 2 + Rust, sem Electron e sem ADB embutido.
- **Explícito:** o dispositivo é identificado pelo serial; a interface não escolhe um alvo implícito.
- **Seguro por fronteiras:** o frontend não executa processos; o backend Rust controla allowlist, validação, confirmação e ciclo de vida.
- **Compatível com seu ambiente:** usa `adb`, `scrcpy` e `fastboot` instalados no sistema.

## Recursos

| Área | Recursos |
|---|---|
| **Dispositivos** | Descoberta via `adb devices -l`, USB, Wi-Fi e emuladores; estados `device`, `offline`, `unauthorized` e `recovery`. |
| **Visão geral** | Propriedades do aparelho, bateria, armazenamento, rede, memória e patch de segurança sob demanda. |
| **Tela** | Integração com scrcpy, presets de qualidade, orientação, áudio, janela no topo e gravação MP4. |
| **Aplicativos** | Lista de pacotes, abrir, ativar/desativar, limpar dados, desinstalar por usuário, instalar e extrair APK. |
| **Debloat** | Classificação heurística de risco, perfis conservador→avançado, operação em lote e reversão pelo Histórico. |
| **Arquivos** | Navegação em `/sdcard`, mkdir, renomear, apagar, push e pull com progresso. |
| **Shell** | Sessão persistente por dispositivo, sempre direcionada ao serial selecionado. |
| **Logs** | `logcat -v threadtime`, filtro `TAG:PRIORITY`, pausa, cópia e download. |
| **Fastboot** | Dispositivos, `getvar`, reinícios, flash, erase, lock e unlock com confirmação explícita. |
| **Histórico** | Auditoria JSONL, operações reversíveis e últimos dispositivos vistos. |

A interface é organizada em **Dispositivos**, **Comandos**, **Fastboot**, **Histórico** e **Configurações**. Dentro de um aparelho: **Visão geral**, **Tela**, **Aplicativos**, **Arquivos**, **Shell**, **Logs**, **Diagnóstico** e **Debloat**.

## Instalação para usuários

Baixe o **AppImage** ou o **`.deb`** na página de [Releases](https://github.com/theeussx/zittodb/releases). As releases oficiais incluem artefatos assinados e `latest.json`.

```bash
# AppImage
chmod +x ./*.AppImage
./Zittodb-*.AppImage

# Debian / Ubuntu
sudo apt install ./*.deb
```

### Requisitos do sistema

- Linux, em Wayland ou X11.
- [`adb`](https://developer.android.com/tools/adb) para controlar aparelhos.
- [`scrcpy` 3.2+](https://github.com/Genymobile/scrcpy/releases) para espelhamento e gravação (opcional).
- `fastboot` para a aba Fastboot (opcional).

Em Debian/Ubuntu:

```bash
sudo apt update
sudo apt install adb fastboot scrcpy
```

> Para Android 15, prefira o release oficial do scrcpy: o pacote da distribuição pode estar em uma versão antiga.

## Desenvolvimento local

### Pré-requisitos

- Node **20.19+** ou **22.12+**.
- Rust estável, com `cargo` no `PATH`.
- Dependências de sistema do Tauri 2 em Debian/Ubuntu:

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file \
  libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev
```

### Setup e modos de execução

```bash
git clone https://github.com/theeussx/zittodb.git
cd zittodb
npm install

# Desenvolvimento rápido no navegador, com dados simulados
npm run dev:demo

# Navegador + ADB local real (sem Rust/Tauri)
npm run dev

# Aplicativo desktop completo
npm run tauri:dev
```

O frontend abre em `http://localhost:1420`. O modo demo não executa comandos reais; o modo local usa ADB pelo serviço loopback integrado ao Vite e deixa explícito o que ainda exige o desktop.

### Comandos do projeto

| Comando | Função |
|---|---|
| `npm run dev` | Frontend + serviço ADB local. |
| `npm run dev:demo` | Demonstração no navegador, sem dispositivo. |
| `npm run build` | TypeScript + build de produção. |
| `npm run preview` | Preview do build estático. |
| `npm run tauri:dev` | Executa o app desktop em desenvolvimento. |
| `npm run tauri:build` | Gera AppImage e `.deb`. |
| `npm test` | Testes frontend com Vitest. |
| `npm run check:version` | Confere a sincronização da versão entre pacote, Tauri, Rust e README. |
| `make test-rust` | Testes backend com Cargo. |
| `npm run check` | Build e testes principais. |
| `npm run check:deps` | Verifica `adb`, `scrcpy` e `fastboot`. |
| `npm run icons` | Regenera os ícones da marca. |

## Atalhos

| Atalho | Ação |
|---|---|
| `Ctrl+Shift+D` | Dispositivos |
| `Ctrl+Shift+A` | Shell |
| `Ctrl+Shift+S` | Screenshot |
| `Ctrl+Shift+F` | Arquivos |
| `Ctrl+Shift+L` | Logs |

## Arquitetura

```text
React + TypeScript
      │  Bridge (Tauri · local · mock)
      ▼
Tauri commands + operações tipadas (Rust)
      │  validação · allowlist · risco · serial
      ▼
ProcessRegistry + storage local
      │
      ├── adb
      ├── scrcpy
      └── fastboot
```

O frontend conhece o contrato `Bridge`, não os detalhes de execução. O backend Rust:

1. resolve e valida o serial do dispositivo;
2. transforma a intenção em uma operação permitida;
3. monta um vetor de argumentos, sem `sh -c`;
4. serializa operações conflitantes por aparelho;
5. controla timeout, cancelamento e limpeza de processos;
6. grava auditoria local quando a ação suporta histórico ou reversão.

Leia a [documentação de arquitetura](docs/ARCHITECTURE.md) para detalhes de camadas, eventos, timeouts, storage e contrato de erro.

## Segurança e privacidade

- Nenhum processo é executado pelo frontend.
- Operações destrutivas exigem confirmação digitada: `APAGAR`, `REMOVER`, `REINICIAR` ou `FLASHAR`.
- O backend valida serial, caminho, pacote, partição e demais argumentos.
- Processos são encerrados com `SIGTERM`, espera de até 3 s e `SIGKILL` quando necessário.
- Configurações, logs e auditoria ficam no filesystem local.
- A única conexão externa do desktop é a consulta de metadados públicos do último release no GitHub para avisar sobre atualizações.

Veja o [modelo de ameaças](docs/SECURITY.md).

## Estrutura do repositório

```text
src/                 frontend React, features e serviços Bridge
src-tauri/           backend Rust, comandos, segurança e storage
tests/               testes frontend
server/              serviço ADB local integrado ao Vite
docs/                arquitetura, desenvolvimento, segurança e release
docs-site/           site público de documentação
scripts/             verificação de dependências e performance
```

## Documentação

- **[Site de documentação](docs-site/)** — guia de usuário com instalação, primeiros passos, recursos e segurança.
- **[Arquitetura](docs/ARCHITECTURE.md)** — fronteiras, operações tipadas, processos e persistência.
- **[Desenvolvimento](docs/DEVELOPMENT.md)** — toolchain, debug, performance e release.
- **[Segurança](docs/SECURITY.md)** — modelo de ameaças e decisões de proteção.
- **[Contribuição](docs/CONTRIBUTING.md)** — padrões para issues e pull requests.
- **[Releasing](docs/RELEASING.md)** — publicação de AppImage, `.deb` e metadata de atualização.
- **[Changelog](CHANGELOG.md)** — histórico de mudanças.

### Publicar o site na Vercel

No projeto da Vercel, importe este repositório e defina **Root Directory** como `docs-site`. Escolha **Other**, deixe o build vazio e publique. O site é estático e não precisa de dependências ou variáveis de ambiente. O passo a passo também está em [`docs-site/README.md`](docs-site/README.md).

## Roadmap

O projeto está na série **v0.1**. O estado atual cobre detecção de ferramentas, dispositivos, informações, shell, screenshots, scrcpy, APKs, arquivos, logcat, diagnóstico, auditoria, histórico, Fastboot e documentação. Novos itens devem preservar os princípios local-first e de execução controlada.

## Licença

Distribuído sob a [licença MIT](LICENSE).
