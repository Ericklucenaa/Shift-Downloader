import clsx from 'clsx';
import { Search } from 'lucide-react';
import { memo, useEffect, useRef } from 'react';
import { actions } from '../lib/actions';
import { bytes, duration, fileKind, percent, percentLabel, speed } from '../lib/format';
import { diagnosticText, useT } from '../lib/i18n';
import { useApp, visibleTasks } from '../lib/store';
import type { TaskView } from '../lib/types';
import { FileGlyph } from './brand';
import { StatusLabel } from './StatusLabel';
import { PrimaryAction, TaskContextMenu, TaskMenu } from './TaskMenu';
import { ProgressBar } from './ui/progress';

// Três larguras: estreita (nome + tamanho), média (+ situação) e larga (+ velocidade e tempo).
const GRID =
  'grid grid-cols-[minmax(0,1fr)_104px_60px] @2xl:grid-cols-[minmax(0,1fr)_132px_112px_60px] @4xl:grid-cols-[minmax(0,1fr)_132px_92px_84px_118px_60px]';

const Row = memo(function Row({ task, selected, onSelect }: { task: TaskView; selected: boolean; onSelect: (id: string) => void }) {
  const { lang } = useT();
  const p = percent(task.completedBytes, task.totalBytes);
  const showBar = task.status !== 'completed' && task.status !== 'cancelled';
  const err = task.error && task.status === 'failed' ? diagnosticText(task.error, lang) : null;
  const sizeText =
    task.status === 'completed'
      ? bytes(task.totalBytes ?? task.completedBytes, lang)
      : task.totalBytes != null
        ? `${bytes(task.completedBytes, lang)} / ${bytes(task.totalBytes, lang)}`
        : bytes(task.completedBytes, lang);

  return (
    <TaskContextMenu task={task} onOpen={() => onSelect(task.id)}>
    <li
      aria-current={selected ? 'true' : undefined}
      id={`task-${task.id}`}
      onClick={() => onSelect(task.id)}
      onDoubleClick={() => actions.primary(task)}
      className={clsx(
        GRID,
        'group relative cursor-default items-center gap-x-4 border-b border-line px-4 py-2.5',
        selected ? 'bg-surface' : 'hover:bg-surface/60'
      )}
    >
      {selected && <span className="absolute inset-y-0 left-0 w-[2px] bg-accent" aria-hidden="true" />}
      <div className="flex min-w-0 items-center gap-3">
        <FileGlyph kind={fileKind(task.fileName)} />
        <div className="min-w-0 flex-1">
          <div className={clsx('truncate text-[13px] font-medium', task.status === 'cancelled' ? 'text-muted line-through decoration-faint' : 'text-ink')} title={task.fileName}>
            {task.fileName}
          </div>
          <div className="mt-0.5 flex min-w-0 items-center gap-1.5 text-[12px] text-muted">
            {err ? (
              <span className="truncate text-bad">{err.title}</span>
            ) : (
              <>
                <span className="truncate">{task.host}</span>
                {p != null && showBar && <span className="num shrink-0 text-ink-2">{percentLabel(p, lang)}</span>}
                {task.status === 'downloading' && task.speed > 0 && (
                  <span className="num shrink-0 text-ink-2 @4xl:hidden">· {speed(task.speed, lang)}</span>
                )}
                {task.kind !== 'http' && task.status === 'downloading' && (
                  <span className="num shrink-0 text-muted">
                    · {task.seeders}↓ {task.uploadSpeed > 0 ? `· ↑ ${speed(task.uploadSpeed, lang)}` : ''}
                  </span>
                )}
                {task.duplicate && <span className="shrink-0 rounded-sm border border-line px-1 text-[10.5px] text-warn">dup</span>}
                {task.tags.slice(0, 2).map((g) => (
                  <span key={g} className="hidden shrink-0 rounded-sm bg-sunken px-1.5 text-[11px] text-ink-2 @2xl:inline">
                    {g}
                  </span>
                ))}
              </>
            )}
            {!err && <StatusLabel task={task} className="ml-auto @2xl:hidden" />}
          </div>
          {showBar && <ProgressBar value={p} status={task.status} className="mt-1.5" />}
        </div>
      </div>
      <div className="num truncate text-right text-[12px] text-ink-2">{sizeText}</div>
      <div className="num hidden text-right text-[12px] text-ink-2 @4xl:block">{task.status === 'downloading' ? speed(task.speed, lang) : ''}</div>
      <div className="num hidden text-right text-[12px] text-ink-2 @4xl:block">{task.status === 'downloading' ? duration(task.etaSeconds) : ''}</div>
      <div className="hidden @2xl:block">
        <StatusLabel task={task} />
      </div>
      <div className="flex justify-end gap-0.5 opacity-100 transition-opacity @2xl:opacity-0 @2xl:group-hover:opacity-100 @2xl:group-aria-[current=true]:opacity-100 @2xl:focus-within:opacity-100">
        <PrimaryAction task={task} />
        <TaskMenu task={task} />
      </div>
    </li>
    </TaskContextMenu>
  );
});

