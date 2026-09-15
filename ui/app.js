const invoke = window.__TAURI__?.core?.invoke;
const listen = window.__TAURI__?.event?.listen;
const app = document.querySelector('#app');
const params = new URLSearchParams(location.search);
const view = params.get('view') || 'panel';
const captureMode = params.get('capture') || '';
const mockMode = params.get('mock') === '1' && !invoke;

const ACCENTS = {
  blue: '#3584e4', teal: '#2190a4', green: '#3a944a', yellow: '#c88800',
  orange: '#ed5b00', red: '#e62d42', pink: '#d56199', purple: '#9141ac', slate: '#6f8396'
};
const ICONS = {
  search: '<svg viewBox="0 0 24 24"><circle cx="11" cy="11" r="6.5"/><path d="m16 16 4 4"/></svg>',
  wifi: '<svg viewBox="0 0 24 24"><path d="M3.5 9.5c4.7-4.2 12.3-4.2 17 0M6.8 13c2.9-2.6 7.5-2.6 10.4 0M10.2 16.6c1-.9 2.6-.9 3.6 0"/><circle cx="12" cy="19" r="1" class="fill"/></svg>',
  bluetooth: '<svg viewBox="0 0 24 24"><path d="m12 3 5 4.5-5 4.5 5 4.5-5 4.5V3ZM7 7l10 10M7 17 17 7"/></svg>',
  volume: '<svg viewBox="0 0 24 24"><path d="M4 10h4l5-4v12l-5-4H4v-4Z"/><path d="M16 9c1.7 1.7 1.7 4.3 0 6M18.5 6.5c3.2 3.2 3.2 7.8 0 11"/></svg>',
  brightness: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="3.5"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9 7 7M17 17l2.1 2.1M19.1 4.9 17 7M7 17l-2.1 2.1"/></svg>',
  power: '<svg viewBox="0 0 24 24"><path d="M12 3v8M7 5.8a8 8 0 1 0 10 0"/></svg>',
  settings: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="3"/><path d="M19 12a7 7 0 0 0-.1-1l2-1.6-2-3.4-2.5 1A7 7 0 0 0 14.7 6l-.4-2.7h-4.6L9.3 6A7 7 0 0 0 7.6 7L5 6 3 9.4 5.1 11A7 7 0 0 0 5 12c0 .3 0 .7.1 1L3 14.6 5 18l2.6-1a7 7 0 0 0 1.7 1l.4 2.7h4.6l.4-2.7a7 7 0 0 0 1.7-1l2.6 1 2-3.4-2.1-1.6c.1-.3.1-.7.1-1Z"/></svg>',
  screenshot: '<svg viewBox="0 0 24 24"><rect x="4" y="6" width="16" height="13" rx="2"/><path d="M9 6 10.5 4h3L15 6"/><circle cx="12" cy="12.5" r="3"/></svg>',
  chevron: '<svg viewBox="0 0 24 24"><path d="m9 6 6 6-6 6"/></svg>'
};

let shell = { appearance: { theme: 'dark', accent: 'blue' }, activitiesOpen: false };
let apps = [];
let windows = [];
let activitiesMode = 'windows';
let calendarCursor = new Date();
let windowEventsBound = false;

