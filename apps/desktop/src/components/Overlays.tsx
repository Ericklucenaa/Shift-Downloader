import clsx from 'clsx';
import { AnimatePresence, motion } from 'motion/react';
import { Power, X } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { reportError, useConfirm } from '../lib/actions';
import { useT } from '../lib/i18n';
import { api } from '../lib/ipc';
import { useApp, type Toast } from '../lib/store';
import { Button, Dialog } from './ui/primitives';

export function ConfirmHost() {
  const req = useConfirm((s) => s.request);
  const set = useConfirm((s) => s.set);
  const [value, setValue] = useState('');
  const inputRef = useRef<HTMLInputElement>(null);
  useEffect(() => setValue(req?.input?.initial ?? ''), [req]);
  const close = (ok: boolean) => {
    req?.resolve(ok ? (req.input ? value : '') : null);
    set(null);
  };
  if (!req) return null;
  const focus = () => {
    if (req.input && inputRef.current) {
      const el = inputRef.current;
      el.focus();
      const dot = el.value.lastIndexOf('.');
      el.setSelectionRange(0, dot > 0 ? dot : el.value.length);
    } else document.getElementById('confirm-keep')?.focus();
  };
  return (
    <Dialog open onOpenChange={(o) => !o && close(false)} title={req.title} width={req.input ? 520 : 420} onOpenFocus={focus}>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          close(true);
        }}
      >
        <div className="px-5 py-4">
          {req.body && <p className="text-[13px] text-ink-2">{req.body}</p>}
          {req.input && (
            <label className={clsx('block', req.body && 'mt-3')}>
              <span className="text-[12px] text-muted">{req.input.label}</span>
              <input
                ref={inputRef}
                value={value}
                onChange={(e) => setValue(e.target.value)}
                spellCheck={false}
                className={clsx('mt-1 h-9 w-full rounded-md border border-line-strong bg-surface-2 px-2.5 text-[13px] outline-none focus:border-ink-2', req.input.mono && 'num')}
              />
            </label>
          )}
        </div>
        <div className="flex justify-end gap-2 border-t border-line bg-surface-2 px-5 py-3">
          <Button id="confirm-keep" variant="ghost" onClick={() => close(false)}>
            {req.cancelLabel}
          </Button>
          <Button type="submit" variant={req.tone === 'danger' ? 'danger' : 'primary'} disabled={!!req.input && !value.trim()}>
            {req.confirmLabel}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}

/** Contagem regressiva antes de suspender/hibernar/desligar (sempre cancelável). */
export function AfterAllHost() {
  const { t } = useT();
  const action = useApp((s) => s.afterAll);
  const setAfterAll = useApp((s) => s.setAfterAll);
  const [left, setLeft] = useState(60);
  useEffect(() => {
    if (!action) return;
    setLeft(60);
    const id = setInterval(() => setLeft((n) => n - 1), 1000);
    return () => clearInterval(id);
  }, [action]);
  useEffect(() => {
    if (action && left <= 0) {
      setAfterAll(null);
      api.powerAction(action).catch(reportError);
    }
  }, [left, action, setAfterAll]);
  if (!action) return null;
  const verb = t(`afterall.verb.${action}` as 'afterall.verb.sleep');
  return (
    <Dialog open onOpenChange={(o) => !o && setAfterAll(null)} title={t('afterall.title')} width={420} onOpenFocus={() => document.getElementById('afterall-cancel')?.focus()}>
      <div className="flex items-center gap-3 px-5 py-4">
        <Power size={18} className="text-accent" />
        <p className="text-[13px] text-ink-2" aria-live="polite">
          {t('afterall.body', { action: verb, n: Math.max(0, left) })}
        </p>
      </div>
      <div className="flex justify-end gap-2 border-t border-line bg-surface-2 px-5 py-3">
        <Button id="afterall-cancel" variant="ghost" onClick={() => setAfterAll(null)}>
          {t('add.cancel')}
        </Button>
        <Button variant="danger" onClick={() => setLeft(0)}>
          {t('afterall.now')}
        </Button>
      </div>
    </Dialog>
  );
}

function ToastItem({ toast }: { toast: Toast }) {
  const dismiss = useApp((s) => s.dismissToast);
  useEffect(() => {
    const id = setTimeout(() => dismiss(toast.id), toast.kind === 'error' ? 7000 : 2500);
    return () => clearTimeout(id);
  }, [toast, dismiss]);
  return (
    <motion.div
      layout
      initial={{ opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, transition: { duration: 0.12 } }}
      transition={{ duration: 0.16, ease: 'easeOut' }}
      role={toast.kind === 'error' ? 'alert' : 'status'}
      className="pointer-events-auto flex w-80 items-start gap-2.5 rounded-md bg-surface px-3 py-2.5 shadow-pop"
    >
      <span className={clsx('mt-1.5 size-1.5 shrink-0 rounded-full', toast.kind === 'error' ? 'bg-bad' : 'bg-ok')} />
      <div className="min-w-0 flex-1">
        <div className="text-[13px] font-medium text-ink">{toast.title}</div>
        {toast.body && <div className="mt-0.5 text-[12px] text-muted">{toast.body}</div>}
      </div>
      <button type="button" aria-label="Fechar" onClick={() => dismiss(toast.id)} className="text-faint hover:text-ink">
        <X size={14} />
      </button>
    </motion.div>
  );
}

export function Toasts() {
  const toasts = useApp((s) => s.toasts);
  return (
    <div className="pointer-events-none fixed right-4 bottom-10 z-50 flex flex-col items-end gap-2">
      <AnimatePresence initial={false}>
        {toasts.map((t) => (
          <ToastItem key={t.id} toast={t} />
        ))}
      </AnimatePresence>
    </div>
  );
}
