(() => {
  const CLOSE_DELAY_MS = 120;

  function tileFor(flyout) {
    const tileId = flyout?.dataset?.ownerTile;
    return tileId ? document.getElementById(tileId) : null;
  }

  function clearExpanded(flyout) {
    tileFor(flyout)?.setAttribute('aria-expanded', 'false');
  }

  function removeImmediately(flyout) {
    if (!flyout) return;
    clearExpanded(flyout);
    flyout.remove();
  }

  function close(id) {
    const flyout = document.getElementById(id);
    if (!flyout || flyout.dataset.closing === 'true') return;
    flyout.dataset.closing = 'true';
    flyout.classList.add('is-closing');
    clearExpanded(flyout);
    setTimeout(() => flyout.remove(), CLOSE_DELAY_MS);
  }

  function closeOthers(exceptId) {
    document.querySelectorAll('.control-flyout').forEach(flyout => {
      if (flyout.id !== exceptId) removeImmediately(flyout);
    });
  }

  function header(title, subtitle, closeLabel) {
    return `
      <div class="control-flyout__header">
        <div>
          <strong>${title}</strong>
          <small>${subtitle}</small>
        </div>
        <button class="control-flyout__close" type="button" aria-label="${closeLabel}">×</button>
      </div>`;
  }

  function open({
    id,
    tileId,
    modifier = '',
    ariaLabel,
    title,
    subtitle,
    closeLabel,
    bodyHtml,
    focusSelector = '',
    onMount
  }) {
    if (!id || !tileId) return null;
    const existing = document.getElementById(id);
    if (existing) return existing;

    const card = document.querySelector('.quick-card');
    const tile = document.getElementById(tileId);
    if (!card || !tile) return null;

    closeOthers(id);

    const flyout = document.createElement('section');
    flyout.id = id;
    flyout.className = ['control-flyout', modifier].filter(Boolean).join(' ');
    flyout.dataset.ownerTile = tileId;
    flyout.setAttribute('role', 'dialog');
    flyout.setAttribute('aria-modal', 'false');
    flyout.setAttribute('aria-label', ariaLabel || title);
    flyout.innerHTML =
      header(title, subtitle, closeLabel || `Close ${title}`) +
      bodyHtml;

    card.appendChild(flyout);
    tile.setAttribute('aria-expanded', 'true');

    flyout.querySelector('.control-flyout__close')
      ?.addEventListener('click', () => close(id));

    flyout.addEventListener('keydown', event => {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      event.stopPropagation();
      close(id);
      tile.focus();
    });

    if (typeof onMount === 'function') onMount(flyout);

    if (focusSelector) {
      requestAnimationFrame(() => flyout.querySelector(focusSelector)?.focus());
    }

    return flyout;
  }

  function toggle(config) {
    const existing = document.getElementById(config.id);
    if (existing) {
      close(config.id);
      return null;
    }
    return open(config);
  }

  window.FedoraWinControlFlyouts = Object.freeze({
    open,
    close,
    toggle
  });
})();
