(() => {
  const params = new URLSearchParams(location.search);
  const view = params.get('view') || 'panel';
  if (view !== 'activities') return;

  function replaceLoadingWorkspace() {
    document.querySelectorAll('#activities-content .loading').forEach(node => {
      node.className = 'workspace-preflight';
      node.removeAttribute('aria-live');
      node.innerHTML = `
        <div class="workspace-preflight__frame" aria-hidden="true">
          <span class="workspace-preflight__window workspace-preflight__window--one"></span>
          <span class="workspace-preflight__window workspace-preflight__window--two"></span>
          <span class="workspace-preflight__window workspace-preflight__window--three"></span>
        </div>`;
    });
  }

  const observer = new MutationObserver(replaceLoadingWorkspace);
  observer.observe(document.documentElement, { childList: true, subtree: true });
  replaceLoadingWorkspace();
})();
