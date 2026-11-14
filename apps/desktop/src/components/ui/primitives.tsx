// Componentes base do SHIFT. Cantos discretos, linhas finas, um acento.

import * as RTooltip from '@radix-ui/react-tooltip';
import * as RDialog from '@radix-ui/react-dialog';
import * as RMenu from '@radix-ui/react-dropdown-menu';
import clsx from 'clsx';
import { X } from 'lucide-react';
import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from 'react';

type Variant = 'primary' | 'secondary' | 'ghost' | 'danger';
type Size = 'sm' | 'md';

const VARIANTS: Record<Variant, string> = {
  primary: 'bg-accent text-accent-ink hover:brightness-[1.06] active:brightness-95 border border-transparent',
  secondary: 'bg-surface text-ink border border-line-strong hover:bg-surface-2 active:bg-sunken',
  ghost: 'text-ink-2 hover:bg-sunken hover:text-ink border border-transparent',
  danger: 'bg-bad text-white hover:brightness-110 border border-transparent'
};

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
  size?: Size;
  icon?: ReactNode;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(function Button(
  { variant = 'secondary', size = 'md', icon, className, children, ...rest },
  ref
) {
  return (
    <button
      ref={ref}
      type="button"
      className={clsx(
        `btn btn-${variant}`,
        'inline-flex items-center justify-center gap-1.5 rounded-md font-medium whitespace-nowrap transition-[background,filter,color] duration-100',
        'disabled:pointer-events-none disabled:opacity-45',
        size === 'md' ? 'h-8 px-3 text-[13px]' : 'h-7 px-2.5 text-[12px]',
        VARIANTS[variant],
        className
      )}
      {...rest}
    >
      <span className="btn-label inline-flex items-center justify-center gap-1.5">
        {icon}
        {children}
      </span>
    </button>
  );
});

export const IconButton = forwardRef<HTMLButtonElement, ButtonHTMLAttributes<HTMLButtonElement> & { label: string; tone?: 'default' | 'danger' }>(
  function IconButton({ label, tone = 'default', className, children, ...rest }, ref) {
    return (
      <Tip label={label}>
        <button
          ref={ref}
          type="button"
          aria-label={label}
          className={clsx(
            'inline-flex size-7 items-center justify-center rounded-md text-muted transition-colors duration-100',
            tone === 'danger' ? 'hover:bg-bad-soft hover:text-bad' : 'hover:bg-sunken hover:text-ink',
            'disabled:pointer-events-none disabled:opacity-40',
            className
          )}
          {...rest}
        >
          {children}
        </button>
      </Tip>
    );
  }
);

export function Tip({ label, children, side = 'top' }: { label: ReactNode; children: ReactNode; side?: 'top' | 'bottom' | 'left' | 'right' }) {
  return (
    <RTooltip.Root delayDuration={400}>
      <RTooltip.Trigger asChild>{children}</RTooltip.Trigger>
      <RTooltip.Portal>
        <RTooltip.Content
          side={side}
          sideOffset={6}
          className="z-50 max-w-72 rounded-sm bg-ink px-2 py-1 text-[12px] leading-snug text-bg shadow-pop"
        >
          {label}
        </RTooltip.Content>
      </RTooltip.Portal>
    </RTooltip.Root>
  );
}

export const TipProvider = RTooltip.Provider;

export function Dialog({
  open,
  onOpenChange,
  title,
  description,
  children,
  width = 480,
  onOpenFocus
}: {
  /** Foco inicial do diálogo; sem ele, o foco fica no próprio diálogo (não no botão de fechar). */
  onOpenFocus?: () => void;
  open: boolean;
  onOpenChange: (o: boolean) => void;
  title: string;
  description?: ReactNode;
  children: ReactNode;
  width?: number;
}) {
  return (
    <RDialog.Root open={open} onOpenChange={onOpenChange}>
      <RDialog.Portal>
        <RDialog.Overlay className="fixed inset-0 z-40 bg-[rgb(10_10_12/0.42)]" />
        <RDialog.Content
          style={{ width }}
          // Cada diálogo decide o foco inicial (ex.: o campo de nome), não o botão de fechar.
          onOpenAutoFocus={(e) => {
            e.preventDefault();
            onOpenFocus?.();
          }}
          className="fixed top-[14vh] left-1/2 z-50 max-w-[calc(100vw-32px)] -translate-x-1/2 rounded-lg bg-surface shadow-pop focus:outline-none"
        >
          <div className="flex items-start justify-between gap-4 border-b border-line px-5 pt-4 pb-3">
            <div className="min-w-0">
              <RDialog.Title className="text-[15px] font-semibold tracking-[-0.01em]">{title}</RDialog.Title>
              {description ? (
                <RDialog.Description className="mt-0.5 text-[12.5px] text-muted">{description}</RDialog.Description>
              ) : (
                <RDialog.Description className="sr-only">{title}</RDialog.Description>
              )}
            </div>
            <RDialog.Close asChild>
              <IconButton label="Fechar" className="-mr-2 -mt-1">
                <X size={15} />
              </IconButton>
            </RDialog.Close>
          </div>
          {children}
        </RDialog.Content>
      </RDialog.Portal>
    </RDialog.Root>
  );
}

export function Menu({ trigger, children }: { trigger: ReactNode; children: ReactNode }) {
  return (
    <RMenu.Root modal={false}>
      <RMenu.Trigger asChild>{trigger}</RMenu.Trigger>
      <RMenu.Portal>
        <RMenu.Content
          align="end"
          sideOffset={4}
          className="z-50 min-w-48 rounded-md bg-surface p-1 shadow-pop"
          onClick={(e) => e.stopPropagation()}
        >
          {children}
        </RMenu.Content>
      </RMenu.Portal>
    </RMenu.Root>
  );
}

export function MenuItem({
  icon,
  children,
  onSelect,
  tone = 'default',
  disabled
}: {
  icon?: ReactNode;
  children: ReactNode;
  onSelect: () => void;
  tone?: 'default' | 'danger';
  disabled?: boolean;
}) {
  return (
    <RMenu.Item
      disabled={disabled}
      onSelect={onSelect}
      className={clsx(
        'flex h-8 cursor-default items-center gap-2 rounded-sm px-2 text-[13px] outline-none select-none',
        'data-[disabled]:opacity-40 data-[highlighted]:bg-sunken',
        tone === 'danger' ? 'text-bad' : 'text-ink'
      )}
    >
      <span className="flex w-4 justify-center text-muted [&>svg]:size-[15px]">{icon}</span>
      {children}
    </RMenu.Item>
  );
}

export const MenuSeparator = () => <RMenu.Separator className="my-1 h-px bg-line" />;

export function Spinner({ size = 14 }: { size?: number }) {
  return (
    <svg className="spin text-muted" width={size} height={size} viewBox="0 0 16 16" aria-hidden="true">
      <circle cx="8" cy="8" r="6" fill="none" stroke="currentColor" strokeOpacity="0.25" strokeWidth="2" />
      <path d="M14 8a6 6 0 0 0-6-6" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
    </svg>
  );
}

export function Kbd({ children }: { children: ReactNode }) {
  return (
    <kbd className="num rounded-sm border border-line-strong bg-surface-2 px-1 py-px text-[10.5px] leading-none text-muted">
      {children}
    </kbd>
  );
}
