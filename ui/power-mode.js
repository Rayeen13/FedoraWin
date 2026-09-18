(() => {
  const invoke = window.__TAURI__?.core?.invoke;
  const Flyouts = window.FedoraWinControlFlyouts;
  const captureMode = new URLSearchParams(location.search).get('capture') || '';
  const MODES = ['bestEfficiency', 'balanced', 'bestPerformance'];
  const LABELS = {
    bestEfficiency: 'Best Power Efficiency',
    balanced: 'Balanced',
    bestPerformance: 'Best Performance'
  };
  const DESCRIPTIONS = {
    bestEfficiency: 'Lower energy use and quieter thermals.',
    balanced: 'Balances responsiveness and power use.',
    bestPerformance: 'Prioritizes performance when you need it.'
  };

  let currentMode = 'balanced';
  let revision = 0;
  let captureFlyoutOpened = false;

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
    if (tile) {
      const subtitle = tile.querySelector('small');
      if (subtitle) subtitle.textContent = LABELS[mode];
      tile.dataset.powerMode = mode;
      tile.setAttribute('title', `Power Mode: ${LABELS[mode]}`);
    }
    document.querySelectorAll('[data-power-mode-option]').forEach(option => {
      const selected = option.dataset.powerModeOption === mode;
      option.setAttribute('aria-checked', String(selected));
      option.classList.toggle('is-selected', selected);
    });
  }

  async function hydratePowerMode() {
    const observedRevision = revision;
    const mode = await call('get_power_mode');
    if (observedRevision === revision && MODES.includes(mode)) updateTile(mode);
  }

  async function setPowerMode(mode) {
    if (!MODES.includes(mode)) return;
    const requestedRevision = ++revision;
    updateTile(mode);
    const actual = await call('set_power_mode', { mode });
    if (requestedRevision !== revision) return;
    if (MODES.includes(actual)) updateTile(actual);
    else if (invoke) await hydratePowerMode();
  }

  async function cyclePowerMode() {
    const index = Math.max(0, MODES.indexOf(currentMode));
    await setPowerMode(MODES[(index + 1) % MODES.length]);
  }

  function closeFlyout() {
    Flyouts?.close('power-mode-flyout');
  }

  function renderFlyout() {
    const existing = document.querySelector('#power-mode-flyout');
    if (existing) {
      if (captureMode !== 'power-mode') closeFlyout();
      return;
    }
    if (!Flyouts) return;

    Flyouts.open({
      id: 'power-mode-flyout',
      tileId: 'power-mode',
      modifier: 'control-flyout--power',
      ariaLabel: 'Power Mode',
      title: 'Power Mode',
      subtitle: 'Windows power behavior',
      closeLabel: 'Close Power Mode',
      bodyHtml: `
        <div class="control-flyout__options" role="radiogroup" aria-label="Power Mode">
          ${MODES.map(mode => `
            <button class="control-option${mode === currentMode ? ' is-selected' : ''}" type="button" role="radio" aria-checked="${mode === currentMode}" data-power-mode-option="${mode}">
              <span class="control-option__indicator" aria-hidden="true"></span>
              <span class="control-option__copy">
                <strong>${LABELS[mode]}</strong>
                <small>${DESCRIPTIONS[mode]}</small>
              </span>
            </button>`).join('')}
        </div>`,
      focusSelector: '[aria-checked="true"]',
      onMount(flyout) {
        flyout.querySelectorAll('[data-power-mode-option]').forEach(option => {
          option.addEventListener('click', async () => {
            await setPowerMode(option.dataset.powerModeOption);
            closeFlyout();
          });
        });
      }
    });
  }

  function bindPowerModeTile() {
    const tile = document.querySelector('#power-mode');
    if (!tile || tile.dataset.powerModeBound === 'true') return;
    tile.dataset.powerModeBound = 'true';
    tile.setAttribute('aria-haspopup', 'dialog');
    tile.setAttribute('aria-expanded', 'false');
    tile.addEventListener('click', cyclePowerMode);

    const more = tile.closest('.quick-item')?.querySelector('.quick-more');
    if (more) {
      more.setAttribute('aria-label', 'Power Mode options');
      more.setAttribute('aria-haspopup', 'dialog');
      more.addEventListener('click', renderFlyout);
    }
    hydratePowerMode();

    if (captureMode === 'power-mode' && !captureFlyoutOpened) {
      captureFlyoutOpened = true;
      requestAnimationFrame(renderFlyout);
    }
  }

  const observer = new MutationObserver(bindPowerModeTile);
  observer.observe(document.documentElement, { childList: true, subtree: true });
  bindPowerModeTile();
})();
