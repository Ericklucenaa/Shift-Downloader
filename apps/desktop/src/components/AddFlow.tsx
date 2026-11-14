// Fluxo de adicionar: colar → consultar o servidor → confirmar → baixar.
// Também: vários links de uma vez, magnet e arquivo .torrent.

import { open as openDialog } from '@tauri-apps/plugin-dialog';
import clsx from 'clsx';
import { ArrowRight, ChevronRight, CornerDownRight, FileStack, FolderOpen, Link2, ListPlus, Magnet } from 'lucide-react';
import { forwardRef, useImperativeHandle, useMemo, useRef, useState } from 'react';
import { reportError } from '../lib/actions';
import { bytes } from '../lib/format';
import { diagnosticText, useT } from '../lib/i18n';
import { api, asDiagnostic } from '../lib/ipc';
import { useApp } from '../lib/store';
import type { Diagnostic, ProbeView, TorrentInfo } from '../lib/types';
import { Button, Dialog, Kbd, Spinner, Tip } from './ui/primitives';

export interface LinkBarHandle {
  focus: () => void;
  submit: (url: string) => void;
  openTorrentFile: () => void;
  /** .torrent aberto pelo Windows (dois cliques no arquivo). */
  openTorrentPath: (path: string) => void;
}

type Pending =
  | { type: 'probe'; url: string; view: ProbeView }
  | { type: 'magnet'; url: string }
  | { type: 'torrent'; path: string; info: TorrentInfo }
  | { type: 'batch'; initial: string };

/** Barra fixa no topo: o jeito mais rápido de começar um download. */
export const LinkBar = forwardRef<LinkBarHandle>(function LinkBar(_, ref) {
  const { t, lang } = useT();
  const [value, setValue] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<Diagnostic | null>(null);
  const [pending, setPending] = useState<Pending | null>(null);
  const input = useRef<HTMLInputElement>(null);

  const submit = async (raw: string) => {
    const url = raw.trim();
    if (!url || busy) return;
    setError(null);
    // Vários links colados de uma vez viram um lote.
    const lines = url.split(/\s*[\r\n]+\s*/).filter(Boolean);
    if (lines.length > 1) {
      setPending({ type: 'batch', initial: lines.join('\n') });
      return;
    }
    setValue(url);
    if (url.startsWith('magnet:')) {
      setPending({ type: 'magnet', url });
      return;
    }
    setBusy(true);
    try {
      setPending({ type: 'probe', url, view: await api.probe(url) });
    } catch (e) {
      setError(asDiagnostic(e));
    } finally {
      setBusy(false);
    }
  };

  const openTorrentPath = async (path: string) => {
    try {
      setPending({ type: 'torrent', path, info: await api.torrentPreview(path) });
    } catch (e) {
      reportError(e);
    }
  };

  const openTorrentFile = async () => {
    const path = await openDialog({ multiple: false, filters: [{ name: 'Torrent', extensions: ['torrent'] }], title: t('torrent.choose') });
    if (typeof path !== 'string') return;
    try {
      setPending({ type: 'torrent', path, info: await api.torrentPreview(path) });
    } catch (e) {
      reportError(e);
    }
  };

  useImperativeHandle(ref, () => ({ focus: () => input.current?.focus(), submit, openTorrentFile, openTorrentPath }));

  const err = error ? diagnosticText(error, lang) : null;
  const done = (added: boolean) => {
    setPending(null);
    if (added) setValue('');
    requestAnimationFrame(() => input.current?.focus());
  };

  return (
    <div className="relative">
      <div className="flex items-center gap-2">
        <form
          className={clsx(
            'flex h-10 min-w-0 flex-1 items-center gap-2 rounded-md border bg-surface pr-1 pl-3 transition-colors',
            error ? 'border-bad' : 'border-line-strong focus-within:border-ink-2'
          )}
          onSubmit={(e) => {
            e.preventDefault();
            void submit(value);
          }}
        >
          <span className="text-muted">{busy ? <Spinner /> : <Link2 size={15} strokeWidth={1.8} />}</span>
          <input
            ref={input}
            value={value}
            onChange={(e) => {
              setValue(e.target.value);
              if (error) setError(null);
            }}
            placeholder={busy ? t('link.checking') : t('link.placeholder')}
            aria-label={t('link.placeholder')}
            aria-invalid={!!error}
            spellCheck={false}
            autoComplete="off"
            className="num h-full min-w-0 flex-1 bg-transparent text-[13px] text-ink outline-none placeholder:font-sans placeholder:text-faint focus-visible:outline-none"
            disabled={busy}
          />
          {!value && !busy && (
            <span className="hidden items-center gap-1 pr-1 text-[11.5px] text-faint xl:flex">
              <Kbd>Ctrl</Kbd>
              <Kbd>V</Kbd>
            </span>
          )}
          <Button type="submit" variant="primary" disabled={!value.trim() || busy} icon={<ArrowRight size={14} />}>
            {t('link.add')}
          </Button>
        </form>
        <Tip label={t('link.batch')}>
          <Button variant="secondary" className="h-10 px-2.5" aria-label={t('link.batch')} onClick={() => setPending({ type: 'batch', initial: '' })}>
            <ListPlus size={15} />
          </Button>
        </Tip>
        <Tip label={t('link.torrent')}>
          <Button variant="secondary" className="h-10 px-2.5" aria-label={t('link.torrent')} onClick={openTorrentFile}>
            <Magnet size={15} />
          </Button>
        </Tip>
      </div>
      {err && error && (
        <div role="alert" className="mt-2 flex gap-2 rounded-md border border-bad/30 bg-bad-soft px-3 py-2 text-[12.5px]">
          <span className="mt-1.5 size-1.5 shrink-0 rounded-full bg-bad" />
          <div className="min-w-0">
            <div className="font-medium text-ink">{err.title}</div>
            <div className="text-ink-2">{err.action}</div>
            {error.detail && <div className="num mt-0.5 truncate text-[11.5px] text-muted selectable">{error.detail}</div>}
          </div>
        </div>
      )}
      {pending?.type === 'probe' && <AddDialog url={pending.url} probe={pending.view} onClose={done} />}
      {pending?.type === 'magnet' && <TorrentDialog source={pending.url} onClose={done} />}
      {pending?.type === 'torrent' && <TorrentDialog source={pending.path} info={pending.info} onClose={done} />}
      {pending?.type === 'batch' && <BatchDialog initial={pending.initial} onClose={done} />}
    </div>
  );
});

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid grid-cols-[96px_1fr] items-baseline gap-3 py-1.5">
      <dt className="text-[12px] text-muted">{label}</dt>
      <dd className="min-w-0 text-[13px] text-ink">{children}</dd>
    </div>
  );
}

