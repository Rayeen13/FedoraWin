(() => {
  const invoke = window.__TAURI__?.core?.invoke;
  const Flyouts = window.FedoraWinControlFlyouts;
  const captureMode = new URLSearchParams(location.search).get('capture') || '';
  let current = null;
  let revision = 0;
  let captureOpened = false;

  function escapeHtml(value = '') {
    return String(value).replace(/[&<>'"]/g, character => ({
      '&': '&amp;', '<': '&lt;', '>': '&gt;', "'": '&#39;', '"': '&quot;'
    }[character]));
  }

  async function call(command, payload = {}) {
    if (!invoke) return null;
    try {
      return await invoke(command, payload);
    } catch {
      return null;
    }
  }

  function label(status) {
    if (!status) return 'Checking radios';
    if (status.paused) return 'Paused';
    if (status.partial) return 'Partially paused';
    if (status.managed) return 'Restore pending';
    if (!status.wifi.supported && !status.bluetooth.supported) return 'Unavailable';
    return 'Not paused';
  }

  function radioLabel(radio) {
    if (radio.error) return 'Could not verify';
    if (!radio.supported) return 'Unavailable';
    return radio.enabled ? 'On' : 'Off';
  }

  function update(status) {
    if (!status || typeof status !== 'object') return;
    current = status;
    const tile = document.querySelector('#radio-pause');
    if (tile) {
      tile.setAttribute('aria-pressed', String(status.paused === true));
      tile.dataset.radioPause = status.partial ? 'partial' : status.paused ? 'paused' : 'idle';
      const small = tile.querySelector('small');
      if (small) small.textContent = label(status);
      tile.title = 'Wireless Pause · ' + label(status);
    }
    const flyout = document.querySelector('#radio-pause-flyout');
    if (!flyout) return;
    const state = flyout.querySelector('[data-pause-state]');
    if (state) state.textContent = label(status);
    for (const name of ['wifi', 'bluetooth']) {
      const report = status[name];
      const value = flyout.querySelector(`[data-radio-state="${name}"]`);
      const error = flyout.querySelector(`[data-radio-error="${name}"]`);
      if (value) value.textContent = radioLabel(report);
      if (error) {
        error.textContent = report.error || '';
        error.hidden = !report.error;
      }
    }
    const action = flyout.querySelector('[data-radio-action]');
    if (action) {
      action.disabled = !status.managed && !status.wifi.supported && !status.bluetooth.supported;
      action.textContent = status.managed ? 'Restore previous radio states' : 'Pause supported radios';
    }
  }

  async function hydrate() {
    const observedRevision = revision;
    const status = await call('get_radio_pause_status');
    if (observedRevision === revision && status) update(status);
  }

  async function change() {
    if (!current) await hydrate();
    if (!current) return;
    const action = document.querySelector('[data-radio-action]');
    if (action) action.disabled = true;
    const observedRevision = ++revision;
    const status = await call('set_radio_pause', { paused: !current.managed });
    if (observedRevision !== revision) return;
    if (status) {
      update(status);
      window.dispatchEvent(new Event('fedorawin:radios-changed'));
    } else {
      await hydrate();
      const error = document.querySelector('[data-radio-request-error]');
      if (error) {
        error.hidden = false;
        error.textContent = 'Windows did not complete the request. Check each radio below.';
      }
    }
  }

  function renderFlyout() {
    if (!Flyouts) return;
    if (document.querySelector('#radio-pause-flyout')) {
      Flyouts.close('radio-pause-flyout');
      return;
    }

    Flyouts.open({
      id: 'radio-pause-flyout',
      tileId: 'radio-pause',
      modifier: 'control-flyout--radios',
      ariaLabel: 'Wireless Pause',
      title: 'Wireless Pause',
      subtitle: 'Wi-Fi + Bluetooth only',
      closeLabel: 'Close Wireless Pause',
      bodyHtml: `
        <div class="control-flyout__options">
          <div class="control-detail">
            <span class="control-detail__indicator" aria-hidden="true"></span>
            <span class="control-option__copy">
              <strong data-pause-state>${escapeHtml(label(current))}</strong>
              <small>Not Windows system-wide Airplane Mode</small>
            </span>
          </div>
          <div class="control-detail">
            <span class="control-detail__indicator" aria-hidden="true"></span>
            <span class="control-option__copy">
              <strong>Wi-Fi: <span data-radio-state="wifi">${current ? radioLabel(current.wifi) : 'Checking'}</span></strong>
              <small data-radio-error="wifi" hidden></small>
            </span>
          </div>
          <div class="control-detail">
            <span class="control-detail__indicator" aria-hidden="true"></span>
            <span class="control-option__copy">
              <strong>Bluetooth: <span data-radio-state="bluetooth">${current ? radioLabel(current.bluetooth) : 'Checking'}</span></strong>
              <small data-radio-error="bluetooth" hidden></small>
            </span>
          </div>
          <small class="control-flyout__note">Only supported radios are changed. Cellular and other radios are untouched. Previous states can be restored during this FedoraWin session.</small>
          <small class="control-flyout__error" data-radio-request-error hidden></small>
          <button class="control-option is-selected" type="button" data-radio-action>
            ${current?.managed ? 'Restore previous radio states' : 'Pause supported radios'}
          </button>
        </div>`,
      focusSelector: '[data-radio-action]',
      onMount(flyout) {
        flyout.querySelector('[data-radio-action]')?.addEventListener('click', change);
        if (current) update(current);
        else hydrate();
      }
    });
  }

  function bind() {
    const tile = document.querySelector('#radio-pause');
    if (!tile || tile.dataset.radioPauseBound === 'true') return;
    tile.dataset.radioPauseBound = 'true';
    tile.setAttribute('aria-haspopup', 'dialog');
    tile.setAttribute('aria-expanded', 'false');
    tile.addEventListener('click', renderFlyout);

    const more = tile.closest('.quick-item')?.querySelector('.quick-more');
    if (more) {
      more.setAttribute('aria-label', 'Wireless Pause options');
      more.setAttribute('aria-haspopup', 'dialog');
      more.addEventListener('click', renderFlyout);
    }

    hydrate();
    if (captureMode === 'radio-pause' && !captureOpened) {
      captureOpened = true;
      requestAnimationFrame(renderFlyout);
    }
  }

  const observer = new MutationObserver(bind);
  observer.observe(document.documentElement, { childList: true, subtree: true });
  window.addEventListener('fedorawin:radios-changed', hydrate);
  bind();
})();
