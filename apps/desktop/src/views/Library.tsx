import clsx from 'clsx';
import { FolderSearch, Info, Search, SquareArrowOutUpRight, Wand2 } from 'lucide-react';
import { useMemo, useState } from 'react';
import { actions, mediaActions } from '../lib/actions';
import { bytes, dateTime, fileKind } from '../lib/format';
import { useT } from '../lib/i18n';
import { libraryItems, useApp, type LibraryItem } from '../lib/store';
import type { Category } from '../lib/types';
import { FileGlyph } from '../components/brand';
import { IconButton, Tip } from '../components/ui/primitives';

const CATS: Category[] = ['video', 'audio', 'image', 'document', 'archive', 'disk', 'program', 'other'];

export function Library() {
  const { t, lang } = useT();
  const tasks = useApp((s) => s.tasks);
  const media = useApp((s) => s.media);
  const navigate = useApp((s) => s.navigate);
  const select = useApp((s) => s.select);
  const [cat, setCat] = useState<Category | 'all' | 'dup'>('all');
  const [tag, setTag] = useState<string | null>(null);
  const [q, setQ] = useState('');
  const items = useMemo(() => libraryItems(tasks, media), [tasks, media]);
  const tags = useMemo(() => [...new Set(items.flatMap((i) => i.tags))].sort(), [items]);
  const query = q.trim().toLocaleLowerCase();
  const visible = items.filter(
    (i) =>
      (cat === 'all' || (cat === 'dup' ? i.duplicate : i.category === cat)) &&
      (!tag || i.tags.includes(tag)) &&
      (!query || i.name.toLocaleLowerCase().includes(query) || i.origin.toLocaleLowerCase().includes(query) || i.tags.some((g) => g.toLocaleLowerCase().includes(query)))
  );
  const total = visible.reduce((a, i) => a + (i.size ?? 0), 0);
  const countOf = (c: Category) => items.filter((i) => i.category === c).length;
  const dups = items.filter((i) => i.duplicate).length;

  const open = (i: LibraryItem) => (i.source === 'download' ? actions.open(tasks[i.id]) : mediaActions.open(media[i.id]));
  const reveal = (i: LibraryItem) => (i.source === 'download' ? actions.reveal(tasks[i.id]) : mediaActions.reveal(media[i.id]));
  const convert = (i: LibraryItem) => {
    useApp.getState().setPendingConvert(i.path);
    navigate('media');
  };

  const Chip = ({ id, label, n }: { id: Category | 'all' | 'dup'; label: string; n: number }) => (
    <button
      type="button"
      aria-pressed={cat === id}
      onClick={() => setCat(id)}
      className={clsx('flex h-7 items-center gap-1.5 rounded-md border px-2.5 text-[12.5px] transition-colors', cat === id ? 'border-ink-2 bg-surface text-ink' : 'border-line text-ink-2 hover:bg-surface')}
    >
      {label}
      <span className="num text-[11px] text-muted">{n}</span>
    </button>
  );

  return (
    <section className="flex min-h-0 min-w-0 flex-1 flex-col">
      <div className="flex flex-wrap items-center gap-3 px-4 pt-4 pb-3">
        <h1 className="text-[15px] font-semibold tracking-[-0.01em]">{t('nav.library')}</h1>
        <span className="num text-[12px] text-muted">{t('library.count', { n: visible.length, size: bytes(total, lang) })}</span>
        <label className="ml-auto flex h-7 w-64 items-center gap-1.5 rounded-md border border-line bg-surface px-2 focus-within:border-line-strong">
          <Search size={13} className="text-faint" />
          <input
            value={q}
            onChange={(e) => setQ(e.target.value)}
            placeholder={t('library.search')}
            aria-label={t('library.search')}
            className="h-full min-w-0 flex-1 bg-transparent text-[12.5px] outline-none placeholder:text-faint"
          />
        </label>
      </div>
      <div className="flex flex-wrap gap-1.5 px-4 pb-3">
        <Chip id="all" label={t('library.all')} n={items.length} />
        {CATS.filter((c) => countOf(c) > 0).map((c) => (
          <Chip key={c} id={c} label={t(`library.cat.${c}` as 'library.cat.video')} n={countOf(c)} />
        ))}
        {dups > 0 && <Chip id="dup" label={t('library.duplicates')} n={dups} />}
        {tags.length > 0 && <span className="mx-1 w-px self-stretch bg-line" aria-hidden="true" />}
        {tags.map((g) => (
          <button
            key={g}
            type="button"
            aria-pressed={tag === g}
            onClick={() => setTag(tag === g ? null : g)}
            className={clsx('h-7 rounded-md px-2 text-[12px]', tag === g ? 'bg-ink text-bg' : 'bg-sunken text-ink-2 hover:text-ink')}
          >
            #{g}
          </button>
        ))}
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto border-t border-line">
        {visible.length === 0 ? (
          <p className="px-6 py-16 text-center text-[13px] text-muted">{t('library.empty')}</p>
        ) : (
          <ul>
            {visible.map((i) => (
              <li key={i.key} className="group flex items-center gap-3 border-b border-line px-4 py-2.5 hover:bg-surface/60" onDoubleClick={() => open(i)}>
                <FileGlyph kind={fileKind(i.name)} />
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="truncate text-[13px] font-medium" title={i.path}>
                      {i.name}
                    </span>
                    {i.duplicate && (
                      <Tip label={t('detail.duplicate')}>
                        <span className="shrink-0 rounded-sm border border-line px-1 text-[10.5px] text-warn">dup</span>
                      </Tip>
                    )}
                  </div>
                  <div className="mt-0.5 flex items-center gap-1.5 text-[12px] text-muted">
                    <span className="truncate">{i.origin}</span>
                    {i.tags.map((g) => (
                      <span key={g} className="shrink-0 rounded-sm bg-sunken px-1.5 text-[11px] text-ink-2">
                        {g}
                      </span>
                    ))}
                  </div>
                </div>
                <span className="num w-24 shrink-0 text-right text-[12px] text-ink-2">{bytes(i.size, lang)}</span>
                <span className="w-32 shrink-0 text-right text-[12px] text-muted">{dateTime(i.finishedAt, lang)}</span>
                <div className="flex shrink-0 gap-0.5 opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100">
                  <IconButton label={t('action.open')} onClick={() => open(i)}>
                    <SquareArrowOutUpRight size={15} />
                  </IconButton>
                  <IconButton label={t('action.reveal')} onClick={() => reveal(i)}>
                    <FolderSearch size={15} />
                  </IconButton>
                  {(i.category === 'video' || i.category === 'audio') && (
                    <IconButton label={t('action.convert')} onClick={() => convert(i)}>
                      <Wand2 size={15} />
                    </IconButton>
                  )}
                  {i.source === 'download' && (
                    <IconButton
                      label={t('detail.timeline')}
                      onClick={() => {
                        navigate('downloads', 'completed');
                        select(i.id);
                      }}
                    >
                      <Info size={15} />
                    </IconButton>
                  )}
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