function FolderRow({ dir, setDir }: { dir: string; setDir: (d: string) => void }) {
  const { t } = useT();
  const choose = async () => {
    const picked = await openDialog({ directory: true, defaultPath: dir, title: t('add.chooseFolder') });
    if (typeof picked === 'string') setDir(picked);
  };
  return (
    <Row label={t('add.folder')}>
      <div className="flex items-center gap-2">
        <span className="num min-w-0 flex-1 truncate text-[12.5px] selectable" title={dir}>
          {dir}
        </span>
        <Button size="sm" variant="ghost" icon={<FolderOpen size={14} />} onClick={choose}>
          {t('add.change')}
        </Button>
      </div>
    </Row>
  );
}

function Footer({ busy, onCancel, onPaused, submitLabel }: { busy: boolean; onCancel: () => void; onPaused?: () => void; submitLabel: string }) {
  const { t } = useT();
  return (
    <div className="flex items-center justify-end gap-2 border-t border-line bg-surface-2 px-5 py-3">
      <Button variant="ghost" onClick={onCancel}>
        {t('add.cancel')}
      </Button>
      {onPaused && (
        <Button onClick={onPaused} disabled={busy}>
          {t('add.startPaused')}
        </Button>
      )}
      <Button type="submit" variant="primary" disabled={busy}>
        {submitLabel}
      </Button>
    </div>
  );
}

function ScheduleNote() {
  const { t } = useT();
  const blocked = useApp((s) => s.stats.scheduleBlocked);
  return blocked ? <p className="mt-3 rounded-md bg-warn-soft px-3 py-2 text-[12px] text-warn">{t('add.scheduleNote')}</p> : null;
}

