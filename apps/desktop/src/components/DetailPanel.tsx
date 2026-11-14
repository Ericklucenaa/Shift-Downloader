import clsx from 'clsx';
import { Copy, FolderSearch, Link2, Pause, PencilLine, Play, RotateCcw, ShieldCheck, SquareArrowOutUpRight, Trash2, X } from 'lucide-react';
import { useEffect, useState } from 'react';
import { actions } from '../lib/actions';
import { bytes, dateTime, duration, percent, percentLabel, speed, time } from '../lib/format';
import { diagnosticText, useT } from '../lib/i18n';
import { api } from '../lib/ipc';
import { useApp } from '../lib/store';
import type { EventView, TaskView } from '../lib/types';
import { StatusLabel } from './StatusLabel';
import { Button, IconButton } from './ui/primitives';
import { ProgressBar } from './ui/progress';

const MAX_AUTO_RETRIES = 5;

function Fact({ label, children, mono }: { label: string; children: React.ReactNode; mono?: boolean }) {
  return (
    <div className="grid grid-cols-[92px_1fr] gap-3 py-1.5">
      <dt className="text-[12px] text-muted">{label}</dt>
      <dd className={clsx('min-w-0 text-[12.5px] break-words text-ink selectable', mono && 'num')}>{children}</dd>
    </div>
  );
}

function useEvents(task: TaskView): EventView[] {
  const [events, setEvents] = useState<EventView[]>([]);
  // Recarrega quando a situação muda (novos eventos só nascem em transições).
  useEffect(() => {
    let alive = true;
    api.events(task.id).then((e) => alive && setEvents(e)).catch(() => {});
    return () => {
      alive = false;
    };
  }, [task.id, task.status, task.retryCount, task.error?.kind, task.fileName, task.priority, task.speedLimit]);
  return events;
}

function TagEditor({ task }: { task: TaskView }) {
  const { t } = useT();
  const [draft, setDraft] = useState('');
  const add = () => {
    const v = draft.trim();
    if (v) void actions.setTags(task, [...task.tags, v]);
    setDraft('');
  };
  return (
    <div className="flex flex-wrap items-center gap-1.5">
      {task.tags.map((g) => (
        <span key={g} className="inline-flex items-center gap-1 rounded-sm bg-sunken py-0.5 pr-1 pl-1.5 text-[12px] text-ink-2">
          {g}
          <button type="button" aria-label={`${t('action.remove')}: ${g}`} onClick={() => actions.setTags(task, task.tags.filter((x) => x !== g))} className="text-faint hover:text-ink">
            <X size={11} />
          </button>
        </span>
      ))}
      <input
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            add();
          }
        }}
        onBlur={add}
        maxLength={32}
        placeholder={t('detail.tagsPlaceholder')}
        aria-label={t('detail.tags')}
        className="h-6 min-w-24 flex-1 bg-transparent text-[12px] outline-none placeholder:text-faint"
      />
    </div>
  );
}

function LimitEditor({ task }: { task: TaskView }) {
  const { t } = useT();
  const kb = Math.round(task.speedLimit / 1024);
  const [v, setV] = useState(kb ? String(kb) : '');
  useEffect(() => setV(kb ? String(kb) : ''), [kb]);
  const commit = () => {
    const n = Number(v) || 0;
    if (n !== kb) void actions.setLimit(task, n);
  };
  return (
    <span className="flex items-center gap-1.5">
      <input
        value={v}
        onChange={(e) => setV(e.target.value.replace(/[^\d]/g, ''))}
        onBlur={commit}
        onKeyDown={(e) => e.key === 'Enter' && (e.currentTarget as HTMLInputElement).blur()}
        inputMode="numeric"
        placeholder={t('detail.noLimit')}
        aria-label={t('detail.limit')}
        className="num h-6 w-24 rounded-sm border border-line bg-surface-2 px-1.5 text-[12px] outline-none focus:border-line-strong placeholder:font-sans"
      />
      <span className="text-[11.5px] text-muted">KB/s</span>
    </span>
  );
}

