# Site do ZittoDB

Site estático mantido em `docs-site/`, sem dependências próprias de frontend.

- `/` — landing oficial do ZittoDB, com hero, preview demonstrativo reconstruído em HTML/CSS, recursos documentados, arquitetura, segurança, instalação e comunidade.
- `/docs` — documentação completa com navegação por páginas, busca, sumário, copiar comandos e navegação anterior/próxima.

A landing usa o logo oficial do projeto e reconstrói a janela do aplicativo com componentes reais de interface. O preview não se conecta a dispositivos: os dados exibidos são explicitamente demonstrativos. Os controles da prévia trocam painéis localmente, e os links de ação apontam para documentação ou releases reais.

## Interações

- Menu mobile abre/fecha e fecha após selecionar uma rota.
- Navegação por âncoras com rolagem suave.
- Preview com abas de Dispositivos, Comandos, Fastboot e Histórico.
- Ações rápidas para Tela, Aplicativos, Arquivos, Shell e Logs.
- Tabs de instalação AppImage e Debian/Ubuntu.
- Cópia do comando de instalação com feedback no botão.
- Estados de foco para teclado e respeito a `prefers-reduced-motion`.

## Publicar na Vercel

1. Importe `theeussx/zittodb` na Vercel.
2. Em **Root Directory**, selecione `docs-site`.
3. Use o framework **Other**.
4. Deixe o comando de build vazio.
5. Publique.

O `vercel.json` reescreve `/docs/*` para `docs/index.html`. Não é necessário instalar dependências nem configurar uma API.

## Desenvolvimento local

A partir da raiz do repositório:

```bash
python3 -m http.server 4173 --bind 0.0.0.0 --directory docs-site
```

Abra <http://localhost:4173> ou <http://localhost:4173/docs>.