function AddDialog({ url, probe, onClose }: { url: string; probe: ProbeView; onClose: (added: boolean) => void }) {
  const { t, lang } = useT();
  const select = useApp((s) => s.select);
  const navigate = useApp((s) => s.navigate);
  const [name, setName] = useState(probe.fileName);
  const [dir, setDir] = useState(probe.directory);
  const [advanced, setAdvanced] = useState(false);
  const [sha, setSha] = useState('');
  const [limit, setLimit] = useState('');
  const [busy, setBusy] = useState(false);
  const nameRef = useRef<HTMLInputElement>(null);

  // Foco no nome, selecionando sem a extensão (como o Explorador faz ao renomear).
  const focusName = () => {
    const el = nameRef.current;
    if (!el) return;
    const dot = el.value.lastIndexOf('.');
    el.focus();
    el.setSelectionRange(0, dot > 0 ? dot : el.value.length);
  };

  const add = async (startPaused: boolean) => {
    setBusy(true);
    try {
      const task = await api.add({
        url,
        probeId: probe.probeId,
        fileName: name.trim() || undefined,
        directory: dir === probe.directory ? undefined : dir,
        startPaused,
        expectedSha256: sha.trim() || undefined,
        speedLimit: Number(limit) > 0 ? Math.round(Number(limit) * 1024) : undefined
      });
      useApp.getState().upsertTasks([task]);
      navigate('downloads', 'all');
      select(task.id);
      onClose(true);
    } catch (e) {
      reportError(e);
      setBusy(false);
    }
  };

  const resume = probe.resumable === true ? t('add.resumeYes') : probe.resumable === false ? t('add.resumeNo') : t('add.resumeUnknown');
  const tight = probe.totalBytes != null && probe.freeSpace != null && probe.freeSpace < probe.totalBytes * 1.1;

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && !busy && onClose(false)}
      title={t('add.title')}
      description={<span className="num">{probe.displayUrl}</span>}
      width={560}
      onOpenFocus={focusName}
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void add(false);
        }}
      >
        <div className="px-5 pt-3 pb-4">
          <label className="block">
            <span className="text-[12px] text-muted">{t('add.fileName')}</span>
            <input
              ref={nameRef}
              value={name}
              onChange={(e) => setName(e.target.value)}
              spellCheck={false}
              className="mt-1 h-9 w-full rounded-md border border-line-strong bg-surface-2 px-2.5 text-[13.5px] text-ink outline-none focus:border-ink-2"
            />
          </label>
          {probe.renamed && <p className="mt-1.5 text-[12px] text-warn">{t('add.renamed')}</p>}

          <dl className="mt-3 divide-y divide-line border-y border-line">
            <FolderRow dir={dir} setDir={setDir} />
            <Row label={t('add.size')}>
              <span className="num">{probe.totalBytes != null ? bytes(probe.totalBytes, lang) : t('add.unknownSize')}</span>
              {probe.freeSpace != null && (
                <span className={clsx('ml-2 text-[12px]', tight ? 'text-warn' : 'text-muted')}>{t('add.free', { free: bytes(probe.freeSpace, lang) })}</span>
              )}
            </Row>
            <Row label={t('add.resume')}>
              <span className={clsx(probe.resumable === false && 'text-warn')}>{resume}</span>
            </Row>
            {probe.redirects > 0 && (
              <Row label="">
                <span className="flex items-center gap-1 text-[12px] text-muted">
                  <CornerDownRight size={12} />
                  {t('add.redirects', { n: probe.redirects, host: probe.host })}
                </span>
              </Row>
            )}
          </dl>

          <button
            type="button"
            onClick={() => setAdvanced((a) => !a)}
            aria-expanded={advanced}
            className="mt-3 flex items-center gap-1 text-[12.5px] text-ink-2 hover:text-ink"
          >
            <ChevronRight size={14} className={clsx('transition-transform', advanced && 'rotate-90')} />
            {t('add.advanced')}
          </button>
          {advanced && (
            <div className="mt-2 grid gap-3">
              <label className="block">
                <span className="text-[12px] text-muted">{t('add.sha256')}</span>
                <input
                  value={sha}
                  onChange={(e) => setSha(e.target.value)}
                  spellCheck={false}
                  placeholder="e3b0c442…"
                  className="num mt-1 h-8 w-full rounded-md border border-line-strong bg-surface-2 px-2.5 text-[12px] outline-none focus:border-ink-2"
                />
                <span className="mt-1 block text-[11.5px] text-faint">{t('add.sha256Help')}</span>
              </label>
              <label className="block">
                <span className="text-[12px] text-muted">{t('add.limit')}</span>
                <input
                  value={limit}
                  onChange={(e) => setLimit(e.target.value.replace(/[^\d]/g, ''))}
                  inputMode="numeric"
                  placeholder="0"
                  className="num mt-1 block h-8 w-32 rounded-md border border-line-strong bg-surface-2 px-2.5 text-[12.5px] outline-none focus:border-ink-2"
                />
              </label>
            </div>
          )}
          <ScheduleNote />
        </div>
        <Footer busy={busy} onCancel={() => onClose(false)} onPaused={() => add(true)} submitLabel={busy ? t('add.adding') : t('add.start')} />
      </form>
    </Dialog>
  );
}

