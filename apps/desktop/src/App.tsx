import { Pause, Play } from 'lucide-react';
import { useEffect, useRef } from 'react';
import { DetailPanel } from './components/DetailPanel';
import { DownloadList } from './components/DownloadList';
import { LinkBar, type LinkBarHandle } from './components/AddFlow';
import { AfterAllHost, ConfirmHost, Toasts } from './components/Overlays';
import { Sidebar } from './components/Sidebar';
import { StatusBar } from './components/StatusBar';
import { Wordmark } from './components/brand';
import { Button, Spinner, TipProvider } from './components/ui/primitives';
import { actions, reportError } from './lib/actions';
import { translate, useT } from './lib/i18n';
import { api, subscribe } from './lib/ipc';
import { useApp } from './lib/store';
import { applyButtons, applyTheme } from './lib/theme';
import { TorrentFilesHost } from './components/TorrentFiles';
import { WelcomeHost } from './components/Welcome';
import type { ExternalAdd } from './lib/types';
import { Library } from './views/Library';
import { Media } from './views/Media';
import { Overview } from './views/Overview';
import { Schedule } from './views/Schedule';
import { Settings } from './views/Settings';

/** Link vindo da extensão do navegador. */
async function handleExternal(req: ExternalAdd, bar: React.RefObject<LinkBarHandle | null>) {
  const st = useApp.getState();
  const lang = st.settings?.language ?? 'pt-BR';
  st.toast({ kind: 'info', title: translate(lang, 'external.received'), body: req.url.slice(0, 120) });
  if (req.kind === 'torrent-file') {
    st.navigate('downloads', 'torrents');
    requestAnimationFrame(() => bar.current?.openTorrentPath(req.url));
    return;
  }
  if (req.kind === 'media') {
    st.setPendingExternal(req);
    st.navigate('media');
    return;
  }
  if (st.settings?.browserConfirm ?? true) {
    st.navigate('downloads', 'all');
    // A barra só existe na tela de downloads: espera ela montar.
    requestAnimationFrame(() => bar.current?.submit(req.url));
    return;
  }
  try {
    const task = req.url.startsWith('magnet:')
      ? await api.addTorrent({ source: req.url })
      : await api.add({ url: req.url, fileName: req.fileName ?? undefined });
    st.upsertTasks([task]);
    st.navigate('downloads', 'all');
    st.select(task.id);
  } catch (e) {
    reportError(e);
  }
}

function useCoreConnection(bar: React.RefObject<LinkBarHandle | null>) {
  useEffect(() => {
    let off: (() => void) | undefined;
    let alive = true;
    (async () => {
      // Assina antes do snapshot: nada que aconteça no meio se perde.
      off = await subscribe({
        onTasks: (t) => useApp.getState().upsertTasks(t),
        onRemoved: (ids) => useApp.getState().removeTasks(ids),
        onEngine: (e) => useApp.getState().setEngine(e),
        onStats: (s) => useApp.getState().setStats(s),
        onMedia: (m) => useApp.getState().upsertMedia(m),
        onMediaRemoved: (ids) => useApp.getState().removeMedia(ids),
        onAfterAll: (a) => useApp.getState().setAfterAll(a),
        onExternalAdd: (r) => void handleExternal(r, bar)
      });
      try {
        const [snap, media] = await Promise.all([api.snapshot(), api.mediaList()]);
        if (!alive) return;
        useApp.getState().loadSnapshot(snap, media);
        applyTheme(snap.settings.theme);
        applyButtons(snap.settings.buttonStyle);
        // 0.5.0: o visual passou a ser o da família Shift (botões planos). Migra uma vez quem estava no
        // "liquid", padrão da 0.4.x; depois disso, a escolha em Configurações é respeitada.
        let migrated: string | null = null;
        try { migrated = localStorage.getItem('shift.design'); } catch { /* sem armazenamento */ }
        if (migrated !== 'family-1') {
          try { localStorage.setItem('shift.design', 'family-1'); } catch { /* idem */ }
          if (snap.settings.buttonStyle === 'liquid') {
            const s = await api.updateSettings({ ...snap.settings, buttonStyle: 'classic' }).catch(() => null);
            if (s && alive) {
              useApp.getState().setSettings?.(s);
              applyButtons(s.buttonStyle);
            }
          }
        }
        document.documentElement.lang = snap.settings.language;
        // O Windows abriu o Shift Downloader com um magnet ou um .torrent.
        const launch = await api.takeLaunchTarget().catch(() => null);
        if (launch) setTimeout(() => void handleExternal(launch, bar), 300);
      } catch (e) {
        useApp.getState().setLoadError(String((e as { detail?: string })?.detail ?? e));
      }
    })();
    return () => {
      alive = false;
      off?.();
    };
  }, [bar]);
}

