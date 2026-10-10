# Site do ZittoDB

O site é uma experiência estática sem dependências adicionais, mantida em `docs-site/`.

- `/` — landing page do produto, com a apresentação visual do ZittoDB, recursos, fluxo de uso e CTAs.
- `/docs` — documentação completa com navegação por páginas, busca, sumário, copiar comandos e navegação anterior/próxima.

A identidade visual combina azul profundo, ciano elétrico e cartões técnicos inspirados na interface do aplicativo. A imagem `assets/BannerfuturistadoZittoDBparaGitHub.png` é a referência visual central da landing.

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
python3 -m http.server 4173 --directory docs-site
```

Abra <http://localhost:4173> ou <http://localhost:4173/docs>.
