(() => {
  const invoke = window.__TAURI__?.core?.invoke;
  let revision = 0;
  let commitTimer = 0;

  async function call(command, payload = {}) {
    if (!invoke) return null;
    try {
      return await invoke(command, payload);
    } catch {
      return null;
    }
  }

  function syncInput(input, value) {
    const normalized = Math.max(0, Math.min(100, Math.round(Number(value) || 0)));
    input.value = String(normalized);
    input.style.setProperty('--value', `${normalized}%`);
  }

  function setSupported(input, supported) {
    input.dataset.nativeBrightness = supported ? 'true' : 'false';
    input.disabled = !supported;
    input.title = supported
      ? 'Internal display brightness'
      : 'Brightness control is unavailable for this display';
  }

  async function hydrateBrightness(input) {
    const observedRevision = revision;
    const status = await call('get_brightness_status');
    if (observedRevision !== revision || !status || typeof status !== 'object') return;
    const supported = status.supported === true && Number.isFinite(status.value);
    setSupported(input, supported);
    if (supported) syncInput(input, status.value);
  }

  function bindBrightness() {
    const input = document.querySelector('#brightness');
    if (!input || input.dataset.brightnessBound === 'true') return;
    input.dataset.brightnessBound = 'true';

    input.addEventListener('input', () => {
      if (input.dataset.nativeBrightness !== 'true') return;
      const requested = Math.max(0, Math.min(100, Math.round(Number(input.value) || 0)));
      revision += 1;
      const requestedRevision = revision;
      syncInput(input, requested);
      clearTimeout(commitTimer);
      commitTimer = setTimeout(async () => {
        const actual = await call('set_brightness', { value: requested });
        if (requestedRevision !== revision) return;
        if (Number.isFinite(actual)) syncInput(input, actual);
        else if (invoke) await hydrateBrightness(input);
      }, 80);
    });

    hydrateBrightness(input);
  }

  const observer = new MutationObserver(bindBrightness);
  observer.observe(document.documentElement, { childList: true, subtree: true });
  bindBrightness();
})();
