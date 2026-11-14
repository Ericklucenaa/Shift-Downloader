// "Escolher arquivos…" de um torrent já adicionado.
import { useEffect, useState } from 'react';
import { reportError, useTorrentFiles } from '../lib/actions';
import { bytes } from '../lib/format';
import { useT } from '../lib/i18n';
import { api } from '../lib/ipc';
import { useApp } from '../lib/store';
import type { TorrentMeta } from '../lib/types';
import { Button, Dialog, Spinner } from './ui/primitives';

export function TorrentFilesHost() {
  const { t, lang } = useT();
  const id = useTorrentFiles((s) => s.id);
  const close = () => useTorrentFiles.getState().set(null);
  const task = useApp((s) => (id ? s.tasks[id] : undefined));
  const [meta, setMeta] = useState<TorrentMeta | null>(null);
  const [sel, setSel] = useState<Set<number>>(new Set());
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    setMeta(null);
    if (!id) return;
    api
      .torrentFiles(id)
      .then((m) => {
        setMeta(m);
        setSel(new Set(m.selected));
      })
      .catch((e) => {
        reportError(e);
        close();
      });
  }, [id]);

  if (!id || !task) return null;
  const size = meta?.files.filter((f) => sel.has(f.index)).reduce((a, f) => a + f.length, 0) ?? 0;
  const apply = async () => {
    setBusy(true);
    try {
      useApp.getState().upsertTasks([await api.setTorrentFiles(id, [...sel])]);
      close();
    } catch (e) {
      reportError(e);
    } finally {
      setBusy(false);
    }
  };
  const toggle = (i: number) =>
    setSel((s) => {
      const n = new Set(s);
      if (n.has(i)) n.delete(i);
      else n.add(i);
      return n;
    });

  return (
    <Dialog open onOpenChange={(o) => !o && close()} title={t('torrentFiles.title')} description={task.fileName} width={600}>
      <div className="px-5 pt-3 pb-4">
        <p className="text-[12.5px] text-muted">{t('torrentFiles.help')}</p>
        {!meta ? (
          <div className="flex justify-center py-8">
            <Spinner />
          </div>
        ) : (
          <>
            <div className="mt-3 flex items-center gap-2">
              <span className="num text-[12px] text-ink-2">{t('torrent.selected', { n: sel.size, total: meta.files.length, size: bytes(size, lang) })}</span>
              <span className="ml-auto flex gap-1">
                <Button size="sm" variant="ghost" onClick={() => setSel(new Set(meta.files.map((f) => f.index)))}>
                  {t('torrent.selectAll')}
                </Button>
                <Button size="sm" variant="ghost" onClick={() => setSel(new Set())}>
                  {t('torrent.selectNone')}
                </Button>
              </span>
            </div>
            <ul className="mt-1.5 max-h-72 overflow-y-auto rounded-md border border-line">
              {meta.files.map((f) => (
                <li key={f.index} className="border-b border-line last:border-b-0">
                  <label className="flex cursor-default items-center gap-2.5 px-3 py-1.5 text-[12.5px] hover:bg-surface-2">
                    <input type="checkbox" checked={sel.has(f.index)} onChange={() => toggle(f.index)} className="accent-[var(--accent)]" />
                    <span className="min-w-0 flex-1 truncate selectable" title={f.path}>
                      {f.path}
                    </span>
                    <span className="num shrink-0 text-muted">{bytes(f.length, lang)}</span>
                  </label>
                </li>
              ))}
            </ul>
          </>
        )}
      </div>
      <div className="flex justify-end gap-2 border-t border-line bg-surface-2 px-5 py-3">
        <Button variant="ghost" onClick={close}>
          {t('add.cancel')}
        </Button>
        <Button variant="primary" disabled={busy || !meta || sel.size === 0} onClick={apply}>
          {t('detail.apply')}
        </Button>
      </div>
    </Dialog>
  );
}
