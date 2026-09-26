const invoke = window.__TAURI__?.core?.invoke;
const listen = window.__TAURI__?.event?.listen;
const app = document.querySelector('#app');
const params = new URLSearchParams(location.search);
const view = params.get('view') || 'panel';
const captureMode = params.get('capture') || '';
const captureEvidence = params.has('capture');
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
let appsLoaded = false;
let windows = [];
let activitiesMode = 'windows';
let calendarCursor = new Date();
let windowEventsBound = false;
let appPage = 0;
let volumeCommitTimer = null;
let volumeRevision = 0;
const quickState = { volume: 68, brightness: 70, power: null };
const APPS_PER_PAGE = 24;
const appIconCache = new Map();
const appIconQueued = new Set();
const appIconQueue = [];
let appIconActive = 0;
const ICON_WORKERS = 4;

function nativeAppIconMarkup(entry, fallback = appInitials(entry.name)) {
  const src = appIconCache.get(entry.appId);
  return src
    ? `<img src="${src}" alt="" draggable="false" loading="eager">`
    : escapeHtml(fallback);
}

function showResolvedAppIcon(appId, uri) {
  document.querySelectorAll('[data-app-id]').forEach(tile => {
    if (tile.dataset.appId !== appId) return;
    const target = tile.querySelector('.app-icon, .dash-icon');
    if (!target) return;
    if (uri) {
      const img = document.createElement('img');
      img.src = uri;
      img.alt = '';
      img.draggable = false;
      target.replaceChildren(img);
    }
  });
}

function runIconQueue() {
  while (invoke && appIconActive < ICON_WORKERS && appIconQueue.length) {
    const appId = appIconQueue.shift();
    appIconActive += 1;
    (async () => {
      const result = await call('get_app_icon', { appId });
      const icon = typeof result === 'string' && /^data:image\/png;base64,[A-Za-z0-9+/=]+$/.test(result)
        ? result : null;
      appIconCache.set(appId, icon);
      if (appIconCache.size > 256) {
        appIconCache.delete(appIconCache.keys().next().value);
      }
      showResolvedAppIcon(appId, icon);
      appIconQueued.delete(appId);
    })().finally(() => {
      appIconActive -= 1;
      runIconQueue();
    });
  }
}

function hydrateRenderedAppIcons() {
  const ids = [...new Set(
    [...document.querySelectorAll('[data-app-id]')]
      .map(tile => tile.dataset.appId)
      .filter(Boolean)
  )];
  for (const appId of ids) {
    if (appIconCache.has(appId) || appIconQueued.has(appId)) continue;
    appIconQueued.add(appId);
    appIconQueue.push(appId);
  }
  runIconQueue();
  return Promise.all(ids.map(id => {
    if (appIconCache.has(id)) return Promise.resolve();
    return new Promise(resolve => {
      // Capture readiness is bounded, even if an app's Shell handler hangs.
      const poll = () => {
        if (appIconCache.has(id) || !appIconQueued.has(id)) resolve();
        else setTimeout(poll, 75);
      };
      poll();
    });
  }));
}

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

function withTimeout(promise, timeoutMs, fallback = null) {
  let timer;
  const timeout = new Promise(resolve => {
    timer = setTimeout(() => resolve(fallback), timeoutMs);
  });
  return Promise.race([promise, timeout]).finally(() => clearTimeout(timer));
}

function nextPaint() {
  return new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve)));
}

function delay(milliseconds) {
  return new Promise(resolve => setTimeout(resolve, milliseconds));
}

