// SHIFT — service worker da extensão.
// Fala com o app pelo Native Messaging (host "app.shift.downloads"). Se o SHIFT não
// responder, NADA é cancelado: o navegador baixa normalmente.

const HOST = 'app.shift.downloads';

const DEFAULTS = {
  intercept: false,
  extensions: 'zip,rar,7z,tar,gz,iso,img,exe,msi,dmg,apk,mp4,mkv,avi,mov,mp3,flac,pdf,epub',
  minSizeMB: 2
};

function send(message) {
  return new Promise((resolve) => {
    try {
      chrome.runtime.sendNativeMessage(HOST, message, (reply) => {
        const err = chrome.runtime.lastError;
        if (err) resolve({ ok: false, error: err.message, hostMissing: /not found/i.test(err.message) });
        else resolve(reply || { ok: false, error: 'sem resposta' });
      });
    } catch (e) {
      resolve({ ok: false, error: String(e) });
    }
  });
}

async function settings() {
  const s = await chrome.storage.local.get(DEFAULTS);
  return { ...DEFAULTS, ...s };
}

function flash(ok) {
  chrome.action.setBadgeBackgroundColor({ color: ok ? '#2e7d4f' : '#c2352b' });
  chrome.action.setBadgeText({ text: ok ? '✓' : '!' });
  setTimeout(() => chrome.action.setBadgeText({ text: '' }), 2500);
}

async function sendToShift(url, kind, extra = {}) {
  const reply = await send({ type: 'add', url, kind, ...extra });
  flash(!!reply.ok);
  return reply;
}

chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.removeAll(() => {
    chrome.contextMenus.create({ id: 'shift-link', title: 'Baixar link com o SHIFT', contexts: ['link'] });
    chrome.contextMenus.create({ id: 'shift-media-src', title: 'Baixar este vídeo/áudio com o SHIFT', contexts: ['video', 'audio'] });
    chrome.contextMenus.create({ id: 'shift-image', title: 'Baixar imagem com o SHIFT', contexts: ['image'] });
    chrome.contextMenus.create({ id: 'shift-page', title: 'Baixar vídeo/áudio desta página com o SHIFT', contexts: ['page'] });
  });
});

const isWeb = (u) => typeof u === 'string' && /^(https?:|magnet:)/i.test(u);

chrome.contextMenus.onClicked.addListener((info, tab) => {
  const referrer = tab && tab.url;
  if (info.menuItemId === 'shift-link' && isWeb(info.linkUrl)) sendToShift(info.linkUrl, 'file', { referrer });
  else if (info.menuItemId === 'shift-image' && isWeb(info.srcUrl)) sendToShift(info.srcUrl, 'file', { referrer });
  else if (info.menuItemId === 'shift-media-src') {
    // Vídeos de sites (streaming) quase sempre precisam da página, não do src (blob:).
    const url = isWeb(info.srcUrl) && !/^blob:/i.test(info.srcUrl) ? info.srcUrl : info.pageUrl;
    if (isWeb(url)) sendToShift(url, url === info.pageUrl ? 'media' : 'file', { referrer });
  } else if (info.menuItemId === 'shift-page' && isWeb(info.pageUrl)) sendToShift(info.pageUrl, 'media', { referrer });
});

// Interceptação seletiva: só com a opção ligada, só links http(s), só tipos/tamanhos escolhidos.
chrome.downloads.onCreated.addListener(async (item) => {
  const s = await settings();
  if (!s.intercept) return;
  const url = item.finalUrl || item.url;
  if (!/^https?:/i.test(url)) return; // blob:, data:, file: ficam com o navegador
  const name = (item.filename || url.split('?')[0].split('/').pop() || '').toLowerCase();
  const ext = name.includes('.') ? name.split('.').pop() : '';
  const wanted = s.extensions.split(',').map((x) => x.trim().toLowerCase()).filter(Boolean);
  const bigEnough = item.fileSize > 0 ? item.fileSize >= s.minSizeMB * 1024 * 1024 : true;
  if (!(wanted.includes(ext) && bigEnough)) return;
  const reply = await sendToShift(url, 'file', { referrer: item.referrer, fileName: item.filename ? item.filename.split(/[\\/]/).pop() : undefined });
  if (reply.ok) {
    chrome.downloads.cancel(item.id, () => chrome.downloads.erase({ id: item.id }));
  }
});

chrome.runtime.onMessage.addListener((msg, _sender, respond) => {
  if (msg && msg.type === 'ping') {
    send({ type: 'ping' }).then(respond);
    return true;
  }
  if (msg && msg.type === 'sendMagnet' && /^magnet:\?/i.test(msg.url || '')) {
    sendToShift(msg.url, 'file', { referrer: msg.referrer }).then(respond);
    return true;
  }
  if (msg && msg.type === 'sendPage' && isWeb(msg.url)) {
    sendToShift(msg.url, 'media').then(respond);
    return true;
  }
  return false;
});
