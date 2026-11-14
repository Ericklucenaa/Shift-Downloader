import clsx from 'clsx';
import type { TaskStatus } from '../../lib/types';

const FILL: Partial<Record<TaskStatus, string>> = {
  downloading: 'bg-accent',
  queued: 'bg-faint',
  paused: 'bg-warn',
  verifying: 'bg-ok',
  completed: 'bg-ok',
  failed: 'bg-bad',
  cancelled: 'bg-faint'
};

/** Barra fina. Sem percentual conhecido + baixando = faixa em movimento. */
export function ProgressBar({ value, status, className, label = 'Progresso' }: { value: number | null; status: TaskStatus; className?: string; label?: string }) {
  const indeterminate = value == null && (status === 'downloading' || status === 'probing');
  return (
    <div
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={value == null ? undefined : Math.round(value)}
      className={clsx('relative h-[3px] w-full overflow-hidden rounded-full bg-track', className)}
    >
      {indeterminate ? (
        <div className={clsx('indeterminate absolute inset-y-0 w-2/5 rounded-full', FILL[status] ?? 'bg-accent')} />
      ) : (
        <div
          className={clsx('absolute inset-y-0 left-0 rounded-full transition-[width] duration-500 ease-out', FILL[status] ?? 'bg-faint')}
          style={{ width: `${status === 'completed' ? 100 : (value ?? 0)}%` }}
        />
      )}
    </div>
  );
}