/** Ctrl+V fora de campos de texto: cola o link direto na barra e consulta. */
function useGlobalPaste(bar: React.RefObject<LinkBarHandle | null>) {
  useEffect(() => {
    const onPaste = (e: ClipboardEvent) => {
      const el = document.activeElement;
      if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) return;
      if (document.querySelector('[role="dialog"]')) return;
      const text = e.clipboardData?.getData('text')?.trim();
      if (!text) return;
      e.preventDefault();
      const st = useApp.getState();
      if (st.view !== 'downloads' && st.view !== 'overview') st.navigate('downloads');
      requestAnimationFrame(() => bar.current?.submit(text));
    };
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'l') {
        e.preventDefault();
        bar.current?.focus();
      }
    };
    window.addEventListener('paste', onPaste);
    window.addEventListener('keydown', onKey);
    return () => {
      window.removeEventListener('paste', onPaste);
      window.removeEventListener('keydown', onKey);
    };
  }, [bar]);
}

function Topbar({ bar }: { bar: React.RefObject<LinkBarHandle | null> }) {
  const { t } = useT();
  const stats = useApp((s) => s.stats);
  return (
    <header className="flex items-start gap-3 border-b border-line bg-bg px-4 py-3">
      <div className="min-w-0 flex-1">
        <LinkBar ref={bar} />
      </div>
      <div className="flex h-10 items-center gap-1">
        <Button variant="ghost" icon={<Pause size={14} />} disabled={stats.active + stats.queued === 0} onClick={() => actions.pauseAll()}>
          {t('action.pauseAll')}
        </Button>
        <Button variant="ghost" icon={<Play size={14} />} disabled={stats.paused === 0} onClick={() => actions.resumeAll()}>
          {t('action.resumeAll')}
        </Button>
      </div>
    </header>
  );
}

function Splash({ error }: { error: string | null }) {
  return (
    <div className="flex h-full flex-col items-center justify-center gap-4 bg-bg">
      <Wordmark />
      {error ? <p className="num max-w-md px-6 text-center text-[12px] text-bad selectable">{error}</p> : <Spinner size={16} />}
    </div>
  );
}

export function App() {
  const bar = useRef<LinkBarHandle>(null);
  useCoreConnection(bar);
  useGlobalPaste(bar);
  const ready = useApp((s) => s.ready);
  const loadError = useApp((s) => s.loadError);
  const view = useApp((s) => s.view);
  const selected = useApp((s) => (s.selectedId ? s.tasks[s.selectedId] : undefined));

  if (!ready) return <Splash error={loadError} />;

  return (
    <TipProvider>
      <div className="flex h-full flex-col">
        <div className="flex min-h-0 flex-1">
          <Sidebar />
          <main className="flex min-w-0 flex-1 flex-col">
            {(view === 'downloads' || view === 'overview') && <Topbar bar={bar} />}
            <div className="flex min-h-0 flex-1">
              {view === 'overview' && <Overview />}
              {view === 'downloads' && (
                <>
                  <DownloadList />
                  {selected && <DetailPanel task={selected} />}
                </>
              )}
              {view === 'media' && <Media />}
              {view === 'library' && <Library />}
              {view === 'schedule' && <Schedule />}
              {view === 'settings' && <Settings />}
            </div>
          </main>
        </div>
        <StatusBar />
      </div>
      <ConfirmHost />
      <AfterAllHost />
      <TorrentFilesHost />
      <WelcomeHost />
      <Toasts />
    </TipProvider>
  );
}
