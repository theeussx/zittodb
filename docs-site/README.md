# Site de documentação do ZittoDB

O site é uma página estática sem dependências adicionais, mantida no diretório `docs-site/` do repositório principal.

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