async function signalCaptureReady() {
  if (!captureEvidence || view === 'panel') return;
  await withTimeout(nextPaint(), 2000, null);
  if (view === 'activities') await withTimeout(syncLiveThumbnails(), 3500, null);

  for (let attempt = 0; attempt < 3; attempt += 1) {
    const acknowledged = await withTimeout(
      call('mark_capture_ready', { label: view }),
      1500,
      null
    );
    if (acknowledged === view) return;
    await delay(120);
  }
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

function appSearchScore(entry, query) {
  const q = query.trim().toLowerCase();
  if (!q) return 0;
  const name = entry.name.toLowerCase();
  const id = entry.appId.toLowerCase();
  const aliases = (entry.aliases || []).map(alias => alias.toLowerCase());
  if (name === q) return 0;
  if (name.startsWith(q)) return 1;
  if (aliases.includes(q)) return 2;
  if (aliases.some(alias => alias.startsWith(q))) return 3;
  if (name.includes(q)) return 4;
  if (aliases.some(alias => alias.includes(q))) return 5;
  if (id.includes(q)) return 6;
  return Number.POSITIVE_INFINITY;
}

function searchApps(query) {
  return apps
    .map(entry => ({ entry, score: appSearchScore(entry, query) }))
    .filter(result => Number.isFinite(result.score))
    .sort((a, b) => a.score - b.score || a.entry.name.localeCompare(b.entry.name))
    .map(result => result.entry);
}

function findFavorite(query) {
  return searchApps(query)[0] || null;
}

function appLooksRunning(entry) {
  const name = entry.name.toLowerCase();
  return windows.some(window => {
    const title = (window.title || '').toLowerCase();
    return title.includes(name) || name.split(/\s+/).some(part => part.length >= 5 && title.includes(part));
  });
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
  const [appList, windowList] = await Promise.all([
    withTimeout(call('list_apps'), 16000, null),
    withTimeout(call('list_windows'), 3000, [])
  ]);
  appsLoaded = Array.isArray(appList);
  if (appsLoaded) apps = appList;
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
  return `<div class="workspace-strip workspace-strip--native">
    <button class="workspace-nav workspace-nav--previous" data-workspace-direction="-1" aria-label="Previous workspace">‹</button>
    <div class="workspace-current"><span>Current workspace</span><small>${count} detected workspace${count === 1 ? '' : 's'}</small></div>
    <div class="workspace-main"><div class="window-grid">${cards}</div></div>
    <button class="workspace-nav workspace-nav--next" data-workspace-direction="1" aria-label="Next workspace">›</button>
  </div>`;
}

async function syncLiveThumbnails() {
  if (!invoke) return;
  const search = document.querySelector('#search');
  if (activitiesMode !== 'windows' || search?.value.trim()) {
    await withTimeout(call('clear_window_thumbnails'), 2000, null);
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
  await withTimeout(call('sync_window_thumbnails', { items }), 3000, null);
}

async function refreshNativeWindows() {
  const windowList = await call('list_windows');
  if (!Array.isArray(windowList)) return;
  windows = windowList;
  refreshDash();
  const search = document.querySelector('#search');
  if (!search?.value.trim() && activitiesMode === 'windows') refreshActivitiesContent();
}

function renderAppGrid(list = apps) {
  const items = list.map(entry => `
    <button class="app-tile" data-app-id="${escapeHtml(entry.appId)}" title="${escapeHtml(entry.name)}">
      <span class="app-icon">${nativeAppIconMarkup(entry)}</span>
      <span class="app-name">${escapeHtml(entry.name)}</span>
    </button>`).join('');
  return `<div class="applications-grid">${items || '<div class="overview-empty">No matching applications</div>'}</div>`;
}

function renderAppDrawer() {
  const pageCount = Math.max(1, Math.ceil(apps.length / APPS_PER_PAGE));
  appPage = Math.min(Math.max(appPage, 0), pageCount - 1);
  const pageApps = apps.slice(appPage * APPS_PER_PAGE, (appPage + 1) * APPS_PER_PAGE);
  const pages = Array.from({ length: pageCount }, (_, index) =>
    `<button class="app-page-dot${index === appPage ? ' is-active' : ''}" data-app-page="${index}" aria-label="Applications page ${index + 1}" aria-current="${index === appPage ? 'page' : 'false'}"></button>`
  ).join('');
  return `<div class="applications-overview">${renderAppGrid(pageApps)}<div class="app-pages">${pages}</div></div>`;
}

function renderDash() {
  const favorites = [
    ['files', 'Files', 'F'],
    ['terminal', 'Terminal', 'T'],
    ['browser', 'Web Browser', 'W']
  ].map(([query, label, fallback]) => {
    const entry = findFavorite(query);
    const target = entry
      ? `data-app-id="${escapeHtml(entry.appId)}"`
      : `data-search="${escapeHtml(query)}"`;
    const running = entry && appLooksRunning(entry) ? ' is-running' : '';
    return `<button class="dash-button dash-favorite${running}" ${target} title="${escapeHtml(entry?.name || label)}"><span class="dash-icon">${entry ? nativeAppIconMarkup(entry, fallback) : escapeHtml(fallback)}</span></button>`;
  }).join('');
  return `${favorites}<span class="dash-separator"></span><button id="show-apps" class="dash-button" title="Show Applications">${iconGrid()}</button>`;
}

function refreshDash() {
  const dash = document.querySelector('#dash');
  if (!dash) return;
  dash.innerHTML = renderDash();
  bindDash();
  hydrateRenderedAppIcons();
}

function bindDash() {
  document.querySelectorAll('#dash [data-app-id]').forEach(button => button.addEventListener('click', async () => {
    await call('launch_app', { appId: button.dataset.appId });
    await call('toggle_activities');
  }));
  document.querySelector('#show-apps')?.addEventListener('click', () => {
    activitiesMode = activitiesMode === 'apps' ? 'windows' : 'apps';
    appPage = 0;
    const search = document.querySelector('#search');
    if (search) search.value = '';
    refreshActivitiesContent();
  });
  document.querySelectorAll('[data-search]').forEach(button => button.addEventListener('click', () => {
    const search = document.querySelector('#search');
    if (!search) return;
    search.value = button.dataset.search;
    search.focus();
    refreshActivitiesContent(search.value);
  }));
}

function bindActivitiesContent() {
  const content = document.querySelector('#activities-content');
  if (!content) return;
  content.querySelectorAll('[data-window]').forEach(button => button.addEventListener('click', async () => {
    await call('activate_window', { handle: button.dataset.window });
    await call('toggle_activities');
  }));
  content.querySelectorAll('[data-close-window]').forEach(button => button.addEventListener('click', async event => {
    event.stopPropagation();
    await call('close_window', { handle: button.dataset.closeWindow });
    setTimeout(refreshNativeWindows, 120);
  }));
  content.querySelectorAll('[data-workspace-direction]').forEach(button => button.addEventListener('click', async () => {
    const direction = Number(button.dataset.workspaceDirection);
    if (direction !== -1 && direction !== 1) return;
    await call('clear_window_thumbnails');
    await call('navigate_workspace', { direction });
  }));
  content.querySelectorAll('[data-app-id]').forEach(button => button.addEventListener('click', async () => {
    await call('launch_app', { appId: button.dataset.appId });
    await call('toggle_activities');
  }));
  content.querySelectorAll('[data-app-page]').forEach(button => button.addEventListener('click', () => {
    appPage = Number(button.dataset.appPage) || 0;
    refreshActivitiesContent();
  }));
}

function refreshActivitiesContent(query = '') {
  const content = document.querySelector('#activities-content');
  if (!content) return;
  const q = query.trim().toLowerCase();
  if (q) {
    const filtered = searchApps(q);
    content.innerHTML = `<div class="search-results"><div class="results-label">Applications</div>${filtered.length ? renderAppGrid(filtered) : `<div class="search-empty" role="status"><span class="search-empty__symbol" aria-hidden="true">⌕</span><strong>${appsLoaded ? 'No matching applications' : 'Application discovery unavailable'}</strong><small>${appsLoaded ? `No installed app matches “${escapeHtml(query)}”.` : 'Windows has not returned the installed apps yet.'}</small></div>`}</div>`;
  } else {
    content.innerHTML = activitiesMode === 'apps' ? renderAppDrawer() : renderWindowOverview();
  }
  document.querySelector('#show-apps')?.classList.toggle('is-active', activitiesMode === 'apps' && !q);
  bindActivitiesContent();
  hydrateRenderedAppIcons();
  requestAnimationFrame(() => requestAnimationFrame(syncLiveThumbnails));
}

async function renderActivities() {
  app.innerHTML = `
    <section class="activities">
      <div class="search-shell"><span class="search-icon">${ICONS.search}</span><input id="search" class="search" placeholder="Type to search" autocomplete="off" spellcheck="false" /></div>
      <div id="activities-content" class="activities-content"><div class="workspace-preflight"><div class="workspace-preflight__frame" aria-hidden="true"><span class="workspace-preflight__window workspace-preflight__window--one"></span><span class="workspace-preflight__window workspace-preflight__window--two"></span><span class="workspace-preflight__window workspace-preflight__window--three"></span></div></div></div>
      <div class="dash-wrap"><div id="dash" class="dash"></div></div>
    </section>`;
  const search = document.querySelector('#search');
  search.addEventListener('input', () => {
    appPage = 0;
    refreshActivitiesContent(search.value);
  });
  search.addEventListener('keydown', async event => {
    if (event.key === 'Escape') {
      event.preventDefault();
      if (search.value) {
        // GNOME overview: first Escape clears a search; another exits Activities.
        search.value = '';
        appPage = 0;
        refreshActivitiesContent();
        return;
      }
      await call('toggle_activities');
      return;
    }
    if (event.key === 'Enter' && search.value.trim()) {
      const first = searchApps(search.value)[0];
      if (!first) return;
      event.preventDefault();
      await call('launch_app', { appId: first.appId });
      await call('toggle_activities');
      return;
    }
    if (!search.value.trim() && activitiesMode === 'apps' && (event.key === 'ArrowLeft' || event.key === 'ArrowRight')) {
      event.preventDefault();
      appPage += event.key === 'ArrowRight' ? 1 : -1;
      refreshActivitiesContent();
    }
  });
  await loadActivitiesData();
  refreshDash();
  if (listen && !windowEventsBound) {
    windowEventsBound = true;
    try {
      await withTimeout(listen('fedorawin://windows-changed', refreshNativeWindows), 2500, null);
    } catch {
      windowEventsBound = false;
    }
  }
  if (captureMode === 'apps') activitiesMode = 'apps';
  if (captureMode === 'search-terminal') {
    // Capture real installed search results, not a mock Terminal card. The
    // Windows CI image may not have Windows Terminal or Command Prompt.
    const target = searchApps('terminal')[0] || findFavorite('files') || apps[0];
    search.value = target ? target.name : 'terminal';
  }
  refreshActivitiesContent(search.value);
  if (captureEvidence) await withTimeout(hydrateRenderedAppIcons(), 3500, null);
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

function syncVolumeInput(input, value) {
  const normalized = Math.max(0, Math.min(100, Math.round(Number(value) || 0)));
  quickState.volume = normalized;
  input.value = String(normalized);
  input.style.setProperty('--value', `${normalized}%`);
}

async function hydrateMasterVolume(input) {
  const revision = volumeRevision;
  const value = await call('get_master_volume');
  if (revision === volumeRevision && Number.isFinite(value)) syncVolumeInput(input, value);
}

function bindMasterVolume() {
  const input = document.querySelector('#volume');
  if (!input) return;
  input.addEventListener('input', () => {
    const requested = Math.max(0, Math.min(100, Math.round(Number(input.value) || 0)));
    quickState.volume = requested;
    volumeRevision += 1;
    const revision = volumeRevision;
    clearTimeout(volumeCommitTimer);
    volumeCommitTimer = setTimeout(async () => {
      const actual = await call('set_master_volume', { value: requested });
      if (revision === volumeRevision && Number.isFinite(actual)) syncVolumeInput(input, actual);
    }, 80);
  });
  hydrateMasterVolume(input);
}

function renderBatterySummary() {
  const power = quickState.power;
  if (!power) return '<span id="battery-summary" class="battery-summary"><span class="battery-mark"><i></i></span><strong>—</strong></span>';
  if (!power.batteryPresent || !Number.isFinite(power.batteryPercent)) {
    return '<span id="battery-summary" class="battery-summary battery-summary--ac"><strong>AC</strong></span>';
  }
  const percent = Math.max(0, Math.min(100, Math.round(power.batteryPercent)));
  const state = power.charging ? 'Charging' : (power.acOnline ? 'Plugged in' : 'On battery');
  return `<span id="battery-summary" class="battery-summary" title="${state}"><span class="battery-mark"><i style="width:${percent}%"></i></span><strong>${percent}%</strong></span>`;
}

async function hydratePowerStatus() {
  const power = await call('get_power_status');
  if (!power || typeof power !== 'object') return;
  quickState.power = power;
  const current = document.querySelector('#battery-summary');
  if (!current) return;
  const wrapper = document.createElement('div');
  wrapper.innerHTML = renderBatterySummary();
  current.replaceWith(wrapper.firstElementChild);
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
      <div class="quick-header">${renderBatterySummary()}<span class="header-actions"><button id="screenshot-open" class="icon-button" title="Screenshot">${ICONS.screenshot}</button><button id="appearance-open" class="icon-button" title="Appearance">${ICONS.settings}</button><button class="icon-button" title="Power">${ICONS.power}</button></span></div>
      <div class="sliders">${slider(ICONS.volume,'volume',quickState.volume,'Volume')}${slider(ICONS.brightness,'brightness',quickState.brightness,'Brightness')}</div>
      <div class="quick-grid">
        ${quickTile('wifi',ICONS.wifi,'Wi-Fi','Connected',true,true)}
        ${quickTile('bluetooth',ICONS.bluetooth,'Bluetooth','On',false,true)}
        ${quickTile('power-mode',ICONS.power,'Power Mode','Balanced',false,true)}
        ${quickTile('dark-style',ICONS.brightness,'Dark Style',shell.appearance.theme === 'dark' ? 'On' : 'Off',shell.appearance.theme === 'dark')}
        ${quickTile('night-light',ICONS.brightness,'Night Light','Off',false)}
        ${quickTile('radio-pause',ICONS.wifi,'Wireless Pause','Wi-Fi + Bluetooth',false,true)}
      </div>
      <button class="background-apps"><span>Background Apps</span><span>0</span></button>`}
  </div></section>`;

  if (appearanceOpen) {
    document.querySelector('#appearance-back').addEventListener('click', () => renderQuickSettings(false));
    bindAppearance();
    return;
  }
  document.querySelectorAll('input[type="range"]').forEach(bindRangeFill);
  bindMasterVolume();
  hydratePowerStatus();
  document.querySelector('#screenshot-open').addEventListener('click', () => call('open_screenshot_overlay'));
  document.querySelector('#appearance-open').addEventListener('click', () => renderQuickSettings(true));
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
  try {
    shell = await withTimeout(call('get_shell_state'), 3000, null) || shell;
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

  } finally {
    await signalCaptureReady();
  }
}
bootstrap();
