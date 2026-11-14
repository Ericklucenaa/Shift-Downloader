// Ações de um download: a MESMA lista alimenta o botão "⋯" e o menu do botão direito.

import * as RCtx from '@radix-ui/react-context-menu';
import * as RMenu from '@radix-ui/react-dropdown-menu';
import clsx from 'clsx';
import {
  ArrowDownToLine, ArrowUpToLine, Check, ChevronRight, Copy, FileCheck2, FolderInput, FolderSearch, Link2, ListChecks,
  MoreHorizontal, Music, Pause, PencilLine, Play, RotateCcw, ShieldCheck, SquareArrowOutUpRight, Tag, Trash, Trash2, Video, X
} from 'lucide-react';
import { useMemo, type ReactNode } from 'react';
import { actions } from '../lib/actions';
import { useT, type T } from '../lib/i18n';
import { useApp } from '../lib/store';
import type { TaskView } from '../lib/types';
import { IconButton } from './ui/primitives';

type Entry =
  | { type: 'item'; label: string; icon: ReactNode; onSelect: () => void; tone?: 'danger'; checked?: boolean }
  | { type: 'sep' }
  | { type: 'sub'; label: string; icon: ReactNode; items: Entry[] };

/** Botão da ação principal da linha (pausar / continuar / tentar de novo / abrir). */
export function PrimaryAction({ task }: { task: TaskView }) {
  const { t } = useT();
  const s = task.status;
  const btn = (label: string, icon: ReactNode, fn: () => void) => (
    <IconButton label={label} onClick={(e) => (e.stopPropagation(), fn())}>
      {icon}
    </IconButton>
  );
  if (s === 'downloading' || s === 'queued') return btn(t('action.pause'), <Pause size={15} />, () => actions.pause(task));
  if (s === 'paused') return btn(t('action.resume'), <Play size={15} />, () => actions.resume(task));
  if (s === 'failed') return btn(t('action.retry'), <RotateCcw size={15} />, () => actions.retry(task));
  if (s === 'completed') return btn(t('action.open'), <SquareArrowOutUpRight size={15} />, () => actions.open(task));
  return <span className="size-7" />;
}

function buildEntries(task: TaskView, t: T, allTags: string[]): Entry[] {
  const s = task.status;
  const http = task.kind === 'http';
  const torrent = !http;
  const done = s === 'completed';
  const waiting = s === 'queued' || s === 'paused';
  const media = task.category === 'video' || task.category === 'audio';
  const out: Entry[] = [];
  const item = (label: string, icon: ReactNode, onSelect: () => void, tone?: 'danger') => out.push({ type: 'item', label, icon, onSelect, tone });
  const sep = () => out.length && out[out.length - 1].type !== 'sep' && out.push({ type: 'sep' });

  if (done) {
    item(t('action.open'), <SquareArrowOutUpRight />, () => actions.open(task));
    item(t('action.reveal'), <FolderSearch />, () => actions.reveal(task));
  }
  if (s === 'downloading' || s === 'queued') item(t('action.pause'), <Pause />, () => actions.pause(task));
  if (s === 'paused') item(t('action.resume'), <Play />, () => actions.resume(task));
  if (s === 'failed') item(t('action.retry'), <RotateCcw />, () => actions.retry(task));
  sep();
  if (torrent && s !== 'cancelled') item(t('menu.chooseFiles'), <ListChecks />, () => actions.chooseFiles(task));
  if (waiting)
    out.push({
      type: 'sub',
      label: t('menu.priority'),
      icon: <ArrowUpToLine />,
      items: [
        { type: 'item', label: t('action.moveTop'), icon: <ArrowUpToLine />, onSelect: () => actions.moveTop(task) },
        { type: 'item', label: t('action.moveBottom'), icon: <ArrowDownToLine />, onSelect: () => actions.moveBottom(task) }
      ]
    });
  sep();
  if (done) {
    item(t('menu.moveTo'), <FolderInput />, () => actions.moveTo(task));
    if (http) item(t('action.rename'), <PencilLine />, () => actions.rename(task));
    item(t('menu.deleteFile'), <Trash />, () => actions.deleteFile(task), 'danger');
  }
  if (done || s === 'cancelled' || s === 'failed') item(t('action.remove'), <Trash2 />, () => actions.remove(task), s === 'failed' ? 'danger' : undefined);
  sep();
  item(t('action.copyLink'), <Copy />, () => actions.copyLink(task));
  if (http && (s === 'failed' || waiting)) item(t('action.changeUrl'), <Link2 />, () => actions.changeUrl(task));
  if (done && media) {
    sep();
    item(t('menu.convertMp3'), <Music />, () => actions.convert(task, 'mp3'));
    if (task.category === 'video') item(t('menu.convertMp4'), <Video />, () => actions.convert(task, 'mp4'));
  }
  if (done) {
    sep();
    item(t('menu.scan'), <ShieldCheck />, () => actions.scan(task));
  }
  sep();
  const tags = [...new Set([...allTags, ...task.tags])].sort();
  out.push({
    type: 'sub',
    label: t('menu.tag'),
    icon: <Tag />,
    items: [
      ...tags.map((g): Entry => ({ type: 'item', label: g, icon: <Tag />, checked: task.tags.includes(g), onSelect: () => actions.toggleTag(task, g) })),
      ...(tags.length ? [{ type: 'sep' } as Entry] : []),
      { type: 'item', label: t('menu.newTag'), icon: <FileCheck2 />, onSelect: () => actions.newTag(task) }
    ]
  });
  if (s === 'downloading' || waiting) {
    sep();
    item(t('action.cancel'), <X />, () => actions.cancel(task), 'danger');
  }
  while (out.length && out[out.length - 1].type === 'sep') out.pop();
  return out;
}

