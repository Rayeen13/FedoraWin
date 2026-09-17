(() => {
  const invoke = window.__TAURI__?.core?.invoke;
  const MODES = ['bestEfficiency', 'balanced', 'bestPerformance'];
  const LABELS = {
    bestEfficiency: 'Best Power Efficiency',
    balanced: 'Balanced',
    bestPerformance: 'Best Performance'
  };

  let currentMode = 'balanced';
  let revision = 0;

  async function call(command, payload = {}) {
    if (!invoke) return null;
    try {
      return await invoke(command, payload);
    } catch {
      return null;
    }
  }

  function updateTile(mode) {
    if (!MODES.includes(mode)) return;
    currentMode = mode;
    const tile = document.querySelector('#power-mode');
    if (!tile) return;
    const subtitle = tile.querySelector('small');
    if (subtitle) subtitle.textContent = LABELS[mode];
    tile.dataset.powerMode = mode;
    tile.setAttribute('title', `Power Mode: ${LABELS[mode]}`);
  }

  async function hydratePowerMode() {
    const observedRevision = revision;
    const mode = await call('get_power_mode');
    if (observedRevision === revision && MODES.includes(mode)) updateTile(mode);
  }

  async function cyclePowerMode() {
    const requestedRevision = ++revision;
    const index = Math.max(0, MODES.indexOf(currentMode));
    const requested = MODES[(index + 1) % MODES.length];
    updateTile(requested);
    const actual = await call('set_power_mode', { mode: requested });
    if (requestedRevision !== revision) return;
    if (MODES.includes(actual)) updateTile(actual);
    else if (invoke) await hydratePowerMode();
  }

  function bindPowerModeTile() {
    const tile = document.querySelector('#power-mode');
    if (!tile || tile.dataset.powerModeBound === 'true') return;
    tile.dataset.powerModeBound = 'true';
    tile.addEventListener('click', cyclePowerMode);

    const more = tile.closest('.quick-item')?.querySelector('.quick-more');
    if (more) {
      more.setAttribute('aria-label', 'Cycle Power Mode');
      more.addEventListener('click', cyclePowerMode);
    }
    hydratePowerMode();
  }

  const observer = new MutationObserver(bindPowerModeTile);
  observer.observe(document.documentElement, { childList: true, subtree: true });
  bindPowerModeTile();
})();
