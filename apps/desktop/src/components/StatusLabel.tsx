import clsx from 'clsx';
import { useEffect, useState } from 'react';
import { duration } from '../lib/format';
import { useT } from '../lib/i18n';
import type { TaskView } from '../lib/types';

const DOT: Record<TaskView['status'], string> = {
  queued: 'bg-faint',
  probing: 'bg-faint',
  downloading: 'bg-accent',
  paused: 'bg-warn',
  verifying: 'bg-ok',
  processing: 'bg-ok',
  completed: 'bg-ok',
  failed: 'bg-bad',
  cancelled: 'bg-faint'
};

/** Contagem regressiva até a próxima tentativa automática. */
function useCountdown(target: number | null): number | null {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!target) return;
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [target]);
  return target ? Math.max(0, Math.ceil((target - now) / 1000)) : null;
}

export function StatusLabel({ task, className }: { task: TaskView; className?: string }) {
  const { t } = useT();
  const secs = useCountdown(task.status === 'failed' ? task.nextRetryAt : null);
  let label: string = t(`status.${task.status}` as const);
  if (task.status === 'failed' && secs != null) label = secs > 0 ? t('status.retryIn', { time: duration(secs) }) : t('status.retrying');
  if (task.status === 'paused' && task.pausedReason === 'schedule') label = t('status.waitingSchedule');
  return (
    <span className={clsx('inline-flex items-center gap-1.5 text-[12px] whitespace-nowrap', task.status === 'failed' ? 'text-bad' : 'text-ink-2', className)}>
      <span className={clsx('size-1.5 shrink-0 rounded-full', DOT[task.status])} aria-hidden="true" />
      {label}
    </span>
  );
}
