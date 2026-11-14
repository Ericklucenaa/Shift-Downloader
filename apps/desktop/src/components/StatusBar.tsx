import clsx from 'clsx';
import { ArrowDown } from 'lucide-react';
import { speed } from '../lib/format';
import { useT, type T } from '../lib/i18n';
import { useApp } from '../lib/store';
import type { EngineState } from '../lib/types';
import { Tip } from './ui/primitives';

export function engineLabel(e: EngineState, t: T): { text: string; tone: 'ok' | 'warn' | 'bad' | 'idle' } {
  switch (e.state) {
    case 'ready':
      return { text: t('engine.ready'), tone: 'ok' };
    case 'starting':
      return { text: t('engine.starting'), tone: 'idle' };
    case 'restarting':
      return { text: t('engine.restarting'), tone: 'warn' };
    case 'failed':
      return { text: t('engine.failed'), tone: 'bad' };
    case 'stopped':
      return { text: t('engine.stopped'), tone: 'idle' };
  }
}

export const TONE_DOT = { ok: 'bg-ok', warn: 'bg-warn', bad: 'bg-bad', idle: 'bg-faint' } as const;

export function StatusBar() {
  const { t, lang } = useT();
  const engine = useApp((s) => s.engine);
  const stats = useApp((s) => s.stats);
  const e = engineLabel(engine, t);
  const detail =
    engine.state === 'ready'
      ? [t('engine.version', { v: engine.version }), engine.restarts ? t('engine.restarts', { n: engine.restarts }) : ''].filter(Boolean).join(' · ')
      : engine.state === 'failed' || engine.state === 'restarting'
        ? engine.detail
        : '';

  return (
    <footer className="flex h-7 shrink-0 items-center gap-4 border-t border-line bg-bg px-3 text-[11.5px] text-muted">
      <Tip label={detail || e.text} side="top">
        <span className="flex items-center gap-1.5" role="status">
          <span className={clsx('size-1.5 rounded-full', TONE_DOT[e.tone], e.tone === 'warn' && 'animate-pulse')} />
          <span className={clsx(e.tone === 'bad' && 'text-bad')}>{e.text}</span>
        </span>
      </Tip>
      <span className="ml-auto">{t('status.bar.tasks', { active: stats.active, queued: stats.queued })}</span>
      <span className="num flex w-28 items-center justify-end gap-1 text-ink-2">
        <ArrowDown size={12} className={stats.downloadSpeed ? 'text-accent' : 'text-faint'} />
        {stats.downloadSpeed ? speed(stats.downloadSpeed, lang) : '0 B/s'}
      </span>
    </footer>
  );
}