function TorrentDialog({ source, info, onClose }: { source: string; info?: TorrentInfo; onClose: (added: boolean) => void }) {
  const { t, lang } = useT();
  const settings = useApp((s) => s.settings);
  const navigate = useApp((s) => s.navigate);
  const select = useApp((s) => s.select);
  const [dir, setDir] = useState(settings?.downloadDir ?? '');
  const [selected, setSelected] = useState<Set<number>>(() => new Set(info?.files.map((f) => f.index) ?? []));
  const [busy, setBusy] = useState(false);
  const size = useMemo(() => info?.files.filter((f) => selected.has(f.index)).reduce((a, f) => a + f.length, 0) ?? 0, [info, selected]);
  const magnetName = useMemo(() => {
    try {
      return new URLSearchParams(source.replace(/^magnet:\?/, '')).get('dn') ?? '';
    } catch {
      return '';
    }
  }, [source]);

  const add = async (startPaused: boolean) => {
    setBusy(true);
    try {
      const task = await api.addTorrent({
        source,
        selected: info ? [...selected] : undefined,
        directory: dir === settings?.downloadDir ? undefined : dir,
        startPaused
      });
      useApp.getState().upsertTasks([task]);
      navigate('downloads', 'torrents');
      select(task.id);
      onClose(true);
    } catch (e) {
      reportError(e);
      setBusy(false);
    }
  };

  const toggle = (i: number) =>
    setSelected((s) => {
      const n = new Set(s);
      if (n.has(i)) n.delete(i);
      else n.add(i);
      return n;
    });

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && !busy && onClose(false)}
      title={info ? t('torrent.title') : t('torrent.magnetTitle')}
      description={<span className="num">{info?.name ?? magnetName}</span>}
      width={600}
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (!info || selected.size > 0) void add(false);
        }}
      >
        <div className="px-5 pt-3 pb-4">
          <dl className="divide-y divide-line border-y border-line">
            <FolderRow dir={dir} setDir={setDir} />
          </dl>
          {info ? (
            <div className="mt-3">
              <div className="flex items-center gap-2">
                <span className="text-[12px] text-muted">{t('torrent.files')}</span>
                <span className="num text-[12px] text-ink-2">
                  {t('torrent.selected', { n: selected.size, total: info.files.length, size: bytes(size, lang) })}
                </span>
                <span className="ml-auto flex gap-1">
                  <Button size="sm" variant="ghost" onClick={() => setSelected(new Set(info.files.map((f) => f.index)))}>
                    {t('torrent.selectAll')}
                  </Button>
                  <Button size="sm" variant="ghost" onClick={() => setSelected(new Set())}>
                    {t('torrent.selectNone')}
                  </Button>
                </span>
              </div>
              <ul className="mt-1.5 max-h-60 overflow-y-auto rounded-md border border-line">
                {info.files.map((f) => (
                  <li key={f.index} className="border-b border-line last:border-b-0">
                    <label className="flex cursor-default items-center gap-2.5 px-3 py-1.5 text-[12.5px] hover:bg-surface-2">
                      <input type="checkbox" checked={selected.has(f.index)} onChange={() => toggle(f.index)} className="accent-[var(--accent)]" />
                      <span className="min-w-0 flex-1 truncate selectable" title={f.path}>
                        {f.path}
                      </span>
                      <span className="num shrink-0 text-muted">{bytes(f.length, lang)}</span>
                    </label>
                  </li>
                ))}
              </ul>
            </div>
          ) : (
            <p className="mt-3 text-[12.5px] text-ink-2">{t('torrent.magnetNote')}</p>
          )}
          <p className="mt-3 text-[11.5px] text-faint">{t('torrent.firewall')}</p>
          <ScheduleNote />
        </div>
        <Footer
          busy={busy || (!!info && selected.size === 0)}
          onCancel={() => onClose(false)}
          onPaused={() => add(true)}
          submitLabel={busy ? t('add.adding') : t('add.start')}
        />
      </form>
    </Dialog>
  );
}

