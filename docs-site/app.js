const pages = [...document.querySelectorAll('.doc-page')];
const navLinks = [...document.querySelectorAll('[data-page]')];
const breadcrumb = document.querySelector('#breadcrumb');
const sidebar = document.querySelector('#sidebar');
const searchModal = document.querySelector('#search-modal');
const searchInput = document.querySelector('#search-input');
const searchResults = document.querySelector('#search-results');
const toc = document.querySelector('.toc');
const titles = Object.fromEntries(pages.map((page) => [page.dataset.page, page.querySelector('h1')?.textContent || page.dataset.page]));
const groups = { inicio: 'Introdução', download: 'Introdução', instalacao: 'Instalação', 'primeiros-passos': 'Primeiros passos', dispositivos: 'Guias', recursos: 'Guias', seguranca: 'Guias', desenvolvimento: 'Projeto', arquitetura: 'Projeto', contribuindo: 'Projeto' };
const ordered = pages.map((page) => page.dataset.page);

function slugFromPath() {
  const path = window.location.pathname.replace(/\/$/, '');
  if (path.startsWith('/docs/')) return path.slice('/docs/'.length) || 'inicio';
  if (path === '/docs') return 'inicio';
  const hash = window.location.hash.replace('#', '');
  return hash || 'inicio';
}
function routeFor(page) { return page === 'inicio' ? '/docs' : `/docs/${page}`; }
function navigate(page) {
  const target = routeFor(page);
  if (window.location.pathname !== target) window.history.pushState({}, '', target);
  renderPage();
}
function readable(value) { return value.replace(/-/g, ' ').replace(/\b\w/g, (letter) => letter.toUpperCase()); }

function renderToc(activePage) {
  const current = document.querySelector(`[data-page="${activePage}"]`);
  if (!current || !toc) return;
  const headings = [...current.querySelectorAll('.section-heading h2')];
  headings.forEach((heading, index) => { heading.id = `${activePage}-section-${index + 1}`; });
  toc.innerHTML = `<p>NESTA PÁGINA</p>${headings.map((heading) => `<a href="#${heading.id}">${heading.textContent}</a>`).join('')}<div class="toc-divider"></div><a href="https://github.com/theeussx/zittodb/blob/main/docs-site/index.html" target="_blank" rel="noreferrer">Editar no GitHub ↗</a>`;
  toc.querySelectorAll('a[href^="#"]').forEach((link) => link.addEventListener('click', (event) => {
    event.preventDefault();
    document.getElementById(link.getAttribute('href').slice(1))?.scrollIntoView({ behavior: 'smooth' });
  }));
}
function addCopyButtons(activePage) {
  const current = document.querySelector(`[data-page="${activePage}"]`);
  current?.querySelectorAll('pre').forEach((pre) => {
    if (pre.parentElement.classList.contains('code-wrap')) return;
    const wrap = document.createElement('div');
    wrap.className = 'code-wrap';
    pre.parentNode.insertBefore(wrap, pre);
    wrap.appendChild(pre);
    const button = document.createElement('button');
    button.className = 'copy-code';
    button.type = 'button';
    button.textContent = 'copiar';
    button.addEventListener('click', async () => {
      try { await navigator.clipboard.writeText(pre.textContent); } catch { /* clipboard opcional */ }
      button.textContent = '✓ copiado';
      setTimeout(() => { button.textContent = 'copiar'; }, 1400);
    });
    wrap.appendChild(button);
  });
}
function addPrevNext(activePage) {
  const current = document.querySelector(`[data-page="${activePage}"]`);
  if (!current) return;
  document.querySelector('.page-navigation')?.remove();
  const index = ordered.indexOf(activePage);
  const previous = ordered[index - 1];
  const next = ordered[index + 1];
  const navigation = document.createElement('nav');
  navigation.className = 'page-navigation';
  navigation.setAttribute('aria-label', 'Páginas adjacentes');
  navigation.innerHTML = `${previous ? `<a href="${routeFor(previous)}"><small>← Anterior</small><strong>${titles[previous]}</strong></a>` : '<span></span>'}${next ? `<a class="next" href="${routeFor(next)}"><small>Próximo →</small><strong>${titles[next]}</strong></a>` : '<span></span>'}`;
  document.querySelector('#page-content').appendChild(navigation);
  navigation.querySelectorAll('a').forEach((link) => link.addEventListener('click', (event) => { event.preventDefault(); navigate(link.getAttribute('href').replace('/docs/', '') || 'inicio'); }));
}
function renderPage() {
  const requested = slugFromPath();
  const page = pages.some((item) => item.dataset.page === requested) ? requested : 'inicio';
  pages.forEach((item) => item.classList.toggle('active', item.dataset.page === page));
  navLinks.forEach((link) => link.classList.toggle('active', link.dataset.page === page));
  breadcrumb.textContent = `${groups[page] || 'Documentação'} / ${titles[page]}`;
  document.title = `${titles[page]} · ZittoDB Docs`;
  renderToc(page);
  addCopyButtons(page);
  addPrevNext(page);
  sidebar.classList.remove('open');
  window.scrollTo({ top: 0, behavior: 'smooth' });
}