function Empty() {
  const { t } = useT();
  const filter = useApp((s) => s.filter);
  const query = useApp((s) => s.query);
  if (query.trim())
    return <div className="px-6 py-16 text-center text-[13px] text-muted">{t('empty.search', { q: query.trim() })}</div>;
  const key = filter;
  return (
    <div className="mx-auto max-w-sm px-6 py-20 text-center">
      <div className="mx-auto mb-4 flex h-10 w-14 items-end justify-center gap-1" aria-hidden="true">
        {[0.35, 0.6, 1, 0.6, 0.35].map((h, i) => (
          <span key={i} className="w-1.5 rounded-full bg-line-strong" style={{ height: `${h * 100}%` }} />
        ))}
      </div>
      <h2 className="text-[14px] font-medium text-ink">{t(`empty.${key}Title`)}</h2>
      <p className="mt-1 text-[12.5px] text-muted">{t(`empty.${key}Body`)}</p>
    </div>
  );
}

export function DownloadList() {
  const { t } = useT();
  const tasks = useApp((s) => s.tasks);
  const filter = useApp((s) => s.filter);
  const query = useApp((s) => s.query);
  const setQuery = useApp((s) => s.setQuery);
  const selectedId = useApp((s) => s.selectedId);
  const select = useApp((s) => s.select);
  const list = visibleTasks(tasks, filter, query);
  const listRef = useRef<HTMLUListElement>(null);

  useEffect(() => {
    if (selectedId) document.getElementById(`task-${selectedId}`)?.scrollIntoView({ block: 'nearest' });
  }, [selectedId]);

  const onKey = (e: React.KeyboardEvent) => {
    const i = list.findIndex((x) => x.id === selectedId);
    const cur = list[i];
    if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
      e.preventDefault();
      const next = list[Math.max(0, Math.min(list.length - 1, i + (e.key === 'ArrowDown' ? 1 : -1)))];
      if (next) select(next.id);
    } else if (e.key === ' ' && cur) {
      e.preventDefault();
      void actions.primary(cur);
    } else if (e.key === 'Enter' && cur?.status === 'completed') {
      void actions.open(cur);
    } else if (e.key === 'Delete' && cur) {
      if (['downloading', 'queued', 'paused'].includes(cur.status)) void actions.cancel(cur);
      else void actions.remove(cur);
    } else if (e.key === 'Escape') {
      select(null);
    }
  };

  const titleKey = { all: 'nav.all', active: 'nav.active', completed: 'nav.completed', failed: 'nav.failed', torrents: 'nav.torrents' } as const;

  return (
    <section className="@container flex min-h-0 min-w-0 flex-1 flex-col">
      <div className="flex items-center gap-3 px-4 pt-4 pb-3">
        <h1 className="text-[15px] font-semibold tracking-[-0.01em]">{t(titleKey[filter])}</h1>
        <span className="num text-[12px] text-muted">{list.length}</span>
        <label className="ml-auto flex h-7 w-56 items-center gap-1.5 rounded-md border border-line bg-surface px-2 focus-within:border-line-strong">
          <Search size={13} className="text-faint" />
          <input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t('search.placeholder')}
            aria-label={t('search.placeholder')}
            className="h-full min-w-0 flex-1 bg-transparent text-[12.5px] outline-none placeholder:text-faint"
          />
        </label>
      </div>
      <div className={clsx(GRID, 'hidden gap-x-4 border-y border-line bg-surface-2 px-4 py-1.5 text-[11px] font-medium tracking-[0.06em] text-muted uppercase @2xl:grid')}>
        <span className="pl-11">{t('col.name')}</span>
        <span className="text-right">{t('col.size')}</span>
        <span className="hidden text-right @4xl:block">{t('col.speed')}</span>
        <span className="hidden text-right @4xl:block">{t('col.eta')}</span>
        <span>{t('col.status')}</span>
        <span />
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto border-t border-line @2xl:border-t-0">
        {list.length === 0 ? (
          <Empty />
        ) : (
          <ul
            ref={listRef}
            tabIndex={0}
            aria-label={t(titleKey[filter])}
            onKeyDown={onKey}
            className="outline-none focus-visible:outline-none"
          >
            {list.map((task) => (
              <Row key={task.id} task={task} selected={task.id === selectedId} onSelect={(id) => (select(id), listRef.current?.focus())} />
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
