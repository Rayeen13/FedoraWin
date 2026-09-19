(() => {
  const invoke = window.__TAURI__?.core?.invoke;
  const Flyouts = window.FedoraWinControlFlyouts;
  const captureMode = new URLSearchParams(location.search).get('capture') || '';
  let current = {
    supported: false,
    enabled: false,
    connected: false,
    networkName: '',
    interfaceName: ''
  };
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

  function escapeHtml(value = '') {
    return String(value).replace(/[&<>'"]/g, ch => ({
      '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;'
    }[ch]));
  }

  function subtitle(status) {
    if (!status.supported) return 'Unavailable';
    if (!status.enabled) return 'Off';
    if (status.connected) return status.networkName || 'Connected';
    return 'On';
  }

  function detail(status) {
    if (!status.supported) return 'No Windows WLAN interface detected';
    if (!status.enabled) return 'Wi-Fi radio is off';
    if (status.connected) return status.networkName || 'Connected network';
    return 'Not connected';
  }

  function updateTile(status) {
    if (!status || typeof status !== 'object') return;
    current = {
      supported: status.supported === true,
      enabled: status.enabled === true,
      connected: status.connected === true,
      networkName: status.networkName || '',
      interfaceName: status.interfaceName || ''
    };

    const tile = document.querySelector('#wifi');
    if (tile) {
      tile.setAttribute('aria-pressed', String(current.enabled));
      tile.setAttribute('aria-disabled', String(!current.supported));
      tile.dataset.wifiConnected = String(current.connected);
      const text = tile.querySelector('small');
      if (text) text.textContent = subtitle(current);
      tile.title = `Wi-Fi: ${subtitle(current)}`;
    }

    const flyout = document.querySelector('#wifi-flyout');
    if (flyout) {
      const state = flyout.querySelector('[data-wifi-state]');
      const description = flyout.querySelector('[data-wifi-detail]');
      const adapter = flyout.querySelector('[data-wifi-adapter]');
      const toggle = flyout.querySelector('[data-wifi-toggle]');
      if (state) state.textContent = subtitle(current);
      if (description) description.textContent = detail(current);
      if (adapter) adapter.textContent = current.interfaceName || 'Windows WLAN adapter';
      if (toggle) {
        toggle.disabled = !current.supported;
        toggle.setAttribute('aria-pressed', String(current.enabled));
        const title = toggle.querySelector('strong');
        const hint = toggle.querySelector('small');
        if (title) title.textContent = current.enabled ? 'Turn Off' : 'Turn On';
        if (hint) hint.textContent = current.enabled ? 'Disable the Wi-Fi radio' : 'Enable the Wi-Fi radio';
      }
    }
  }

  async function hydrate() {
    const observedRevision = revision;
    const status = await call('get_wifi_status');
    if (observedRevision === revision && status) updateTile(status);
  }

  async function setEnabled(enabled) {
    const requestedRevision = ++revision;
    updateTile({ ...current, enabled, connected: enabled ? current.connected : false });
    const status = await call('set_wifi_enabled', { enabled });
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
    Flyouts?.close('wifi-flyout');
  }

  function renderFlyout() {
    if (!Flyouts) return;
    const existing = document.querySelector('#wifi-flyout');
    if (existing) {
      closeFlyout();
      return;
    }

    Flyouts.open({
      id: 'wifi-flyout',
      tileId: 'wifi',
      modifier: 'control-flyout--wifi',
      ariaLabel: 'Wi-Fi',
      title: 'Wi-Fi',
      subtitle: 'Windows WLAN control',
      closeLabel: 'Close Wi-Fi',
      bodyHtml: `
        <div class="control-flyout__options">
          <div class="control-detail">
            <span class="control-detail__indicator" aria-hidden="true"></span>
            <span class="control-option__copy">
              <strong data-wifi-state>${escapeHtml(subtitle(current))}</strong>
              <small data-wifi-detail>${escapeHtml(detail(current))}</small>
            </span>
          </div>
          <div class="control-detail control-detail--secondary">
            <span class="control-detail__indicator" aria-hidden="true"></span>
            <span class="control-option__copy">
              <strong>Adapter</strong>
              <small data-wifi-adapter>${escapeHtml(current.interfaceName || 'Windows WLAN adapter')}</small>
            </span>
          </div>
          <button class="control-option" type="button" data-wifi-toggle aria-pressed="${current.enabled}" ${current.supported ? '' : 'disabled'}>
            <span class="control-option__indicator" aria-hidden="true"></span>
            <span class="control-option__copy">
              <strong>${current.enabled ? 'Turn Off' : 'Turn On'}</strong>
              <small>${current.enabled ? 'Disable the Wi-Fi radio' : 'Enable the Wi-Fi radio'}</small>
            </span>
          </button>
        </div>`,
      focusSelector: '[data-wifi-toggle]',
      onMount(flyout) {
        flyout.querySelector('[data-wifi-toggle]')?.addEventListener('click', toggle);
      }
    });
  }

  function bind() {
    const tile = document.querySelector('#wifi');
    if (!tile || tile.dataset.wifiBound === 'true') return;
    tile.dataset.wifiBound = 'true';
    tile.setAttribute('aria-haspopup', 'dialog');
    tile.setAttribute('aria-expanded', 'false');
    tile.addEventListener('click', toggle);

    const more = tile.closest('.quick-item')?.querySelector('.quick-more');
    if (more) {
      more.setAttribute('aria-label', 'Wi-Fi options');
      more.setAttribute('aria-haspopup', 'dialog');
      more.addEventListener('click', renderFlyout);
    }

    hydrate();

    if (captureMode === 'wifi' && !captureFlyoutOpened) {
      captureFlyoutOpened = true;
      requestAnimationFrame(renderFlyout);
    }
  }

  window.addEventListener('fedorawin:radios-changed', hydrate);
  const observer = new MutationObserver(bind);
  observer.observe(document.documentElement, { childList: true, subtree: true });
  bind();
})();
