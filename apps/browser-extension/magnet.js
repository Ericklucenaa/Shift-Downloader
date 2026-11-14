// Clique em link magnet: manda para o SHIFT (se a opção estiver ligada).
// Se o SHIFT não responder, o link segue para o programa padrão do Windows.
(() => {
  let enabled = true;
  try {
    chrome.storage.local.get({ openMagnets: true }, (s) => (enabled = s.openMagnets !== false));
    chrome.storage.onChanged.addListener((c) => c.openMagnets && (enabled = c.openMagnets.newValue !== false));
  } catch (e) {
    /* contexto da extensão indisponível */
  }
  const handle = (ev) => {
    if (!enabled || ev.defaultPrevented || ev.button > 1) return;
    const a = ev.target instanceof Element ? ev.target.closest('a[href^="magnet:"]') : null;
    if (!a) return;
    const href = a.href;
    ev.preventDefault();
    ev.stopPropagation();
    chrome.runtime.sendMessage({ type: 'sendMagnet', url: href, referrer: location.href }, (r) => {
      if (chrome.runtime.lastError || !r || !r.ok) location.href = href; // plano B: programa padrão
    });
  };
  document.addEventListener('click', handle, true);
  document.addEventListener('auxclick', handle, true);
})();
