import clsx from 'clsx';
import type { ReactNode } from 'react';

export function Field({ label, help, children }: { label: string; help?: ReactNode; children: ReactNode }) {
  return (
    <div className="grid grid-cols-[220px_1fr] gap-6 border-b border-line py-4 last:border-b-0 max-[900px]:grid-cols-1 max-[900px]:gap-2">
      <div>
        <div className="text-[13px] font-medium text-ink">{label}</div>
        {help && <div className="mt-1 text-[12px] leading-snug text-muted">{help}</div>}
      </div>
      <div className="min-w-0 self-center">{children}</div>
    </div>
  );
}

export function Section({ title, children, aside }: { title: string; children: ReactNode; aside?: ReactNode }) {
  return (
    <>
      <div className="mt-7 flex items-center first:mt-5">
        <h2 className="text-[11px] font-medium tracking-[0.08em] text-muted uppercase">{title}</h2>
        {aside && <div className="ml-auto">{aside}</div>}
      </div>
      <div className="mt-2 rounded-lg border border-line bg-surface px-4">{children}</div>
    </>
  );
}

export function Segmented<V extends string>({ value, options, onChange, label }: { value: V; options: [V, string][]; onChange: (v: V) => void; label: string }) {
  return (
    <div
      role="radiogroup"
      aria-label={label}
      className="seg inline-flex flex-wrap rounded-md border border-line-strong bg-surface-2 p-0.5"
      onKeyDown={(e) => {
        // Setas trocam a opção (padrão de radiogroup).
        const i = options.findIndex(([v]) => v === value);
        const d = e.key === 'ArrowRight' || e.key === 'ArrowDown' ? 1 : e.key === 'ArrowLeft' || e.key === 'ArrowUp' ? -1 : 0;
        if (!d) return;
        e.preventDefault();
        const next = options[(i + d + options.length) % options.length][0];
        onChange(next);
        requestAnimationFrame(() => (e.currentTarget.querySelector('[aria-checked="true"]') as HTMLElement | null)?.focus());
      }}
    >
      {options.map(([v, text]) => (
        <button
          key={v}
          type="button"
          role="radio"
          aria-checked={value === v}
          tabIndex={value === v ? 0 : -1}
          onClick={() => onChange(v)}
          className={clsx('seg-item h-7 rounded-[4px] px-3 text-[12.5px] transition-colors', value === v ? 'bg-surface text-ink shadow-[0_0_0_1px_var(--line-strong)]' : 'text-muted hover:text-ink')}
        >
          <span>{text}</span>
        </button>
      ))}
    </div>
  );
}

export function Stepper({ value, min, max, step = 1, onChange, label, format }: { value: number; min: number; max: number; step?: number; onChange: (v: number) => void; label: string; format?: (v: number) => string }) {
  const clamp = (v: number) => Math.min(max, Math.max(min, Math.round(v * 100) / 100));
  return (
    <div className="inline-flex h-8 items-center rounded-md border border-line-strong bg-surface">
      <button type="button" aria-label={`${label} −`} disabled={value <= min} onClick={() => onChange(clamp(value - step))} className="h-full w-8 text-muted hover:text-ink disabled:opacity-30">
        −
      </button>
      <span className="num min-w-10 px-1 text-center text-[13px]" aria-live="polite">
        {format ? format(value) : value}
      </span>
      <button type="button" aria-label={`${label} +`} disabled={value >= max} onClick={() => onChange(clamp(value + step))} className="h-full w-8 text-muted hover:text-ink disabled:opacity-30">
        +
      </button>
    </div>
  );
}

export function Toggle({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      onClick={() => onChange(!checked)}
      className={clsx('lq-switch relative inline-flex h-5 w-9 shrink-0 items-center rounded-full border transition-colors', checked ? 'border-accent bg-accent' : 'border-line-strong bg-sunken')}
    >
      <span className={clsx('absolute size-3.5 rounded-full transition-transform', checked ? 'translate-x-[18px] bg-accent-ink' : 'translate-x-[2px] bg-faint')} />
    </button>
  );
}

export function TextInput(props: React.InputHTMLAttributes<HTMLInputElement> & { mono?: boolean }) {
  const { mono, className, ...rest } = props;
  return (
    <input
      {...rest}
      className={clsx('h-8 rounded-md border border-line-strong bg-surface px-2.5 text-[13px] outline-none focus:border-ink-2 placeholder:text-faint disabled:cursor-not-allowed disabled:bg-surface-2 disabled:text-muted', mono && 'num', className)}
    />
  );
}
