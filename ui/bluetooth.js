(() => {
  const invoke = window.__TAURI__?.core?.invoke;
  const Flyouts = window.FedoraWinControlFlyouts;
  let current = { supported: false, enabled: false, name: '', state: 'unavailable' };
  let revision = 0;

  async function call(command, payload = {}) {
    if (!invoke) return null;
    try {
      return await invoke(command, payload);
    } catch {
      return null;
    }
  }

  function escapeHtml(value = '') {
    return String(value).replace(/[&<>'"]/g, ch => ({
      '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;'
    }[ch]));
  }

  function labelFor(status) {
    if (!status?.supported) return 'Unavailable';
    if (status.state === 'disabled') return 'Disabled by system';
    return status.enabled ? 'On' : 'Off';
  }

  function updateTile(status) {
    if (!status || typeof status !== 'object') return;
    current = {
      supported: status.supported === true,
      enabled: status.enabled === true,
      name: status.name || '',
      state: status.state || 'unknown'
    };

    const tile = document.querySelector('#bluetooth');
    if (tile) {
      tile.setAttribute('aria-pressed', String(current.enabled));
      tile.setAttribute('aria-disabled', String(!current.supported));
      tile.dataset.bluetoothState = current.state;
      const subtitle = tile.querySelector('small');
      if (subtitle) subtitle.textContent = labelFor(current);
      tile.title = current.name
        ? `Bluetooth: ${labelFor(current)} · ${current.name}`
        : `Bluetooth: ${labelFor(current)}`;
    }

    const flyout = document.querySelector('#bluetooth-flyout');
    if (flyout) {
      const state = flyout.querySelector('[data-bluetooth-state]');
      const detail = flyout.querySelector('[data-bluetooth-detail]');
      const toggle = flyout.querySelector('[data-bluetooth-toggle]');
      if (state) state.textContent = labelFor(current);
      if (detail) detail.textContent = current.name || (current.supported ? 'Windows Bluetooth radio' : 'No Bluetooth radio detected');
      if (toggle) {
        toggle.disabled = !current.supported;
        toggle.setAttribute('aria-pressed', String(current.enabled));
        const title = toggle.querySelector('strong');
        const hint = toggle.querySelector('small');
        if (title) title.textContent = current.enabled ? 'Turn Off' : 'Turn On';
        if (hint) hint.textContent = current.enabled ? 'Disable the Bluetooth radio' : 'Enable the Bluetooth radio';
      }
    }
  }

  async function hydrate() {
    const observedRevision = revision;
    const status = await call('get_bluetooth_status');
    if (observedRevision === revision && status) updateTile(status);
  }

  async function setEnabled(enabled) {
    const requestedRevision = ++revision;
    updateTile({ ...current, enabled });
    const status = await call('set_bluetooth_enabled', { enabled });
    if (requestedRevision !== revision) return;
    if (status) updateTile(status);
    else if (invoke) await hydrate();
  }

  async function toggle() {
    if (!current.supported) {
      await hydrate();
      if (!current.supported) return;
    }
    await setEnabled(!current.enabled);
  }

  function closeFlyout() {
    Flyouts?.close('bluetooth-flyout');
  }

  function renderFlyout() {
    if (!Flyouts) return;
    const existing = document.querySelector('#bluetooth-flyout');
    if (existing) {
      closeFlyout();
      return;
    }

    Flyouts.open({
      id: 'bluetooth-flyout',
      tileId: 'bluetooth',
      modifier: 'control-flyout--bluetooth',
      ariaLabel: 'Bluetooth',
      title: 'Bluetooth',
      subtitle: 'Windows radio control',
      closeLabel: 'Close Bluetooth',
      bodyHtml: `
        <div class="control-flyout__options">
          <div class="control-detail">
            <span class="control-detail__indicator" aria-hidden="true"></span>
            <span class="control-option__copy">
              <strong data-bluetooth-state>${escapeHtml(labelFor(current))}</strong>
              <small data-bluetooth-detail>${escapeHtml(current.name || (current.supported ? 'Windows Bluetooth radio' : 'No Bluetooth radio detected'))}</small>
            </span>
          </div>
          <button class="control-option" type="button" data-bluetooth-toggle aria-pressed="${current.enabled}" ${current.supported ? '' : 'disabled'}>
            <span class="control-option__indicator" aria-hidden="true"></span>
            <span class="control-option__copy">
              <strong>${current.enabled ? 'Turn Off' : 'Turn On'}</strong>
              <small>${current.enabled ? 'Disable the Bluetooth radio' : 'Enable the Bluetooth radio'}</small>
            </span>
          </button>
        </div>`,
      focusSelector: '[data-bluetooth-toggle]',
      onMount(flyout) {
        flyout.querySelector('[data-bluetooth-toggle]')?.addEventListener('click', toggle);
      }
    });
  }

  function bind() {
    const tile = document.querySelector('#bluetooth');
    if (!tile || tile.dataset.bluetoothBound === 'true') return;
    tile.dataset.bluetoothBound = 'true';
    tile.setAttribute('aria-haspopup', 'dialog');
    tile.setAttribute('aria-expanded', 'false');
    tile.addEventListener('click', toggle);

    const more = tile.closest('.quick-item')?.querySelector('.quick-more');
    if (more) {
      more.setAttribute('aria-label', 'Bluetooth options');
      more.setAttribute('aria-haspopup', 'dialog');
      more.addEventListener('click', renderFlyout);
    }

    hydrate();
  }

  const observer = new MutationObserver(bind);
  observer.observe(document.documentElement, { childList: true, subtree: true });
  bind();
})();