function useEntries(task: TaskView): Entry[] {
  const { t } = useT();
  const tasks = useApp((s) => s.tasks);
  const allTags = useMemo(() => [...new Set(Object.values(tasks).flatMap((x) => x.tags))], [tasks]);
  return buildEntries(task, t, allTags);
}

const itemClass = (tone?: 'danger') =>
  clsx(
    'flex h-8 cursor-default items-center gap-2 rounded-sm px-2 text-[13px] outline-none select-none',
    'data-[disabled]:opacity-40 data-[highlighted]:bg-sunken data-[state=open]:bg-sunken',
    tone === 'danger' ? 'text-bad' : 'text-ink'
  );
const panelClass = 'z-50 min-w-56 rounded-md bg-surface p-1 shadow-pop';
const Icon = ({ children }: { children: ReactNode }) => <span className="flex w-4 justify-center text-muted [&>svg]:size-[15px]">{children}</span>;

/** Renderiza a mesma árvore com os primitivos do Radix (dropdown ou contexto). */
function Entries({ entries, kit }: { entries: Entry[]; kit: typeof RMenu | typeof RCtx }) {
  const K = kit as typeof RMenu;
  return (
    <>
      {entries.map((e, i) => {
        if (e.type === 'sep') return <K.Separator key={i} className="my-1 h-px bg-line" />;
        if (e.type === 'sub')
          return (
            <K.Sub key={i}>
              <K.SubTrigger className={itemClass()}>
                <Icon>{e.icon}</Icon>
                <span className="flex-1">{e.label}</span>
                <ChevronRight size={14} className="text-muted" />
              </K.SubTrigger>
              <K.Portal>
                <K.SubContent className={panelClass} sideOffset={4} onClick={(ev) => ev.stopPropagation()}>
                  <Entries entries={e.items} kit={kit} />
                </K.SubContent>
              </K.Portal>
            </K.Sub>
          );
        return (
          <K.Item key={i} className={itemClass(e.tone)} onSelect={e.onSelect}>
            <Icon>{e.checked ? <Check /> : e.icon}</Icon>
            <span className="flex-1">{e.label}</span>
          </K.Item>
        );
      })}
    </>
  );
}

/** Botão "⋯" da linha. */
export function TaskMenu({ task }: { task: TaskView }) {
  const { t } = useT();
  const entries = useEntries(task);
  return (
    <RMenu.Root modal={false}>
      <RMenu.Trigger asChild>
        <IconButton label={t('action.more')} onClick={(e) => e.stopPropagation()}>
          <MoreHorizontal size={15} />
        </IconButton>
      </RMenu.Trigger>
      <RMenu.Portal>
        <RMenu.Content align="end" sideOffset={4} className={panelClass} onClick={(e) => e.stopPropagation()}>
          <Entries entries={entries} kit={RMenu} />
        </RMenu.Content>
      </RMenu.Portal>
    </RMenu.Root>
  );
}

/** Botão direito em cima do download: mesmo menu do "⋯". */
export function TaskContextMenu({ task, children, onOpen }: { task: TaskView; children: ReactNode; onOpen?: () => void }) {
  const entries = useEntries(task);
  return (
    <RCtx.Root onOpenChange={(o) => o && onOpen?.()}>
      <RCtx.Trigger asChild>{children}</RCtx.Trigger>
      <RCtx.Portal>
        <RCtx.Content className={panelClass} onClick={(e) => e.stopPropagation()}>
          <Entries entries={entries} kit={RCtx} />
        </RCtx.Content>
      </RCtx.Portal>
    </RCtx.Root>
  );
}