function applyAppearance() {
  const { theme, accent } = shell.appearance;
  document.documentElement.dataset.theme = theme === 'system'
    ? (matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark')
    : theme;
  document.documentElement.style.setProperty('--accent', ACCENTS[accent] || ACCENTS.blue);
}

async function call(command, payload = {}) {
  if (!invoke) return null;
  try { return await invoke(command, payload); }
  catch (error) { console.error(`[FedoraWin] ${command}`, error); return null; }
}

function escapeHtml(value = '') {
  return String(value).replace(/[&<>'"]/g, ch => ({'&':'&amp;','<':'&lt;','>':'&gt;',"'":'&#39;','"':'&quot;'}[ch]));
}

function iconGrid() {
  return '<span class="grid-icon" aria-hidden="true">' + '<i></i>'.repeat(9) + '</span>';
}

function appInitials(name) {
  return name.split(/\s+/).filter(Boolean).slice(0,2).map(part => part[0]).join('').toUpperCase();
}

function appHaystack(entry) {
  return [entry.name, entry.appId, ...(entry.aliases || [])].join(' ').toLowerCase();
}

function renderPanel() {
  app.innerHTML = `
    <section class="panel">
      <div class="panel__left"><button id="activities" class="panel-button activities-button" aria-label="Activities"><span class="workspace-pill"></span><span class="workspace-dot"></span></button></div>
      <div class="panel__center"><button id="clock" class="panel-button"></button></div>
      <div class="panel__right">
        <button id="quick" class="panel-button status" aria-label="Quick Settings">
          ${ICONS.wifi}${ICONS.volume}<span class="battery-mark"><i></i></span>
        </button>
      </div>
    </section>`;
  const clock = document.querySelector('#clock');
  const tick = () => clock.textContent = new Intl.DateTimeFormat(undefined, { weekday: 'short', month: 'short', day: 'numeric', hour: 'numeric', minute: '2-digit' }).format(new Date());
  tick(); setInterval(tick, 15000);
  document.querySelector('#activities').addEventListener('click', () => call('toggle_activities'));
  document.querySelector('#clock').addEventListener('click', () => call('toggle_surface', { label: 'date-menu' }));
  document.querySelector('#quick').addEventListener('click', () => call('toggle_surface', { label: 'quick-settings' }));
}

async function loadActivitiesData() {
  if (mockMode) return;
  const [appList, windowList] = await Promise.all([call('list_apps'), call('list_windows')]);
  if (Array.isArray(appList)) apps = appList;
  if (Array.isArray(windowList)) windows = windowList;
}

function renderWindowOverview() {
  const currentWindows = windows.filter(w => w.onCurrentWorkspace !== false);
  const workspaceIds = [...new Set(windows.map(w => w.desktopId).filter(Boolean))];
  const cards = currentWindows.length ? currentWindows.map(w => `
    <article class="window-card" title="${escapeHtml(w.title)}">
      <div class="window-card__preview">
        <div class="window-card__bar">
          <span class="window-card__bar-title">${escapeHtml(w.title)}</span>
          <button class="window-card__close" data-close-window="${escapeHtml(w.handle)}" aria-label="Close ${escapeHtml(w.title)}">×</button>
        </div>
        <button class="window-card__live-preview" data-window="${escapeHtml(w.handle)}" data-thumbnail-window="${escapeHtml(w.handle)}" aria-label="Open ${escapeHtml(w.title)}"></button>
      </div>
      <button class="window-card__title-button" data-window="${escapeHtml(w.handle)}">${escapeHtml(w.title)}</button>
    </article>`).join('') : '<div class="overview-empty">No open windows on this desktop</div>';
  const count = Math.max(workspaceIds.length, 1);
  return `<div class="workspace-strip workspace-strip--native"><div class="workspace-current"><span>Current workspace</span><small>${count} detected workspace${count === 1 ? '' : 's'}</small></div><div class="workspace-main"><div class="window-grid">${cards}</div></div></div>`;
}

async function syncLiveThumbnails() {
  if (!invoke) return;
  const search = document.querySelector('#search');
  if (activitiesMode !== 'windows' || search?.value.trim()) {
    await call('clear_window_thumbnails');
    return;
  }

  const items = [...document.querySelectorAll('[data-thumbnail-window]')].map(element => {
    const rect = element.getBoundingClientRect();
    return {
      handle: element.dataset.thumbnailWindow,
      left: rect.left,
      top: rect.top,
      width: rect.width,
      height: rect.height
    };
  });
  await call('sync_window_thumbnails', { items });
}

async function refreshNativeWindows() {
  const windowList = await call('list_windows');
  if (!Array.isArray(windowList)) return;
  windows = windowList;
  const search = document.querySelector('#search');
  if (!search?.value.trim() && activitiesMode === 'windows') refreshActivitiesContent();
}

function renderAppGrid(list = apps) {
  const items = list.map(entry => `
    <button class="app-tile" data-app-id="${escapeHtml(entry.appId)}" title="${escapeHtml(entry.name)}">
      <span class="app-icon">${escapeHtml(appInitials(entry.name))}</span>
      <span class="app-name">${escapeHtml(entry.name)}</span>
    </button>`).join('');
  return `<div class="applications-grid">${items || '<div class="overview-empty">No matching applications</div>'}</div>`;
}

function bindActivitiesContent() {
  document.querySelectorAll('[data-window]').forEach(button => button.addEventListener('click', async () => {
    await call('activate_window', { handle: button.dataset.window });
    await call('toggle_activities');
  }));
  document.querySelectorAll('[data-close-window]').forEach(button => button.addEventListener('click', async event => {
    event.stopPropagation();
    await call('close_window', { handle: button.dataset.closeWindow });
    setTimeout(refreshNativeWindows, 120);
  }));
  document.querySelectorAll('[data-app-id]').forEach(button => button.addEventListener('click', async () => {
    await call('launch_app', { appId: button.dataset.appId });
    await call('toggle_activities');
  }));
}

function refreshActivitiesContent(query = '') {
  const content = document.querySelector('#activities-content');
  if (!content) return;
  const q = query.trim().toLowerCase();
  if (q) {
    const filtered = apps.filter(entry => appHaystack(entry).includes(q));
    content.innerHTML = `<div class="search-results"><div class="results-label">Applications</div>${renderAppGrid(filtered)}</div>`;
  } else {
    content.innerHTML = activitiesMode === 'apps' ? renderAppGrid() : renderWindowOverview();
  }
  document.querySelector('#show-apps')?.classList.toggle('is-active', activitiesMode === 'apps' && !q);
  bindActivitiesContent();
  requestAnimationFrame(() => requestAnimationFrame(syncLiveThumbnails));
}

async function renderActivities() {
  app.innerHTML = `
    <section class="activities">
      <div class="search-shell"><span class="search-icon">${ICONS.search}</span><input id="search" class="search" placeholder="Type to search" autocomplete="off" spellcheck="false" /></div>
      <div id="activities-content" class="activities-content"><div class="loading">Loading workspace…</div></div>
      <div class="dash-wrap"><div class="dash">
        <button class="dash-button" data-search="files" title="Files"><span class="dash-icon">F</span></button>
        <button class="dash-button" data-search="terminal" title="Terminal"><span class="dash-icon">T</span></button>
        <button class="dash-button" data-search="browser" title="Web Browser"><span class="dash-icon">W</span></button>
        <span class="dash-separator"></span>
        <button id="show-apps" class="dash-button" title="Show Applications">${iconGrid()}</button>
      </div></div>
    </section>`;
  const search = document.querySelector('#search');
  search.addEventListener('input', () => refreshActivitiesContent(search.value));
  document.querySelector('#show-apps').addEventListener('click', () => {
    activitiesMode = activitiesMode === 'apps' ? 'windows' : 'apps';
    search.value = '';
    refreshActivitiesContent();
  });
  document.querySelectorAll('[data-search]').forEach(button => button.addEventListener('click', () => {
    search.value = button.dataset.search;
    search.focus();
    refreshActivitiesContent(search.value);
  }));
  await loadActivitiesData();
  if (listen && !windowEventsBound) {
    windowEventsBound = true;
    await listen('fedorawin://windows-changed', refreshNativeWindows);
  }
  if (captureMode === 'apps') activitiesMode = 'apps';
  if (captureMode === 'search-terminal') search.value = 'terminal';
  refreshActivitiesContent(search.value);
  search.focus();
}

function slider(icon, id, value, label) {
  return `<div class="slider-row"><span class="slider-icon">${icon}</span><input id="${id}" aria-label="${label}" type="range" min="0" max="100" value="${value}" style="--value:${value}%"></div>`;
}

function quickTile(id, icon, label, subtitle, pressed = false, expandable = false) {
  return `<div class="quick-item"><button id="${id}" class="quick-tile" aria-pressed="${pressed}"><span class="quick-icon">${icon}</span><span class="quick-copy"><strong>${label}</strong><small>${subtitle}</small></span></button>${expandable ? `<button class="quick-more" aria-label="${label} options">${ICONS.chevron}</button>` : ''}</div>`;
}

function renderAppearanceSheet() {
  const themeButtons = ['light','dark','system'].map(t => `<button data-theme="${t}" aria-pressed="${shell.appearance.theme === t}">${t[0].toUpperCase()+t.slice(1)}</button>`).join('');
  const accents = Object.entries(ACCENTS).map(([name,color]) => `<button class="accent" data-accent="${name}" style="--swatch:${color}" aria-label="${name}" aria-pressed="${shell.appearance.accent === name}"></button>`).join('');
  return `<section class="appearance-sheet"><div class="sheet-heading"><button id="appearance-back" class="icon-button">‹</button><strong>Appearance</strong><span></span></div><div class="segmented">${themeButtons}</div><div class="accents">${accents}</div></section>`;
}

function bindRangeFill(input) {
  const sync = () => input.style.setProperty('--value', `${input.value}%`);
  input.addEventListener('input', sync); sync();
}

function bindAppearance() {
  document.querySelectorAll('[data-theme]').forEach(button => button.addEventListener('click', async () => {
    shell = await call('set_appearance', { theme: button.dataset.theme, accent: shell.appearance.accent }) || shell;
    applyAppearance(); renderQuickSettings(true);
  }));
  document.querySelectorAll('[data-accent]').forEach(button => button.addEventListener('click', async () => {
    shell = await call('set_appearance', { theme: shell.appearance.theme, accent: button.dataset.accent }) || shell;
    applyAppearance(); renderQuickSettings(true);
  }));
}

function renderQuickSettings(appearanceOpen = false) {
  app.innerHTML = `<section class="popover quick-popover"><div class="popover-card quick-card">
    ${appearanceOpen ? renderAppearanceSheet() : `
      <div class="quick-header"><span class="battery-summary"><span class="battery-mark"><i></i></span><strong>100%</strong></span><span class="header-actions"><button class="icon-button" title="Screenshot">${ICONS.screenshot}</button><button id="appearance-open" class="icon-button" title="Appearance">${ICONS.settings}</button><button class="icon-button" title="Power">${ICONS.power}</button></span></div>
      <div class="sliders">${slider(ICONS.volume,'volume',68,'Volume')}${slider(ICONS.brightness,'brightness',70,'Brightness')}</div>
      <div class="quick-grid">
        ${quickTile('wifi',ICONS.wifi,'Wi-Fi','Connected',true,true)}
        ${quickTile('bluetooth',ICONS.bluetooth,'Bluetooth','On',false,true)}
        ${quickTile('power-mode',ICONS.power,'Power Mode','Balanced',false,true)}
        ${quickTile('dark-style',ICONS.brightness,'Dark Style',shell.appearance.theme === 'dark' ? 'On' : 'Off',shell.appearance.theme === 'dark')}
        ${quickTile('night-light',ICONS.brightness,'Night Light','Off',false)}
        ${quickTile('airplane',ICONS.wifi,'Airplane Mode','Off',false)}
      </div>
      <button class="background-apps"><span>Background Apps</span><span>0</span></button>`}
  </div></section>`;

  if (appearanceOpen) {
    document.querySelector('#appearance-back').addEventListener('click', () => renderQuickSettings(false));
    bindAppearance();
    return;
  }
  document.querySelectorAll('input[type="range"]').forEach(bindRangeFill);
  document.querySelector('#appearance-open').addEventListener('click', () => renderQuickSettings(true));
  document.querySelector('#wifi').addEventListener('click', async event => {
    const next = event.currentTarget.getAttribute('aria-pressed') !== 'true';
    const result = await call('set_wifi_enabled', { enabled: next });
    if (result !== null || !invoke) event.currentTarget.setAttribute('aria-pressed', String(next));
  });
  document.querySelector('#dark-style').addEventListener('click', async () => {
    const nextTheme = shell.appearance.theme === 'dark' ? 'light' : 'dark';
    shell = await call('set_appearance', { theme: nextTheme, accent: shell.appearance.accent }) || { ...shell, appearance: { ...shell.appearance, theme: nextTheme } };
    applyAppearance(); renderQuickSettings(false);
  });
}

function monthModel(date) {
  const year = date.getFullYear();
  const month = date.getMonth();
  const first = new Date(year, month, 1);
  const days = new Date(year, month + 1, 0).getDate();
  const mondayOffset = (first.getDay() + 6) % 7;
  return { year, month, days, mondayOffset };
}

function renderCalendarGrid() {
  const now = new Date();
  const { year, month, days, mondayOffset } = monthModel(calendarCursor);
  const cells = [];
  for (let i = 0; i < mondayOffset; i++) cells.push('<span class="calendar-day is-outside"></span>');
  for (let day = 1; day <= days; day++) {
    const today = year === now.getFullYear() && month === now.getMonth() && day === now.getDate();
    cells.push(`<button class="calendar-day${today ? ' is-today' : ''}">${day}</button>`);
  }
  while (cells.length % 7) cells.push('<span class="calendar-day is-outside"></span>');
  const monthTitle = new Intl.DateTimeFormat(undefined,{month:'long',year:'numeric'}).format(calendarCursor);
  return `<div class="calendar-head"><button id="month-prev" class="icon-button">‹</button><strong>${escapeHtml(monthTitle)}</strong><button id="month-next" class="icon-button">›</button></div><div class="weekdays">${['M','T','W','T','F','S','S'].map(d=>`<span>${d}</span>`).join('')}</div><div class="calendar-grid">${cells.join('')}</div>`;
}

function renderDateMenu() {
  const now = new Date();
  const dateTitle = new Intl.DateTimeFormat(undefined, { weekday: 'long', month: 'long', day: 'numeric' }).format(now);
  app.innerHTML = `<section class="popover date-popover"><div class="popover-card date-card">
    <div class="notifications-pane"><div class="date-title">${escapeHtml(dateTitle)}</div><div class="notification-placeholder"><strong>No Notifications</strong><span>Notifications will appear here</span></div></div>
    <div class="calendar-pane">${renderCalendarGrid()}<div class="calendar-footer"><button>Open Calendar</button><button>Date &amp; Time Settings</button></div></div>
  </div></section>`;
  document.querySelector('#month-prev').addEventListener('click', () => { calendarCursor = new Date(calendarCursor.getFullYear(), calendarCursor.getMonth()-1, 1); renderDateMenu(); });
  document.querySelector('#month-next').addEventListener('click', () => { calendarCursor = new Date(calendarCursor.getFullYear(), calendarCursor.getMonth()+1, 1); renderDateMenu(); });
}

async function bootstrap() {
  shell = await call('get_shell_state') || shell;
  if (mockMode) {
    apps = [
      { name: 'Files', appId: 'mock.files', aliases: ['files','file manager'] },
      { name: 'Windows Terminal', appId: 'mock.terminal', aliases: ['terminal','console','shell'] },
      { name: 'Firefox', appId: 'mock.firefox', aliases: ['browser','web'] },
      { name: 'Settings', appId: 'mock.settings', aliases: ['settings','preferences'] },
      { name: 'Text Editor', appId: 'mock.editor', aliases: ['editor','text'] },
      { name: 'Calculator', appId: 'mock.calculator', aliases: ['calculator','math'] },
      { name: 'Photos', appId: 'mock.photos', aliases: ['images','photos'] },
      { name: 'Calendar', appId: 'mock.calendar', aliases: ['calendar','events'] },
      { name: 'Maps', appId: 'mock.maps', aliases: ['maps','location'] },
      { name: 'Weather', appId: 'mock.weather', aliases: ['weather'] },
    ];
    windows = [
      { handle: '1', title: 'FedoraWin — GitHub', minimized: false, desktopId: 'mock-1', onCurrentWorkspace: true },
      { handle: '2', title: 'README.md — Text Editor', minimized: false, desktopId: 'mock-1', onCurrentWorkspace: true },
      { handle: '3', title: 'Windows Terminal', minimized: false, desktopId: 'mock-1', onCurrentWorkspace: true },
    ];
  }
  applyAppearance();
  if (view === 'panel') renderPanel();
  else if (view === 'activities') await renderActivities();
  else if (view === 'quick-settings') renderQuickSettings(captureMode === 'appearance');
  else renderDateMenu();
}
bootstrap();
