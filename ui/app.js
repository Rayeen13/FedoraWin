const invoke = window.__TAURI__?.core?.invoke;
const app = document.querySelector('#app');
const params = new URLSearchParams(location.search);
const view = params.get('view') || 'panel';

const ACCENTS = {
  blue: '#3584e4', teal: '#2190a4', green: '#3a944a', yellow: '#c88800',
  orange: '#ed5b00', red: '#e62d42', pink: '#d56199', purple: '#9141ac', slate: '#6f8396'
};

let shell = { appearance: { theme: 'dark', accent: 'blue' }, activitiesOpen: false };

function applyAppearance() {
  const { theme, accent } = shell.appearance;
  document.documentElement.dataset.theme = theme === 'system'
    ? (matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark')
    : theme;
  document.documentElement.style.setProperty('--accent', ACCENTS[accent] || ACCENTS.blue);
}

async function call(command, payload = {}) {
  if (!invoke) return null;
  return invoke(command, payload);
}

function iconGrid() {
  return '<span class="grid-icon" aria-hidden="true">' + '<i></i>'.repeat(9) + '</span>';
}

function renderPanel() {
  app.innerHTML = `
    <section class="panel">
      <div class="panel__left"><button id="activities" class="panel-button workspace-indicator" aria-label="Activities"></button></div>
      <div class="panel__center"><button id="clock" class="panel-button"></button></div>
      <div class="panel__right">
        <button id="quick" class="panel-button status" aria-label="Quick Settings">
          <svg viewBox="0 0 20 16"><path d="M2 6c4-4 12-4 16 0M5 9c3-3 7-3 10 0M9 12c1-1 2-1 3 0"/></svg>
          <svg viewBox="0 0 20 18"><path d="M2 7h4l5-4v12l-5-4H2z"/><path d="M13 6c2 2 2 4 0 6M15 4c4 4 4 6 0 10"/></svg>
          <span>100%</span>
        </button>
      </div>
    </section>`;
  const clock = document.querySelector('#clock');
  const tick = () => clock.textContent = new Intl.DateTimeFormat(undefined, { weekday: 'short', hour: '2-digit', minute: '2-digit' }).format(new Date());
  tick(); setInterval(tick, 1000);
  document.querySelector('#activities').addEventListener('click', () => call('toggle_activities'));
}

function renderActivities() {
  app.innerHTML = `
    <section class="activities">
      <div class="search-shell"><input id="search" class="search" placeholder="Type to search" autocomplete="off" /></div>
      <div class="overview">
        <div class="workspace-peek"></div>
        <div class="workspace-main"></div>
        <div class="workspace-peek"></div>
      </div>
      <div class="dash-wrap"><div class="dash">
        <button class="dash-button" title="Files">Files</button>
        <button class="dash-button" title="Terminal">Term</button>
        <button class="dash-button" title="Browser">Web</button>
        <button class="dash-button" title="Show Applications">${iconGrid()}</button>
      </div></div>
    </section>`;
  document.querySelector('#search').focus();
}

function renderQuickSettings() {
  const themeButtons = ['light','dark','system'].map(t => `<button data-theme="${t}" aria-pressed="${shell.appearance.theme === t}">${t[0].toUpperCase()+t.slice(1)}</button>`).join('');
  const accents = Object.entries(ACCENTS).map(([name,color]) => `<button class="accent" data-accent="${name}" style="--swatch:${color}" aria-label="${name}" aria-pressed="${shell.appearance.accent === name}"></button>`).join('');
  app.innerHTML = `
    <section class="popover"><div class="popover-card">
      <div class="control-row"><span>Vol</span><input type="range" min="0" max="100" value="68"></div>
      <div class="control-row"><span>Sun</span><input type="range" min="0" max="100" value="70"></div>
      <div class="quick-grid">
        <button id="wifi" class="quick-tile" aria-pressed="true">Wi-Fi<small>Connected</small></button>
        <button class="quick-tile" aria-pressed="false">Bluetooth<small>On</small></button>
        <button class="quick-tile" aria-pressed="false">Power Mode<small>Balanced</small></button>
        <button class="quick-tile" aria-pressed="${shell.appearance.theme === 'dark'}">Dark Style<small>Appearance</small></button>
        <button class="quick-tile" aria-pressed="false">Night Light<small>Display</small></button>
        <button class="quick-tile" aria-pressed="false">Airplane Mode<small>Network</small></button>
      </div>
      <div class="appearance">
        <div class="segmented">${themeButtons}</div>
        <div class="accents">${accents}</div>
      </div>
    </div></section>`;
  document.querySelector('#wifi').addEventListener('click', async e => {
    const next = e.currentTarget.getAttribute('aria-pressed') !== 'true';
    await call('set_wifi_enabled', { enabled: next });
    e.currentTarget.setAttribute('aria-pressed', String(next));
  });
  document.querySelectorAll('[data-theme]').forEach(button => button.addEventListener('click', async () => {
    shell = await call('set_appearance', { theme: button.dataset.theme, accent: shell.appearance.accent }) || shell;
    applyAppearance(); renderQuickSettings();
  }));
  document.querySelectorAll('[data-accent]').forEach(button => button.addEventListener('click', async () => {
    shell = await call('set_appearance', { theme: shell.appearance.theme, accent: button.dataset.accent }) || shell;
    applyAppearance(); renderQuickSettings();
  }));
}

function renderDateMenu() {
  const now = new Date();
  const title = new Intl.DateTimeFormat(undefined, { weekday: 'long', month: 'long', day: 'numeric' }).format(now);
  app.innerHTML = `<section class="popover"><div class="popover-card"><h2>${title}</h2><p style="color:var(--muted)">Calendar and notification integration is the next native gate.</p></div></section>`;
}

async function bootstrap() {
  shell = await call('get_shell_state') || shell;
  applyAppearance();
  if (view === 'panel') renderPanel();
  else if (view === 'activities') renderActivities();
  else if (view === 'quick-settings') renderQuickSettings();
  else renderDateMenu();
}
bootstrap();
