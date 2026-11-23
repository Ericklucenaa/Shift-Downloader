import clsx from 'clsx';
import { ArrowRight, CalendarClock, Lightbulb, TriangleAlert } from 'lucide-react';
import { useEffect, useState } from 'react';
import { api } from '../lib/ipc';
import type { Insights } from '../lib/types';
import { bytes } from '../lib/format';
import { fileKind, percent, percentLabel, speed } from '../lib/format';
import { PageTitle } from '../components/ui/page';
import { useT } from '../lib/i18n';
import { useApp } from '../lib/store';
import { FileGlyph } from '../components/brand';
import { SpeedChart, chartTop } from '../components/SpeedChart';
import { engineLabel, TONE_DOT } from '../components/StatusBar';
import { Button } from '../components/ui/primitives';
import { ProgressBar } from '../components/ui/progress';

function Metric({ label, value, tone, onClick }: { label: string; value: number; tone?: 'bad'; onClick?: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex flex-col items-start gap-1 px-4 py-3 text-left transition-colors hover:bg-surface-2"
    >
      <span className="text-[12px] text-muted">{label}</span>
      <span className={clsx('num text-[22px] leading-none font-medium tracking-[-0.02em]', tone === 'bad' && value > 0 ? 'text-bad' : 'text-ink')}>
        {value}
      </span>
    </button>
  );
}

