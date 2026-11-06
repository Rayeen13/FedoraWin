const invoke = window.__TAURI__?.core?.invoke;
const listen = window.__TAURI__?.event?.listen;
const root = document.querySelector('#osd-root');
const params = new URLSearchParams(location.search);

const ACCENTS = {
  blue: '#3584e4', teal: '#2190a4', green: '#3a944a', yellow: '#c88800',
  orange: '#ed5b00', red: '#e62d42', pink: '#d56199', purple: '#9141ac', slate: '#6f8396'
};

const ICONS = {
  volume: '<svg viewBox="0 0 24 24"><path d="M4 10h4l5-4v12l-5-4H4v-4Z"/><path d="M16 9c1.7 1.7 1.7 4.3 0 6M18.5 6.5c3.2 3.2 3.2 7.8 0 11"/></svg>',
  brightness: '<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="3.5"/><path d="M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9 7 7M17 17l2.1 2.1M19.1 4.9 17 7M7 17l-2.1 2.1"/></svg>',
  power: '<svg viewBox="0 0 24 24"><path d="M12 3v8M7 5.8a8 8 0 1 0 10 0"/></svg>',
  media: '<svg viewBox="0 0 24 24"><path d="M6 5v14l12-7-12-7Z"/></svg>'
};

function escapeHtml(value = '') {
  return String(value).replace(/[&<>'"]/g, ch => ({'&':'&amp;','<':'&lt;','>':'&gt;',"'":'&#39;','"':'&quot;'}[ch]));
}

function normalizePayload(payload = {}) {
  const kind = ['volume', 'brightness', 'power', 'media'].includes(payload.kind) ? payload.kind : 'volume';
  const value = Number.isFinite(Number(payload.value)) && payload.value !== ''
    ? Math.max(0, Math.min(100, Math.round(Number(payload.value))))
    : null;
  return {
    kind,
    value,
    detail: payload.detail || ''
  };
}

function fromQuery() {
  return normalizePayload({
    kind: params.get('kind') || 'volume',
    value: params.get('value'),
    detail: params.get('detail') || ''
  });
}

function titleFor(kind) {
  return {
    volume: 'Volume',
    brightness: 'Brightness',
    power: 'Power Mode',
    media: 'Media'
  }[kind] || 'Control';
}

function detailFor(payload) {
  if (payload.kind === 'power') {
    return {
      bestEfficiency: 'Power Saver',
      balanced: 'Balanced',
      bestPerformance: 'Performance'
    }[payload.detail] || 'Balanced';
  }
  if (payload.kind === 'media') return payload.detail || 'Now Playing';
  return payload.value === null ? '' : `${payload.value}%`;
}

function render(payloadLike) {
  const payload = normalizePayload(payloadLike);
  const detail = detailFor(payload);
  const hasMeter = payload.value !== null && payload.kind !== 'power' && payload.kind !== 'media';
  root.innerHTML = `
    <section class="control-osd control-osd--${payload.kind}" role="status" aria-label="${escapeHtml(titleFor(payload.kind))} ${escapeHtml(detail)}">
      <span class="control-osd__icon" aria-hidden="true">${ICONS[payload.kind]}</span>
      <span class="control-osd__content">
        <strong>${escapeHtml(titleFor(payload.kind))}</strong>
        ${hasMeter ? `<span class="control-osd__meter" aria-hidden="true"><i style="width:${payload.value}%"></i></span>` : `<small>${escapeHtml(detail)}</small>`}
      </span>
      ${hasMeter ? `<span class="control-osd__value">${escapeHtml(detail)}</span>` : ''}
    </section>`;
  document.body.classList.remove('osd-pulse');
  requestAnimationFrame(() => document.body.classList.add('osd-pulse'));
}

async function applyAppearance() {
  if (!invoke) return;
  try {
    const shell = await invoke('get_shell_state');
    const appearance = shell?.appearance || {};
    const theme = appearance.theme === 'system'
      ? (matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark')
      : (appearance.theme || 'dark');
    document.documentElement.dataset.theme = theme;
    document.documentElement.style.setProperty('--accent', ACCENTS[appearance.accent] || ACCENTS.blue);
  } catch {
    // Keep the CSS defaults if shell state is unavailable.
  }
}

async function bootstrap() {
  await applyAppearance();
  render(fromQuery());
  if (!listen) return;
  try {
    await listen('fedorawin://osd', event => render(event.payload));
  } catch {
    // Initial query rendering still provides a usable one-shot OSD.
  }
}

bootstrap();
