// Primeira abertura: como o SHIFT vai trabalhar neste computador.
import { useEffect, useState } from 'react';
import { reportError } from '../lib/actions';
import { useT } from '../lib/i18n';
import { api } from '../lib/ipc';
import { useApp } from '../lib/store';
import { Mark } from './brand';
import { Button, Dialog } from './ui/primitives';

export function WelcomeHost() {
  const { t } = useT();
  const settings = useApp((s) => s.settings);
  const [open, setOpen] = useState(false);
  const [makeDefault, setMakeDefault] = useState(true);
  const [autostart, setAutostart] = useState(false);
  const [extension, setExtension] = useState(true);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!settings || settings.onboarded) return;
    setOpen(true);
    // O instalador já perguntou sobre magnet/.torrent: a caixa vem com a resposta.
    api.defaultsStatus().then((d) => setMakeDefault(d.installerRequested || !(d.magnetDefault && d.torrentDefault))).catch(() => {});
  }, [settings]);

  if (!open || !settings) return null;

  const finish = async (apply: boolean) => {
    setBusy(true);
    try {
      if (apply) {
        if (makeDefault) await api.makeDefault();
        if (autostart) await api.setAutostart(true);
      }
      const s = await api.updateSettings({ ...settings, onboarded: true });
      useApp.getState().setSettings(s);
      if (apply && extension) {
        useApp.getState().navigate('settings');
        await api.openExtensionFolder().catch(() => {});
      }
      setOpen(false);
    } catch (e) {
      reportError(e);
    } finally {
      setBusy(false);
    }
  };

  const Option = ({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: string }) => (
    <label className="flex cursor-default items-start gap-3 rounded-md border border-line px-3 py-2.5 hover:bg-surface-2">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} className="mt-0.5 accent-[var(--accent)]" />
      <span className="text-[13px] text-ink">{label}</span>
    </label>
  );

  return (
    <Dialog open onOpenChange={(o) => !o && !busy && finish(false)} title={t('welcome.title')} description={t('welcome.body')} width={520} onOpenFocus={() => document.getElementById('welcome-apply')?.focus()}>
      <div className="grid gap-2 px-5 py-4">
        <div className="mb-1 flex items-center gap-3">
          <Mark size={28} />
          <p className="text-[12px] text-muted">{t('defaults.windowsNote')}</p>
        </div>
        <Option checked={makeDefault} onChange={setMakeDefault} label={t('welcome.default')} />
        <Option checked={autostart} onChange={setAutostart} label={t('welcome.autostart')} />
        <Option checked={extension} onChange={setExtension} label={t('welcome.extension')} />
      </div>
      <div className="flex justify-end gap-2 border-t border-line bg-surface-2 px-5 py-3">
        <Button variant="ghost" disabled={busy} onClick={() => finish(false)}>
          {t('welcome.later')}
        </Button>
        <Button id="welcome-apply" variant="primary" disabled={busy} onClick={() => finish(true)}>
          {t('welcome.apply')}
        </Button>
      </div>
    </Dialog>
  );
}
