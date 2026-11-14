// LiquidButton: cápsula de acrílico/gel com espessura, reflexo e luz interna.
// O visual está em src/liquid.css; aqui ficam semântica, teclado e estados.

import { forwardRef, useRef, type ButtonHTMLAttributes, type KeyboardEvent, type ReactNode } from 'react';

export interface LiquidButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  icon?: ReactNode;
  /** Pontos decorativos à direita (como na referência). */
  dots?: boolean;
  /** Aceso permanentemente (ação principal). */
  lit?: boolean;
  size?: 'sm' | 'md';
}

export const LiquidButton = forwardRef<HTMLButtonElement, LiquidButtonProps>(function LiquidButton(
  { icon, dots, lit, size = 'md', className, children, type = 'button', ...rest },
  ref
) {
  return (
    <button ref={ref} type={type} data-lit={lit ? 'true' : undefined} data-size={size} className={['lq', className].filter(Boolean).join(' ')} {...rest}>
      {icon && (
        <span className="lq-icon" aria-hidden="true">
          {icon}
        </span>
      )}
      <span className="lq-label">{children}</span>
      {dots && (
        <span className="lq-dots" aria-hidden="true">
          <i />
          <i />
          <i />
        </span>
      )}
    </button>
  );
});

export interface LiquidOption<V extends string> {
  value: V;
  label: string;
  icon?: ReactNode;
}

/** Grupo em que só uma cápsula fica acesa (radiogroup com setas do teclado). */
export function LiquidGroup<V extends string>({
  label,
  value,
  options,
  onChange,
  className,
  dots
}: {
  label: string;
  value: V;
  options: LiquidOption<V>[];
  onChange: (v: V) => void;
  className?: string;
  dots?: boolean;
}) {
  const refs = useRef<(HTMLButtonElement | null)[]>([]);
  const onKey = (e: KeyboardEvent) => {
    const i = options.findIndex((o) => o.value === value);
    const d = e.key === 'ArrowRight' || e.key === 'ArrowDown' ? 1 : e.key === 'ArrowLeft' || e.key === 'ArrowUp' ? -1 : 0;
    if (!d) return;
    e.preventDefault();
    const n = (i + d + options.length) % options.length;
    onChange(options[n].value);
    refs.current[n]?.focus();
  };
  return (
    <div role="radiogroup" aria-label={label} className={className} onKeyDown={onKey}>
      {options.map((o, i) => (
        <LiquidButton
          key={o.value}
          ref={(el) => {
            refs.current[i] = el;
          }}
          role="radio"
          aria-checked={o.value === value}
          tabIndex={o.value === value ? 0 : -1}
          icon={o.icon}
          dots={dots}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </LiquidButton>
      ))}
    </div>
  );
}
