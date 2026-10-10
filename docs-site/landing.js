const menuToggle = document.querySelector('.menu-toggle');
const siteNav = document.querySelector('#site-nav');

menuToggle?.addEventListener('click', () => {
  const isOpen = menuToggle.getAttribute('aria-expanded') === 'true';
  menuToggle.setAttribute('aria-expanded', String(!isOpen));
  siteNav?.classList.toggle('open', !isOpen);
});

document.querySelectorAll('.site-nav a').forEach((link) => link.addEventListener('click', () => {
  menuToggle?.setAttribute('aria-expanded', 'false');
  siteNav?.classList.remove('open');
}));

const previewTitles = {
  devices: ['Dispositivos', 'Gerencie seus dispositivos Android conectados.'],
  commands: ['Comandos', 'Operações tipadas, claras e auditáveis.'],
  fastboot: ['Fastboot', 'Ações de bootloader com confirmação explícita.'],
  history: ['Histórico', 'Auditoria local das operações realizadas.'],
  screen: ['Tela', 'Espelhamento e gravação com scrcpy.'],
  apps: ['Aplicativos', 'Gerencie pacotes, APKs e debloat.'],
  files: ['Arquivos', 'Navegue e transfira arquivos pelo dispositivo.'],
  shell: ['Shell', 'Sessão persistente no dispositivo selecionado.'],
  logs: ['Logs', 'Logcat em tempo real com filtros.'],
};

function showPreview(view) {
  const [title, description] = previewTitles[view] || previewTitles.devices;
  const titleEl = document.querySelector('#preview-title');
  const descriptionEl = document.querySelector('#preview-description');
  if (titleEl) titleEl.textContent = title;
  if (descriptionEl) descriptionEl.textContent = description;
  document.querySelectorAll('[data-panel]').forEach((panel) => panel.classList.toggle('hidden', panel.dataset.panel !== (['devices', 'commands', 'fastboot', 'history'].includes(view) ? view : 'devices')));
  document.querySelectorAll('.app-nav').forEach((button) => button.classList.toggle('active', button.dataset.view === view));
  document.querySelectorAll('.quick-actions button').forEach((button) => button.classList.toggle('selected', button.dataset.view === view));
}

document.querySelectorAll('[data-view]').forEach((button) => button.addEventListener('click', () => showPreview(button.dataset.view)));
document.querySelector('[data-preview-action="refresh"]')?.addEventListener('click', (event) => {
  const button = event.currentTarget;
  button.textContent = '✓ Atualizado';
  setTimeout(() => { button.textContent = '↻ Atualizar'; }, 1200);
});

document.querySelectorAll('[data-install]').forEach((tab) => tab.addEventListener('click', () => {
  document.querySelectorAll('[data-install]').forEach((item) => item.classList.toggle('active', item === tab));
  document.querySelectorAll('[data-code]').forEach((code) => code.classList.toggle('hidden', code.dataset.code !== tab.dataset.install));
}));

document.querySelector('[data-copy-install]')?.addEventListener('click', async (event) => {
  const activeCode = document.querySelector('[data-code]:not(.hidden) code')?.textContent || '';
  const button = event.currentTarget;
  try { await navigator.clipboard.writeText(activeCode); button.textContent = 'Copiado'; } catch { button.textContent = 'Selecione o comando'; }
  setTimeout(() => { button.textContent = 'Copiar'; }, 1400);
});

const reduceMotion = window.matchMedia('(prefers-reduced-motion: reduce)');
if (reduceMotion.matches) document.documentElement.classList.add('reduce-motion');