navLinks.forEach((link) => link.addEventListener('click', (event) => { event.preventDefault(); navigate(link.dataset.page); }));
window.addEventListener('popstate', renderPage);
window.addEventListener('hashchange', renderPage);
renderPage();

document.querySelector('#menu-button')?.addEventListener('click', () => sidebar.classList.toggle('open'));
document.querySelector('#search-trigger')?.addEventListener('click', openSearch);
document.querySelectorAll('[data-close-search]').forEach((element) => element.addEventListener('click', closeSearch));
document.addEventListener('keydown', (event) => {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') { event.preventDefault(); openSearch(); }
  if (event.key === 'Escape') closeSearch();
});

const searchIndex = pages.map((page) => ({ id: page.dataset.page, title: titles[page.dataset.page], group: groups[page.dataset.page], text: page.textContent.replace(/\s+/g, ' ').trim() }));
function openSearch() { searchModal.classList.add('open'); searchModal.setAttribute('aria-hidden', 'false'); searchInput.focus(); searchInput.select(); renderResults(''); }
function closeSearch() { searchModal.classList.remove('open'); searchModal.setAttribute('aria-hidden', 'true'); }
function renderResults(query) {
  const normalized = query.trim().toLowerCase();
  const results = normalized ? searchIndex.filter((item) => `${item.title} ${item.text}`.toLowerCase().includes(normalized)) : searchIndex.slice(0, 5);
  if (!results.length) { searchResults.innerHTML = '<div class="search-empty">Nada encontrado. Tente outro termo.</div>'; return; }
  searchResults.innerHTML = results.map((item, index) => `<a class="search-result${index === 0 ? ' selected' : ''}" href="${routeFor(item.id)}"><small>${item.group}</small><strong>${item.title}</strong><span>${item.text.slice(0, 115)}…</span></a>`).join('');
  searchResults.querySelectorAll('a').forEach((link) => link.addEventListener('click', (event) => { event.preventDefault(); closeSearch(); navigate(link.getAttribute('href').replace('/docs/', '') || 'inicio'); }));
}
searchInput.addEventListener('input', (event) => renderResults(event.target.value));
searchInput.addEventListener('keydown', (event) => {
  const results = [...searchResults.querySelectorAll('.search-result')];
  if (event.key === 'Enter' && results[0]) { results[0].click(); return; }
  if ((event.key === 'ArrowDown' || event.key === 'ArrowUp') && results.length) {
    event.preventDefault();
    const active = searchResults.querySelector('.selected');
    const index = Math.max(0, results.indexOf(active) + (event.key === 'ArrowDown' ? 1 : -1));
    results.forEach((result) => result.classList.remove('selected'));
    results[Math.min(index, results.length - 1)].classList.add('selected');
  }
});
