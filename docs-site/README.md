# Site de documentação do ZittoDB

O site é uma página estática sem dependências adicionais, mantida no diretório `docs-site/` do repositório principal. A navegação segue o sistema do Pterodroid: grupos de documentação, rotas `/docs/...`, busca, sumário por página, copiar comandos e navegação anterior/próxima.

## Publicar na Vercel

1. Importe `theeussx/zittodb` na Vercel.
2. Em **Root Directory**, selecione `docs-site`.
3. Use o framework **Other**.
4. Deixe o comando de build vazio.
5. Publique.

A Vercel servirá `index.html` diretamente. Não é necessário instalar dependências nem configurar uma API.

## Desenvolvimento local

A partir da raiz do repositório, rode um servidor estático simples:

```bash
python3 -m http.server 4173 --directory docs-site
```

Abra <http://localhost:4173>.


## Sistema de documentação

As páginas são seções sem dependências externas dentro de `index.html`; `app.js` monta a navegação por rota, o índice de busca, o sumário e os controles de código. O `vercel.json` reescreve `/docs/*` para a página principal, como no site de referência.

Também existem páginas espelho em `docs/` para que links como `/docs/download` continuem funcionando mesmo quando a configuração de rewrites da Vercel ainda não estiver ativa no projeto.