export function Overview() {
  const { t, lang } = useT();
  const stats = useApp((s) => s.stats);
  const tasks = useApp((s) => s.tasks);
  const engine = useApp((s) => s.engine);
  const navigate = useApp((s) => s.navigate);
  const select = useApp((s) => s.select);
  const active = Object.values(tasks).filter((x) => x.status === 'downloading').sort((a, b) => b.speed - a.speed);
  const hist = stats.speedHistory;
  const busy = hist.filter((v) => v > 0);
  const peak = Math.max(0, ...hist);
  const avg = busy.length ? busy.reduce((a, b) => a + b, 0) / busy.length : 0;
  const e = engineLabel(engine, t);

  return (
    <div className="min-h-0 flex-1 overflow-y-auto">
      <div className="mx-auto max-w-[1040px] px-6 pt-5 pb-8">
        <PageTitle eyebrow="Shift Downloader" title={t('overview.title')} />
        {stats.scheduleBlocked && (
          <button type="button" onClick={() => navigate('schedule')} className="mt-3 flex w-full items-center gap-2 rounded-md border border-warn/30 bg-warn-soft px-3 py-2 text-left text-[12.5px] text-warn">
            <CalendarClock size={14} /> {t('overview.scheduleBlocked')}
          </button>
        )}

        <section className="mt-4 grid grid-cols-[1.4fr_repeat(5,1fr)] overflow-hidden rounded-lg border border-line bg-surface max-[1100px]:grid-cols-3 [&>*]:border-line [&>*:not(:first-child)]:border-l max-[1100px]:[&>*]:border-b max-[1100px]:[&>*]:border-l-0">
          <div className="flex flex-col gap-1 px-4 py-3">
            <span className="text-[12px] text-muted">{t('overview.speedNow')}</span>
            <span className="num text-[22px] leading-none font-medium tracking-[-0.02em] text-ink">
              {stats.downloadSpeed ? speed(stats.downloadSpeed, lang) : '0 B/s'}
            </span>
            {stats.uploadSpeed > 0 && (
              <span className="num text-[11.5px] text-muted">
                {t('overview.upload')} ↑ {speed(stats.uploadSpeed, lang)}
              </span>
            )}
          </div>
          <Metric label={t('overview.active')} value={stats.active} onClick={() => navigate('downloads', 'active')} />
          <Metric label={t('overview.queued')} value={stats.queued} onClick={() => navigate('downloads', 'active')} />
          <Metric label={t('overview.paused')} value={stats.paused} onClick={() => navigate('downloads', 'active')} />
          <Metric label={t('overview.completed')} value={stats.completed} onClick={() => navigate('downloads', 'completed')} />
          <Metric label={t('overview.failed')} value={stats.failed} tone="bad" onClick={() => navigate('downloads', 'failed')} />
        </section>

        <section className="mt-4 rounded-lg border border-line bg-surface">
          <div className="flex items-baseline gap-3 px-4 pt-3">
            <h2 className="text-[13px] font-medium">{t('overview.chart')}</h2>
            {peak > 0 && (
              <span className="num ml-auto text-[11.5px] text-muted">
                {t('overview.peak', { v: speed(peak, lang) })} · {t('overview.avg', { v: speed(avg, lang) })}
              </span>
            )}
          </div>
          <div className="relative h-44 px-4 pt-3 pb-4">
            {peak > 0 && (
              <span className="num pointer-events-none absolute top-1 left-4 text-[10.5px] text-faint">{speed(chartTop(hist), lang)}</span>
            )}
            <SpeedChart samples={hist} lang={lang} height={160} />
            {peak === 0 && (
              <p className="absolute inset-0 flex items-center justify-center px-8 text-center text-[12.5px] text-faint">
                {t('overview.chartEmpty')}
              </p>
            )}
          </div>
        </section>

        <InsightsPanel />

        <div className="mt-4 grid grid-cols-[1fr_300px] gap-4 max-[1100px]:grid-cols-1">
          <section className="rounded-lg border border-line bg-surface">
            <div className="flex items-center justify-between border-b border-line px-4 py-2.5">
              <h2 className="text-[13px] font-medium">{t('overview.now')}</h2>
              <Button size="sm" variant="ghost" onClick={() => navigate('downloads', 'active')}>
                {t('overview.seeAll')} <ArrowRight size={13} />
              </Button>
            </div>
            {active.length === 0 ? (
              <p className="px-4 py-6 text-[12.5px] text-muted">{t('overview.nowEmpty')}</p>
            ) : (
              <ul>
                {active.slice(0, 6).map((task) => {
                  const p = percent(task.completedBytes, task.totalBytes);
                  return (
                    <li key={task.id}>
                      <button
                        type="button"
                        onClick={() => (navigate('downloads', 'all'), select(task.id))}
                        className="flex w-full items-center gap-3 border-b border-line px-4 py-2.5 text-left last:border-b-0 hover:bg-surface-2"
                      >
                        <FileGlyph kind={fileKind(task.fileName)} />
                        <div className="min-w-0 flex-1">
                          <div className="flex items-baseline gap-2">
                            <span className="truncate text-[13px] font-medium">{task.fileName}</span>
                            <span className="num ml-auto shrink-0 text-[12px] text-ink-2">{speed(task.speed, lang)}</span>
                          </div>
                          <div className="mt-1.5 flex items-center gap-2">
                            <ProgressBar value={p} status={task.status} />
                            <span className="num w-10 shrink-0 text-right text-[11.5px] text-muted">{percentLabel(p, lang)}</span>
                          </div>
                        </div>
                      </button>
                    </li>
                  );
                })}
              </ul>
            )}
          </section>

          <section className="h-fit rounded-lg border border-line bg-surface px-4 py-3">
            <h2 className="text-[13px] font-medium">{t('overview.engine')}</h2>
            <div className="mt-2 flex items-center gap-2 text-[13px]">
              <span className={clsx('size-2 rounded-full', TONE_DOT[e.tone])} />
              <span className={clsx(e.tone === 'bad' && 'text-bad')}>{e.text}</span>
            </div>
            {engine.state === 'ready' && (
              <p className="num mt-1 text-[12px] text-muted">
                {t('engine.version', { v: engine.version })}
                {engine.restarts > 0 && ` · ${t('engine.restarts', { n: engine.restarts })}`}
              </p>
            )}
            {engine.state === 'failed' && <p className="mt-2 text-[12.5px] text-ink-2">{t('engine.failedHelp')}</p>}
            {(engine.state === 'failed' || engine.state === 'restarting') && engine.detail && (
              <p className="num mt-2 text-[11.5px] break-words text-muted selectable">{engine.detail}</p>
            )}
          </section>
        </div>
      </div>
    </div>
  );
}

