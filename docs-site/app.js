const pages = [...document.querySelectorAll('.doc-page')];
const navLinks = [...document.querySelectorAll('[data-page]')];
const breadcrumb = document.querySelector('#breadcrumb');
const sidebar = document.querySelector('#sidebar');
const searchModal = document.querySelector('#search-modal');
const searchInput = document.querySelector('#search-input');
const searchResults = document.querySelector('#search-results');
const titles = Object.fromEntries(pages.map((page) => [page.dataset.page, page.querySelector('h1')?.textContent || page.dataset.page]));

function currentPage() {
  const requested = window.location.hash.replace('#', '') || 'inicio';
  return pages.some((page) => page.dataset.page === requested) ? requested : 'inicio';
}

function renderPage() {
  const page = currentPage();
  pages.forEach((item) => item.classList.toggle('active', item.dataset.page === page));
  navLinks.forEach((link) => link.classList.toggle('active', link.dataset.page === page));
  breadcrumb.textContent = titles[page];
  document.title = `${titles[page]} · ZittoDB`;
  sidebar.classList.remove('open');
  window.scrollTo({ top: 0, behavior: 'smooth' });
}

window.addEventListener('hashchange', renderPage);
renderPage();

document.querySelector('#menu-button')?.addEventListener('click', () => sidebar.classList.toggle('open'));

document.querySelector('#search-trigger')?.addEventListener('click', openSearch);
document.querySelectorAll('[data-close-search]').forEach((element) => element.addEventListener('click', closeSearch));

document.addEventListener('keydown', (event) => {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
    event.preventDefault();
    openSearch();
  }
  if (event.key === 'Escape') closeSearch();
});

const searchIndex = pages.map((page) => ({
  id: page.dataset.page,
  title: titles[page.dataset.page],
  text: page.textContent.replace(/\s+/g, ' ').trim(),
}));

function openSearch() {
  searchModal.classList.add('open');
  searchModal.setAttribute('aria-hidden', 'false');
  searchInput.focus();
  searchInput.select();
  renderResults('');
}
function closeSearch() {
  searchModal.classList.remove('open');
  searchModal.setAttribute('aria-hidden', 'true');
}
function renderResults(query) {
  const normalized = query.trim().toLowerCase();
  const results = normalized ? searchIndex.filter((item) => `${item.title} ${item.text}`.toLowerCase().includes(normalized)) : searchIndex.slice(0, 5);
  if (!results.length) {
    searchResults.innerHTML = '<div class="search-empty">Nenhum resultado. Tente outra palavra-chave.</div>';
    return;
  }
  searchResults.innerHTML = results.map((item, index) => `<a class="search-result${index === 0 ? ' selected' : ''}" href="#${item.id}"><strong>${item.title}</strong><span>${item.text.slice(0, 115)}…</span></a>`).join('');
  searchResults.querySelectorAll('a').forEach((link) => link.addEventListener('click', closeSearch));
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