export function DetailPanel({ task }: { task: TaskView }) {
  const { t, lang } = useT();
  const select = useApp((s) => s.select);
  const events = useEvents(task);
  const p = percent(task.completedBytes, task.totalBytes);
  const err = task.error && task.status === 'failed' ? diagnosticText(task.error, lang) : null;
  const s = task.status;
  const http = task.kind === 'http';
  const torrent = !http;

  return (
    <aside className="flex w-[300px] shrink-0 flex-col border-l border-line bg-surface xl:w-[340px]" aria-label={task.fileName}>
      <div className="flex items-start gap-2 border-b border-line px-4 pt-4 pb-3">
        <div className="min-w-0 flex-1">
          <h2 className="text-[14px] leading-snug font-semibold break-words selectable">{task.fileName}</h2>
          <StatusLabel task={task} className="mt-1" />
        </div>
        <IconButton label={t('action.close')} onClick={() => select(null)} className="-mt-1 -mr-2">
          <X size={15} />
        </IconButton>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-4 py-4">
        {s !== 'cancelled' && (
          <div>
            <div className="flex items-baseline justify-between">
              <span className="num text-[22px] leading-none font-medium tracking-[-0.02em]">
                {s === 'completed' ? '100%' : p != null ? percentLabel(p, lang) : bytes(task.completedBytes, lang)}
              </span>
              {s === 'downloading' && <span className="num text-[13px] text-ink-2">{speed(task.speed, lang)}</span>}
            </div>
            <ProgressBar value={p} status={s} className="mt-3 h-1" />
            <div className="num mt-2 flex justify-between text-[12px] text-muted">
              <span>
                {bytes(task.completedBytes, lang)}
                {task.totalBytes != null && ` ${t('detail.of')} ${bytes(task.totalBytes, lang)}`}
              </span>
              {s === 'downloading' && task.etaSeconds != null && (
                <span>
                  {duration(task.etaSeconds)} {t('detail.remaining')}
                </span>
              )}
            </div>
          </div>
        )}

        <div className="mt-4 flex flex-wrap gap-2">
          {(s === 'downloading' || s === 'queued') && (
            <Button size="sm" icon={<Pause size={13} />} onClick={() => actions.pause(task)}>
              {t('action.pause')}
            </Button>
          )}
          {s === 'paused' && (
            <Button size="sm" variant="primary" icon={<Play size={13} />} onClick={() => actions.resume(task)}>
              {t('action.resume')}
            </Button>
          )}
          {s === 'failed' && (
            <Button size="sm" variant="primary" icon={<RotateCcw size={13} />} onClick={() => actions.retry(task)}>
              {t('action.retry')}
            </Button>
          )}
          {http && (s === 'failed' || s === 'paused') && (
            <Button size="sm" icon={<Link2 size={13} />} onClick={() => actions.changeUrl(task)}>
              {t('action.changeUrl')}
            </Button>
          )}
          {s === 'completed' && (
            <>
              <Button size="sm" variant="primary" icon={<SquareArrowOutUpRight size={13} />} onClick={() => actions.open(task)}>
                {t('action.open')}
              </Button>
              <Button size="sm" icon={<FolderSearch size={13} />} onClick={() => actions.reveal(task)}>
                {t('action.reveal')}
              </Button>
              {http && (
                <Button size="sm" variant="ghost" icon={<PencilLine size={13} />} onClick={() => actions.rename(task)}>
                  {t('action.rename')}
                </Button>
              )}
            </>
          )}
          <Button size="sm" variant="ghost" icon={<Copy size={13} />} onClick={() => actions.copyLink(task)}>
            {t('action.copyLink')}
          </Button>
        </div>

        {err && task.error && (
          <section className="mt-5 rounded-md border border-bad/25 bg-bad-soft p-3">
            <div className="text-[11px] font-medium tracking-[0.06em] text-bad uppercase">{t('detail.whatHappened')}</div>
            <p className="mt-1 text-[13px] font-medium text-ink">{err.title}</p>
            <div className="mt-2.5 text-[11px] font-medium tracking-[0.06em] text-muted uppercase">{t('detail.whatToDo')}</div>
            <p className="mt-1 text-[12.5px] text-ink-2">{err.action}</p>
            {task.nextRetryAt && <p className="mt-2 text-[12px] text-muted">{t('detail.autoRetry', { n: task.retryCount, max: MAX_AUTO_RETRIES })}</p>}
            {task.error.detail && (
              <details className="mt-2">
                <summary className="cursor-default text-[12px] text-muted">{t('detail.technical')}</summary>
                <p className="num mt-1 text-[11.5px] break-all text-muted selectable">{task.error.detail}</p>
              </details>
            )}
          </section>
        )}

        <dl className="mt-5 divide-y divide-line border-y border-line">
          <Fact label={t('detail.origin')} mono>
            {task.displayUrl}
          </Fact>
          <Fact label={t('detail.folder')} mono>
            {task.directory}
          </Fact>
          {http && (
            <Fact label={t('detail.resume')}>
              <span className={clsx(task.resumable === false && 'text-warn')}>
                {task.resumable === true ? t('detail.resumeYes') : task.resumable === false ? t('detail.resumeNo') : t('detail.resumeUnknown')}
              </span>
            </Fact>
          )}
          {s === 'downloading' && (
            <Fact label={t('detail.connections')} mono>
              {task.connections}
              {torrent && ` · ${task.seeders} ${t('torrent.seeders').toLowerCase()}`}
            </Fact>
          )}
          {torrent && (task.uploadedBytes > 0 || task.uploadSpeed > 0) && (
            <Fact label={t('torrent.uploaded')} mono>
              {bytes(task.uploadedBytes, lang)}
              {task.uploadSpeed > 0 && ` · ↑ ${speed(task.uploadSpeed, lang)}`}
            </Fact>
          )}
          {s !== 'completed' && s !== 'cancelled' && (
            <Fact label={t('detail.limit')}>
              <LimitEditor task={task} />
            </Fact>
          )}
          {task.contentType && (
            <Fact label={t('detail.type')} mono>
              {task.contentType}
            </Fact>
          )}
          {http && (task.sha256 || task.expectedSha256 || s === 'completed') && (
            <Fact label={t('detail.integrity')}>
              <span className="num block text-[11.5px] break-all">
                {task.sha256 ?? <span className="font-sans text-muted">{t('detail.sha256Pending')}</span>}
              </span>
              {task.expectedSha256 && task.sha256 === task.expectedSha256 && (
                <span className="mt-1 flex items-center gap-1 text-[12px] text-ok">
                  <ShieldCheck size={13} /> {t('detail.sha256Ok')}
                </span>
              )}
              {task.expectedSha256 && task.sha256 !== task.expectedSha256 && (
                <span className="num mt-1 block text-[11px] break-all text-muted">
                  {t('detail.sha256Expected')}: {task.expectedSha256}
                </span>
              )}
              {task.duplicate && <span className="mt-1 block text-[12px] text-warn">{t('detail.duplicate')}</span>}
            </Fact>
          )}
          <Fact label={t('detail.tags')}>
            <TagEditor task={task} />
          </Fact>
          <Fact label={t('detail.added')}>{dateTime(task.createdAt, lang)}</Fact>
          {task.finishedAt && <Fact label={t('detail.finished')}>{dateTime(task.finishedAt, lang)}</Fact>}
        </dl>

        <section className="mt-5">
          <h3 className="text-[11px] font-medium tracking-[0.06em] text-muted uppercase">{t('detail.timeline')}</h3>
          {events.length === 0 ? (
            <p className="mt-2 text-[12px] text-faint">{t('detail.noEvents')}</p>
          ) : (
            <ol className="mt-2">
              {events.map((e, i) => (
                <li key={i} className="relative grid grid-cols-[64px_1fr] gap-2 pb-2.5 text-[12px]">
                  <span className="num text-muted">{time(e.at, lang)}</span>
                  <span className="min-w-0 text-ink-2">
                    <span className="text-ink">{e.toStatus ? t(`status.${e.toStatus}` as 'status.queued') : t(`event.${e.kind}` as 'event.created')}</span>
                    {e.detail && <span className="block break-words text-muted selectable">{e.detail}</span>}
                  </span>
                </li>
              ))}
            </ol>
          )}
        </section>
      </div>

      {(s === 'downloading' || s === 'queued' || s === 'paused') && (
        <div className="border-t border-line px-4 py-3">
          <Button size="sm" variant="ghost" className="text-bad hover:bg-bad-soft hover:text-bad" icon={<X size={13} />} onClick={() => actions.cancel(task)}>
            {t('action.cancel')}
          </Button>
        </div>
      )}
      {(s === 'completed' || s === 'cancelled' || s === 'failed') && (
        <div className="border-t border-line px-4 py-3">
          <Button size="sm" variant="ghost" icon={<Trash2 size={13} />} onClick={() => actions.remove(task)}>
            {t('action.remove')}
          </Button>
        </div>
      )}
    </aside>
  );
}
