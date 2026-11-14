import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { Check, FolderOpen, RefreshCw } from 'lucide-react';
import { useEffect, useState } from 'react';
import { reportError } from '../lib/actions';
import { useT } from '../lib/i18n';
import { api } from '../lib/ipc';
import { useApp } from '../lib/store';
import { applyButtons, applyTheme } from '../lib/theme';
import type { BrowserStatus, DefaultsStatus, HostStat, Language, Settings as S, SpeedMode, Theme, ToolsInfo } from '../lib/types';
import { Button } from '../components/ui/primitives';
import { Field, Section, Segmented, Stepper, Toggle } from '../components/ui/form';
import { LiquidGroup } from '../components/ui/liquid';
import { BedDouble, MoonStar, UserRound } from 'lucide-react';

export function Settings() {
  const { t } = useT();
  const saved = useApp((s) => s.settings);
  const setSettings = useApp((s) => s.setSettings);
  const [draft, setDraft] = useState<S | null>(saved);
  const [state, setState] = useState<'idle' | 'saving' | 'saved'>('idle');
  const [hosts, setHosts] = useState<HostStat[]>([]);
  const [browser, setBrowser] = useState<BrowserStatus | null>(null);
  const [tools, setTools] = useState<ToolsInfo | null>(null);
  const [updating, setUpdating] = useState(false);
  const [defaults, setDefaults] = useState<DefaultsStatus | null>(null);
  const [demo, setDemo] = useState<'sleep' | 'dnd' | 'personal'>('dnd');

  useEffect(() => setDraft(saved), [saved]);
  useEffect(() => {
    api.insights().then((i) => setHosts(i.hosts.filter((h) => h.connectionLimit != null))).catch(() => {});
    api.browserStatus().then(setBrowser).catch(() => {});
    api.mediaTools().then(setTools).catch(() => {});
    api.defaultsStatus().then(setDefaults).catch(() => {});
    // Volta da tela de Aplicativos padrão do Windows: atualiza a situação.
    const onFocus = () => api.defaultsStatus().then(setDefaults).catch(() => {});
    window.addEventListener('focus', onFocus);
    return () => window.removeEventListener('focus', onFocus);
  }, []);
  if (!draft || !saved) return null;

  const dirty = JSON.stringify(draft) !== JSON.stringify(saved);
  const patch = (p: Partial<S>) => {
    setDraft({ ...draft, ...p });
    setState('idle');
  };

  const save = async (next: S = draft) => {
    setState('saving');
    try {
      const s = await api.updateSettings(next);
      setSettings(s);
      applyTheme(s.theme);
      applyButtons(s.buttonStyle);
      document.documentElement.lang = s.language;
      setState('saved');
    } catch (e) {
      reportError(e);
      setState('idle');
    }
  };

  // Tema e idioma valem na hora: são escolhas que a pessoa quer ver aplicadas.
  const instant = (p: Partial<S>) => {
    setDraft({ ...draft, ...p });
    void save({ ...saved, ...p });
  };

  const chooseDir = async () => {
    const picked = await openDialog({ directory: true, defaultPath: draft.downloadDir, title: t('add.chooseFolder') });
    if (typeof picked === 'string') patch({ downloadDir: picked });
  };

  const updateYtdlp = async () => {
    setUpdating(true);
    try {
      await api.mediaUpdateYtdlp();
      setTools(await api.mediaTools());
      useApp.getState().toast({ kind: 'info', title: t('media.updated') });
    } catch (e) {
      reportError(e);
    } finally {
      setUpdating(false);
    }
  };

  const limitKb = Math.round(draft.speedLimit / 1024);

  return (
    <div className="min-h-0 flex-1 overflow-y-auto">
      <div className="mx-auto max-w-[840px] px-6 pt-5 pb-12">
        <div className="sticky top-0 z-10 -mx-6 flex items-center gap-3 bg-bg/95 px-6 py-2">
          <h1 className="text-[15px] font-semibold tracking-[-0.01em]">{t('settings.title')}</h1>
          <div className="ml-auto flex items-center gap-2">
            {state === 'saved' && !dirty && (
              <span className="flex items-center gap-1 text-[12px] text-ok">
                <Check size={13} /> {t('action.saved')}
              </span>
            )}
            <Button variant="primary" disabled={!dirty || state === 'saving'} onClick={() => save()}>
              {t('action.save')}
            </Button>
          </div>
        </div>

        <Section title={t('settings.downloads')}>
          <Field label={t('settings.folder')} help={t('settings.folderHelp')}>
            <div className="flex items-center gap-2">
              <span className="num min-w-0 flex-1 truncate rounded-md border border-line bg-surface-2 px-2.5 py-1.5 text-[12.5px] selectable" title={draft.downloadDir}>
                {draft.downloadDir}
              </span>
              <Button icon={<FolderOpen size={14} />} onClick={chooseDir}>
                {t('add.change')}
              </Button>
            </div>
          </Field>
          <Field label={t('settings.organize')} help={t('settings.organizeHelp')}>
            <Toggle checked={draft.organizeByType} onChange={(v) => patch({ organizeByType: v })} label={t('settings.organize')} />
          </Field>
          <Field label={t('settings.concurrent')} help={t('settings.concurrentHelp')}>
            <Stepper label={t('settings.concurrent')} value={draft.maxConcurrent} min={1} max={20} onChange={(v) => patch({ maxConcurrent: v })} />
          </Field>
          <Field label={t('settings.connections')} help={t('settings.connectionsHelp')}>
            <Stepper label={t('settings.connections')} value={draft.connections} min={1} max={16} onChange={(v) => patch({ connections: v })} />
          </Field>
          <Field label={t('settings.limit')} help={t('settings.limitHelp')}>
            <div className="flex items-center gap-3">
              <Segmented
                label={t('settings.limit')}
                value={draft.speedLimit === 0 ? 'none' : 'on'}
                options={[
                  ['none', t('settings.limitNone')],
                  ['on', 'KB/s']
                ]}
                onChange={(v) => patch({ speedLimit: v === 'none' ? 0 : 1024 * 1024 })}
              />
              {draft.speedLimit > 0 && (
                <input
                  type="number"
                  min={16}
                  step={64}
                  value={limitKb}
                  aria-label="KB/s"
                  onChange={(e) => patch({ speedLimit: Math.max(16, Number(e.target.value) || 16) * 1024 })}
                  className="num h-8 w-28 rounded-md border border-line-strong bg-surface px-2 text-[13px] outline-none focus:border-ink-2"
                />
              )}
            </div>
          </Field>
          <Field label={t('settings.notify')} help={t('settings.notifyHelp')}>
            <Toggle checked={draft.notify} onChange={(v) => patch({ notify: v })} label={t('settings.notify')} />
          </Field>
        </Section>

        <Section title={t('defaults.title')}>
          <Field label={t('defaults.links')} help={t('defaults.linksHelp')}>
            {defaults && (
              <div className="grid gap-2">
                <div className="flex flex-wrap gap-x-5 gap-y-1 text-[12.5px]">
                  {([
                    ['defaults.magnet', defaults.magnetDefault],
                    ['defaults.torrent', defaults.torrentDefault]
                  ] as const).map(([k, on]) => (
                    <span key={k} className="flex items-center gap-1.5">
                      <span className={on ? 'size-1.5 rounded-full bg-ok' : 'size-1.5 rounded-full bg-faint'} />
                      {t(k)}: <span className={on ? 'text-ok' : 'text-muted'}>{on ? t('defaults.yes') : t('defaults.no')}</span>
                    </span>
                  ))}
                </div>
                <div className="flex flex-wrap gap-2">
                  {!(defaults.magnetDefault && defaults.torrentDefault) && (
                    <Button variant="primary" onClick={async () => setDefaults(await api.makeDefault().catch((e) => (reportError(e), defaults)))}>
                      {t('defaults.make')}
                    </Button>
                  )}
                  <Button onClick={() => api.openDefaultApps().catch(reportError)}>{t('defaults.openWindows')}</Button>
                </div>
                <p className="text-[11.5px] text-muted">{t('defaults.windowsNote')}</p>
              </div>
            )}
          </Field>
          <Field label={t('defaults.autostart')} help={t('defaults.autostartHelp')}>
            <Toggle
              checked={!!defaults?.autostart}
              onChange={async (v) => setDefaults(await api.setAutostart(v).catch((e) => (reportError(e), defaults)))}
              label={t('defaults.autostart')}
            />
          </Field>
          <Field label={t('settings.closeToTray')} help={t('settings.closeToTrayHelp')}>
            <Toggle checked={draft.closeToTray} onChange={(v) => instant({ closeToTray: v })} label={t('settings.closeToTray')} />
          </Field>
        </Section>

        <Section title={t('settings.speedMode')}>
          <Field label={t('settings.speedMode')} help={t('settings.speedModeHelp')}>
            <Segmented<SpeedMode>
              label={t('settings.speedMode')}
              value={draft.speedMode}
              options={[
                ['economy', t('settings.mode.economy')],
                ['balanced', t('settings.mode.balanced')],
                ['max', t('settings.mode.max')]
              ]}
              onChange={(v) => patch({ speedMode: v })}
            />
          </Field>
          <Field label={t('settings.hostLimits')}>
            {hosts.length === 0 ? (
              <span className="text-[12.5px] text-muted">{t('settings.hostLimitsNone')}</span>
            ) : (
              <div className="flex flex-wrap items-center gap-2">
                {hosts.map((h) => (
                  <span key={h.host} className="num rounded-sm bg-sunken px-1.5 py-0.5 text-[12px]">
                    {h.host} · {h.connectionLimit}
                  </span>
                ))}
                <Button
                  size="sm"
                  variant="ghost"
                  onClick={async () => {
                    await api.resetHostLimits().catch(reportError);
                    setHosts([]);
                  }}
                >
                  {t('action.reset')}
                </Button>
              </div>
            )}
          </Field>
        </Section>

        <Section title={t('settings.torrents')}>
          <Field label={t('settings.extraTrackers')} help={t('settings.extraTrackersHelp')}>
            <Toggle checked={draft.extraTrackers} onChange={(v) => patch({ extraTrackers: v })} label={t('settings.extraTrackers')} />
          </Field>
          <Field label={t('settings.seedRatio')} help={t('settings.seedRatioHelp')}>
            <Stepper label={t('settings.seedRatio')} value={draft.seedRatio} min={0} max={10} step={0.5} format={(v) => v.toFixed(1)} onChange={(v) => patch({ seedRatio: v })} />
          </Field>
        </Section>

        <Section title={t('settings.media')}>
          <Field label={t('settings.mediaJobs')}>
            <Stepper label={t('settings.mediaJobs')} value={draft.maxMediaJobs} min={1} max={4} onChange={(v) => patch({ maxMediaJobs: v })} />
          </Field>
          <Field label={t('settings.tools')} help={tools?.available === false ? t('media.unavailable') : undefined}>
            {tools?.available && (
              <div className="flex flex-wrap items-center gap-x-4 gap-y-2 text-[12.5px]">
                <span className="num">yt-dlp {tools.ytdlp}</span>
                <span className="num">FFmpeg {tools.ffmpeg}</span>
                <span className="text-muted">{tools.hwEncoder ? t('media.hw', { enc: tools.hwEncoder }) : t('media.noHw')}</span>
                <Button size="sm" icon={<RefreshCw size={13} className={updating ? 'spin' : ''} />} disabled={updating} onClick={updateYtdlp}>
                  {updating ? t('media.updating') : t('media.update')}
                </Button>
              </div>
            )}
          </Field>
        </Section>

        <Section title={t('settings.browser')}>
          <Field label={t('settings.browserConfirm')} help={t('settings.browserConfirmHelp')}>
            <Toggle checked={draft.browserConfirm} onChange={(v) => patch({ browserConfirm: v })} label={t('settings.browserConfirm')} />
          </Field>
          <Field
            label={t('settings.browser')}
            help={browser && (browser.registered.length ? t('settings.browserRegistered', { list: browser.registered.join(', ') }) : t('settings.browserNone'))}
          >
            <p className="text-[12.5px] leading-relaxed text-ink-2">{t('settings.browserSteps')}</p>
            {browser?.extensionDir && (
              <div className="mt-2 flex items-center gap-2">
                <span className="num min-w-0 flex-1 truncate rounded-md border border-line bg-surface-2 px-2.5 py-1.5 text-[12px] selectable" title={browser.extensionDir}>
                  {browser.extensionDir}
                </span>
                <Button icon={<FolderOpen size={14} />} onClick={() => api.openExtensionFolder().catch(reportError)}>
                  {t('settings.openExtension')}
                </Button>
              </div>
            )}
          </Field>
        </Section>

        <Section title={t('settings.appearance')}>
          <Field label={t('settings.theme')}>
            <Segmented<Theme>
              label={t('settings.theme')}
              value={draft.theme}
              options={[
                ['system', t('settings.themeSystem')],
                ['light', t('settings.themeLight')],
                ['dark', t('settings.themeDark')]
              ]}
              onChange={(v) => instant({ theme: v })}
            />
          </Field>
          <Field label={t('settings.buttons')} help={t('settings.buttonsHelp')}>
            <Segmented<'classic' | 'liquid'>
              label={t('settings.buttons')}
              value={draft.buttonStyle}
              options={[
                ['classic', t('settings.buttons.classic')],
                ['liquid', t('settings.buttons.liquid')]
              ]}
              onChange={(v) => instant({ buttonStyle: v })}
            />
          </Field>
          <Field label={t('liquid.preview')} help={t('liquid.previewHelp')}>
            <div className="lq-stage">
              <LiquidGroup
                label={t('liquid.preview')}
                className="lq-stage-surface"
                value={demo}
                onChange={setDemo}
                dots
                options={[
                  { value: 'sleep', label: t('liquid.sleep'), icon: <BedDouble /> },
                  { value: 'dnd', label: t('liquid.dnd'), icon: <MoonStar /> },
                  { value: 'personal', label: t('liquid.personal'), icon: <UserRound /> }
                ]}
              />
            </div>
          </Field>
          <Field label={t('settings.language')}>
            <Segmented<Language>
              label={t('settings.language')}
              value={draft.language}
              options={[
                ['pt-BR', 'Português'],
                ['en', 'English'],
                ['es', 'Español']
              ]}
              onChange={(v) => instant({ language: v })}
            />
          </Field>
        </Section>

        <h2 className="mt-8 text-[11px] font-medium tracking-[0.08em] text-muted uppercase">{t('settings.about')}</h2>
        <p className="mt-2 max-w-prose text-[12.5px] text-muted">{t('settings.aboutBody')}</p>
      </div>
    </div>
  );
}
