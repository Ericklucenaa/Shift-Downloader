import clsx from 'clsx';
import { Check } from 'lucide-react';
import { useEffect, useState } from 'react';
import { reportError } from '../lib/actions';
import { PageTitle } from '../components/ui/page';
import { useT } from '../lib/i18n';
import { api } from '../lib/ipc';
import { useApp } from '../lib/store';
import type { AfterAll, Schedule as Sch } from '../lib/types';
import { Button } from '../components/ui/primitives';
import { Field, Section, Segmented, TextInput, Toggle } from '../components/ui/form';

const DAYS = [1, 2, 3, 4, 5, 6, 0];

export function Schedule() {
  const { t } = useT();
  const settings = useApp((s) => s.settings);
  const setSettings = useApp((s) => s.setSettings);
  const blocked = useApp((s) => s.stats.scheduleBlocked);
  const [draft, setDraft] = useState<Sch | null>(settings?.schedule ?? null);
  const [after, setAfter] = useState<AfterAll>(settings?.afterAll ?? 'nothing');
  const [saved, setSaved] = useState(false);
  useEffect(() => {
    setDraft(settings?.schedule ?? null);
    setAfter(settings?.afterAll ?? 'nothing');
  }, [settings]);
  if (!settings || !draft) return null;

  const dirty = JSON.stringify(draft) !== JSON.stringify(settings.schedule) || after !== settings.afterAll;
  const valid = /^\d{2}:\d{2}$/.test(draft.start) && /^\d{2}:\d{2}$/.test(draft.end);
  const save = async () => {
    try {
      const s = await api.updateSettings({ ...settings, schedule: draft, afterAll: after });
      setSettings(s);
      setSaved(true);
    } catch (e) {
      reportError(e);
    }
  };
  const toggleDay = (d: number) => {
    setSaved(false);
    setDraft({ ...draft, days: draft.days.includes(d) ? draft.days.filter((x) => x !== d) : [...draft.days, d].sort() });
  };

  return (
    <div className="min-h-0 flex-1 overflow-y-auto">
      <div className="mx-auto max-w-[840px] px-6 pt-5 pb-12">
        <div className="py-2">
          <PageTitle eyebrow={t('nav.groupTools')} title={t('nav.schedule')}>
            {saved && !dirty && (
              <span className="flex items-center gap-1 text-[12px] text-ok">
                <Check size={13} /> {t('action.saved')}
              </span>
            )}
            <Button variant="primary" disabled={!dirty || !valid} onClick={save}>
              {t('action.save')}
            </Button>
          </PageTitle>
        </div>

        {settings.schedule.enabled && (
          <p role="status" className={clsx('mt-2 flex items-center gap-2 text-[12.5px]', blocked ? 'text-warn' : 'text-ok')}>
            <span className={clsx('size-1.5 rounded-full', blocked ? 'bg-warn' : 'bg-ok')} />
            {blocked ? t('schedule.stateClosed') : t('schedule.stateOpen')}
          </p>
        )}

        <Section title={t('schedule.windowTitle')}>
          <Field label={t('schedule.enable')} help={t('schedule.enableHelp')}>
            <Toggle
              checked={draft.enabled}
              onChange={(v) => {
                setSaved(false);
                setDraft({ ...draft, enabled: v });
              }}
              label={t('schedule.enable')}
            />
          </Field>
          <Field label={t('schedule.hours')}>
            <div className="flex items-center gap-2">
              <span className="text-[12.5px] text-muted">{t('schedule.from')}</span>
              <TextInput type="time" mono value={draft.start} disabled={!draft.enabled} onChange={(e) => (setSaved(false), setDraft({ ...draft, start: e.target.value }))} aria-label={t('schedule.from')} />
              <span className="text-[12.5px] text-muted">{t('schedule.to')}</span>
              <TextInput type="time" mono value={draft.end} disabled={!draft.enabled} onChange={(e) => (setSaved(false), setDraft({ ...draft, end: e.target.value }))} aria-label={t('schedule.to')} />
            </div>
          </Field>
          <Field label={t('schedule.days')}>
            <div className="flex flex-wrap gap-1.5">
              {DAYS.map((d) => (
                <button
                  key={d}
                  type="button"
                  aria-pressed={draft.days.includes(d)}
                  disabled={!draft.enabled}
                  onClick={() => toggleDay(d)}
                  className={clsx(
                    'h-8 w-12 rounded-md border text-[12.5px] transition-colors disabled:cursor-not-allowed',
                    draft.days.includes(d) ? 'border-ink-2 bg-surface text-ink' : 'border-line bg-surface-2 text-faint hover:text-ink'
                  )}
                >
                  {t(`schedule.day${d}` as 'schedule.day0')}
                </button>
              ))}
            </div>
          </Field>
        </Section>

        <Section title={t('schedule.afterAll')}>
          <Field label={t('schedule.afterAll')} help={t('schedule.afterAllHelp')}>
            <Segmented<AfterAll>
              label={t('schedule.afterAll')}
              value={after}
              options={[
                ['nothing', t('schedule.after.nothing')],
                ['sleep', t('schedule.after.sleep')],
                ['hibernate', t('schedule.after.hibernate')],
                ['shutdown', t('schedule.after.shutdown')]
              ]}
              onChange={(v) => {
                setSaved(false);
                setAfter(v);
              }}
            />
          </Field>
        </Section>
      </div>
    </div>
  );
}