function InsightsPanel() {
  const { t, lang } = useT();
  const [data, setData] = useState<Insights | null>(null);
  const completed = useApp((s) => s.stats.completed + s.stats.failed);
  useEffect(() => {
    let alive = true;
    const load = () => api.insights().then((d) => alive && setData(d)).catch(() => {});
    load();
    const id = setInterval(load, 10_000);
    return () => {
      alive = false;
      clearInterval(id);
    };
  }, [completed]);
  return (
    <div className="mt-4 grid grid-cols-[300px_1fr] gap-4 max-[1100px]:grid-cols-1">
      <section className="h-fit rounded-lg border border-line bg-surface">
        <h2 className="flex items-center gap-2 border-b border-line px-4 py-2.5 text-[13px] font-medium">
          <Lightbulb size={14} className="text-muted" /> {t('overview.suggestions')}
        </h2>
        {!data || data.suggestions.length === 0 ? (
          <p className="px-4 py-4 text-[12.5px] text-muted">{t('overview.noSuggestions')}</p>
        ) : (
          <ul>
            {data.suggestions.map((s) => (
              <li key={s.id} className="flex gap-2.5 border-b border-line px-4 py-3 last:border-b-0">
                {s.level === 'warn' ? <TriangleAlert size={14} className="mt-0.5 shrink-0 text-warn" /> : <Lightbulb size={14} className="mt-0.5 shrink-0 text-muted" />}
                <div className="min-w-0">
                  <div className="text-[12.5px] font-medium">{s.title}</div>
                  <p className="mt-0.5 text-[12px] text-muted">{s.body}</p>
                </div>
              </li>
            ))}
          </ul>
        )}
      </section>
      <section className="rounded-lg border border-line bg-surface">
        <h2 className="border-b border-line px-4 py-2.5 text-[13px] font-medium">{t('overview.hosts')}</h2>
        {!data || data.hosts.length === 0 ? (
          <p className="px-4 py-4 text-[12.5px] text-muted">{t('overview.hostsEmpty')}</p>
        ) : (
          <table className="w-full text-[12.5px]">
            <thead>
              <tr className="text-left text-[11px] tracking-[0.06em] text-muted uppercase">
                <th className="px-4 py-2 font-medium">{t('overview.colHost')}</th>
                <th className="px-2 py-2 text-right font-medium">{t('overview.colDownloads')}</th>
                <th className="px-2 py-2 text-right font-medium">{t('overview.colBytes')}</th>
                <th className="px-2 py-2 text-right font-medium">{t('overview.colAvg')}</th>
                <th className="px-4 py-2 text-right font-medium">{t('overview.colLimit')}</th>
              </tr>
            </thead>
            <tbody>
              {data.hosts.slice(0, 8).map((h) => (
                <tr key={h.host} className="border-t border-line">
                  <td className="max-w-0 truncate px-4 py-2" title={h.host}>
                    {h.host}
                    {h.failures > 0 && <span className="ml-2 text-[11.5px] text-bad">{h.failures}×</span>}
                  </td>
                  <td className="num px-2 py-2 text-right">{h.downloads}</td>
                  <td className="num px-2 py-2 text-right">{bytes(h.bytes, lang)}</td>
                  <td className="num px-2 py-2 text-right">{h.avgSpeed ? speed(h.avgSpeed, lang) : '—'}</td>
                  <td className="num px-4 py-2 text-right">
                    {h.activeConnections > 0 ? h.activeConnections : '—'}
                    {h.connectionLimit != null && <span className="text-warn"> / {h.connectionLimit}</span>}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>
    </div>
  );
}
