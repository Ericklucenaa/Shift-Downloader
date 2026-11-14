import clsx from 'clsx';
import { ArrowDownToLine, CalendarClock, CircleAlert, CircleCheck, Film, Gauge, Layers, Library, Magnet, Settings } from 'lucide-react';
import type { ReactNode } from 'react';
import { useT } from '../lib/i18n';
import { counts, useApp, type Filter, type View } from '../lib/store';
import { Wordmark } from './brand';

function NavItem({ icon, label, count, active, onClick, tone }: { icon: ReactNode; label: string; count?: number; active: boolean; onClick: () => void; tone?: 'bad' | 'accent' }) {
  return (
    <button
      type="button"
      onClick={onClick}
      aria-current={active ? 'page' : undefined}
      className={clsx(
        'nav-item group relative flex h-8 w-full items-center gap-2.5 rounded-md px-2.5 text-left text-[13px] transition-colors duration-100',
        active ? 'bg-surface text-ink shadow-[0_0_0_1px_var(--line)]' : 'text-ink-2 hover:bg-sunken hover:text-ink'
      )}
    >
      {active && <span className="nav-bar absolute top-2 bottom-2 -left-3 w-[2px] rounded-full bg-accent" aria-hidden="true" />}
      <span className={clsx('nav-icon flex w-4 justify-center [&>svg]:size-[15px]', active ? 'text-ink' : 'text-muted')}>{icon}</span>
      <span className="flex-1 truncate">{label}</span>
      {count != null && count > 0 && <span className={clsx('num text-[11.5px]', tone === 'bad' ? 'text-bad' : tone === 'accent' ? 'text-accent' : 'text-muted')}>{count}</span>}
    </button>
  );
}

const Group = ({ children }: { children: ReactNode }) => (
  <div className="px-2.5 pt-4 pb-1.5 text-[11px] font-medium tracking-[0.08em] text-faint uppercase">{children}</div>
);

export function Sidebar() {
  const { t } = useT();
  const view = useApp((s) => s.view);
  const filter = useApp((s) => s.filter);
  const tasks = useApp((s) => s.tasks);
  const media = useApp((s) => s.media);
  const scheduleOn = useApp((s) => s.settings?.schedule.enabled ?? false);
  const navigate = useApp((s) => s.navigate);
  const c = counts(tasks);
  const mediaActive = Object.values(media).filter((m) => m.status === 'running' || m.status === 'queued').length;
  const isDl = (f: Filter) => view === 'downloads' && filter === f;
  const go = (v: View, f?: Filter) => () => navigate(v, f);

  return (
    <aside className="flex w-[216px] shrink-0 flex-col overflow-y-auto border-r border-line bg-bg px-3 pt-4 pb-3">
      <div className="px-2.5 pb-4">
        <Wordmark />
      </div>

      <nav className="flex flex-col gap-0.5" aria-label="SHIFT">
        <NavItem icon={<Gauge />} label={t('nav.overview')} active={view === 'overview'} onClick={go('overview')} />
        <Group>{t('nav.groupDownloads')}</Group>
        <NavItem icon={<Layers />} label={t('nav.all')} count={c.all} active={isDl('all')} onClick={go('downloads', 'all')} />
        <NavItem icon={<ArrowDownToLine />} label={t('nav.active')} count={c.active} active={isDl('active')} onClick={go('downloads', 'active')} />
        <NavItem icon={<CircleCheck />} label={t('nav.completed')} count={c.completed} active={isDl('completed')} onClick={go('downloads', 'completed')} />
        <NavItem icon={<CircleAlert />} label={t('nav.failed')} count={c.failed} tone="bad" active={isDl('failed')} onClick={go('downloads', 'failed')} />
        <NavItem icon={<Magnet />} label={t('nav.torrents')} count={c.torrents} active={isDl('torrents')} onClick={go('downloads', 'torrents')} />
        <Group>{t('nav.groupTools')}</Group>
        <NavItem icon={<Film />} label={t('nav.media')} count={mediaActive} tone="accent" active={view === 'media'} onClick={go('media')} />
        <NavItem icon={<Library />} label={t('nav.library')} active={view === 'library'} onClick={go('library')} />
        <NavItem icon={<CalendarClock />} label={t('nav.schedule')} count={scheduleOn ? 1 : undefined} tone="accent" active={view === 'schedule'} onClick={go('schedule')} />
      </nav>

      <div className="mt-auto pt-4">
        <NavItem icon={<Settings />} label={t('nav.settings')} active={view === 'settings'} onClick={go('settings')} />
      </div>
    </aside>
  );
}
