# Site do ZittoDB

Site estático mantido em `docs-site/`, sem dependências próprias de frontend.

- `/` — landing oficial do ZittoDB, com hero, captura real do aplicativo, recursos documentados, arquitetura, segurança, instalação e comunidade.
- `/docs` — documentação completa com navegação por páginas, busca, sumário, copiar comandos e navegação anterior/próxima.

A landing usa o logo oficial do projeto e exibe a captura real enviada pelo usuário na área principal do hero. A captura é apenas visual e não se conecta a dispositivos; os links e controles do restante da página apontam para documentação ou releases reais.

## Interações

- Menu mobile abre/fecha e fecha após selecionar uma rota.
- Navegação por âncoras com rolagem suave.
- Captura real do aplicativo exibindo Histórico e auditoria local.
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