function BatchDialog({ initial, onClose }: { initial: string; onClose: (added: boolean) => void }) {
  const { t, lang } = useT();
  const settings = useApp((s) => s.settings);
  const navigate = useApp((s) => s.navigate);
  const [text, setText] = useState(initial);
  const [dir, setDir] = useState(settings?.downloadDir ?? '');
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{ ok: number; failed: { url: string; error: Diagnostic | null }[] } | null>(null);
  const urls = text
    .split(/\s*[\r\n]+\s*/)
    .map((u) => u.trim())
    .filter(Boolean);

  const add = async (startPaused: boolean) => {
    setBusy(true);
    try {
      const res = await api.addBatch(urls, dir === settings?.downloadDir ? undefined : dir, startPaused);
      useApp.getState().upsertTasks(res.flatMap((r) => (r.task ? [r.task] : [])));
      const failed = res.filter((r) => !r.task).map((r) => ({ url: r.url, error: r.error }));
      if (failed.length === 0) {
        navigate('downloads', 'all');
        onClose(true);
        return;
      }
      setResult({ ok: res.length - failed.length, failed });
      setText(failed.map((f) => f.url).join('\n'));
    } catch (e) {
      reportError(e);
    } finally {
      setBusy(false);
    }
  };

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && !busy && onClose(!!result)}
      title={t('batch.title')}
      description={t('batch.help')}
      width={620}
      onOpenFocus={() => document.getElementById('batch-text')?.focus()}
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void add(false);
        }}
      >
        <div className="px-5 pt-3 pb-4">
          <textarea
            id="batch-text"
            value={text}
            onChange={(e) => setText(e.target.value)}
            spellCheck={false}
            rows={9}
            aria-label={t('batch.title')}
            placeholder={'https://…\nhttps://…\nmagnet:?xt=…'}
            className="num w-full resize-y rounded-md border border-line-strong bg-surface-2 px-2.5 py-2 text-[12px] leading-relaxed outline-none focus:border-ink-2"
          />
          <div className="mt-1 flex items-center gap-2 text-[12px] text-muted">
            <FileStack size={13} />
            {t('batch.count', { n: urls.length })}
            {busy && (
              <span className="ml-auto flex items-center gap-1.5">
                <Spinner size={12} /> {t('batch.adding', { n: urls.length })}
              </span>
            )}
          </div>
          {result && (
            <div role="status" className="mt-3 rounded-md border border-line bg-surface-2 px-3 py-2 text-[12.5px]">
              <div className="font-medium">{t('batch.result', { ok: result.ok, fail: result.failed.length })}</div>
              <ul className="mt-1 max-h-28 overflow-y-auto">
                {result.failed.map((f) => (
                  <li key={f.url} className="truncate text-muted">
                    <span className="num">{f.url}</span> — {f.error ? diagnosticText(f.error, lang).title : ''}
                  </li>
                ))}
              </ul>
            </div>
          )}
          <dl className="mt-3 divide-y divide-line border-y border-line">
            <FolderRow dir={dir} setDir={setDir} />
          </dl>
          <ScheduleNote />
        </div>
        <Footer busy={busy || urls.length === 0} onCancel={() => onClose(!!result)} onPaused={() => add(true)} submitLabel={t('batch.add')} />
      </form>
    </Dialog>
  );
}
