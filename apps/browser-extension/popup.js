const $ = (id) => document.getElementById(id);
const DEFAULTS = { openMagnets: true, intercept: false, extensions: 'zip,rar,7z,tar,gz,iso,img,exe,msi,dmg,apk,mp4,mkv,avi,mov,mp3,flac,pdf,epub', minSizeMB: 2 };

async function load() {
  const s = { ...DEFAULTS, ...(await chrome.storage.local.get(DEFAULTS)) };
  $('intercept').checked = s.intercept;
  $('openMagnets').checked = s.openMagnets;
  $('extensions').value = s.extensions;
  $('minSizeMB').value = s.minSizeMB;
  $('opts').setAttribute('aria-disabled', String(!s.intercept));
}

function save() {
  const intercept = $('intercept').checked;
  $('opts').setAttribute('aria-disabled', String(!intercept));
  chrome.storage.local.set({
    openMagnets: $('openMagnets').checked,
    intercept,
    extensions: $('extensions').value,
    minSizeMB: Math.max(0, Number($('minSizeMB').value) || 0)
  });
}

async function status() {
  const r = await chrome.runtime.sendMessage({ type: 'ping' });
  const el = $('status');
  const help = $('help');
  if (r && r.ok && r.running) {
    el.textContent = 'Conectado';
    el.className = 'status ok';
    help.hidden = true;
  } else if (r && r.ok) {
    el.textContent = 'SHIFT fechado';
    el.className = 'status';
    help.textContent = 'Ao enviar um link, o SHIFT abre sozinho.';
    help.hidden = false;
  } else {
    el.textContent = 'Não instalado';
    el.className = 'status bad';
    help.textContent = r && r.hostMissing
      ? 'Abra o SHIFT uma vez no computador para ativar a integração e tente de novo.'
      : 'Não foi possível falar com o SHIFT: ' + ((r && r.error) || 'erro desconhecido');
    help.hidden = false;
  }
}

$('intercept').addEventListener('change', save);
$('openMagnets').addEventListener('change', save);
$('extensions').addEventListener('change', save);
$('minSizeMB').addEventListener('change', save);
$('sendPage').addEventListener('click', async () => {
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  if (!tab || !/^https?:/i.test(tab.url || '')) return;
  $('sendPage').disabled = true;
  const r = await chrome.runtime.sendMessage({ type: 'sendPage', url: tab.url });
  $('sendPage').textContent = r && r.ok ? 'Enviado ao SHIFT' : 'Não deu certo';
  setTimeout(() => window.close(), 900);
});

load();
status();
