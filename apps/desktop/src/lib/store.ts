// Estado da interface. O núcleo é a fonte da verdade: aqui só ficam o último
// snapshot, as atualizações por evento e escolhas de navegação.

import { create } from 'zustand';
import type { Category, EngineState, ExternalAdd, MediaView, Settings, Snapshot, Stats, TaskStatus, TaskView } from './types';

export type View = 'overview' | 'downloads' | 'media' | 'library' | 'schedule' | 'settings';
export type Filter = 'all' | 'active' | 'completed' | 'failed' | 'torrents';

export interface Toast {
  id: number;
  kind: 'info' | 'error';
  title: string;
  body?: string;
}

interface AppState {
  ready: boolean;
  loadError: string | null;
  tasks: Record<string, TaskView>;
  media: Record<string, MediaView>;
  engine: EngineState;
  stats: Stats;
  settings: Settings | null;
  view: View;
  filter: Filter;
  query: string;
  selectedId: string | null;
  toasts: Toast[];
  /** Pedido vindo do navegador esperando a tela certa tratar. */
  pendingExternal: ExternalAdd | null;
  /** Arquivo local enviado da biblioteca para a conversão. */
  pendingConvert: string | null;
  afterAll: string | null;

  loadSnapshot: (s: Snapshot, media: MediaView[]) => void;
  setLoadError: (e: string) => void;
  upsertTasks: (tasks: TaskView[]) => void;
  removeTasks: (ids: string[]) => void;
  upsertMedia: (jobs: MediaView[]) => void;
  removeMedia: (ids: string[]) => void;
  setEngine: (e: EngineState) => void;
  setStats: (s: Stats) => void;
  setSettings: (s: Settings) => void;
  navigate: (view: View, filter?: Filter) => void;
  setQuery: (q: string) => void;
  select: (id: string | null) => void;
  toast: (t: Omit<Toast, 'id'>) => void;
  dismissToast: (id: number) => void;
  setPendingExternal: (r: ExternalAdd | null) => void;
  setPendingConvert: (p: string | null) => void;
  setAfterAll: (a: string | null) => void;
}

const EMPTY_STATS: Stats = {
  downloadSpeed: 0, uploadSpeed: 0, active: 0, queued: 0, paused: 0, completed: 0, failed: 0, speedHistory: [], scheduleBlocked: false
};
let toastSeq = 0;

const byId = <T extends { id: string }>(list: T[]) => Object.fromEntries(list.map((t) => [t.id, t]));

export const useApp = create<AppState>((set) => ({
  ready: false,
  loadError: null,
  tasks: {},
  media: {},
  engine: { state: 'starting' },
  stats: EMPTY_STATS,
  settings: null,
  view: 'downloads',
  filter: 'all',
  query: '',
  selectedId: null,
  toasts: [],
  pendingExternal: null,
  pendingConvert: null,
  afterAll: null,

  loadSnapshot: (s, media) =>
    set({ ready: true, tasks: byId(s.tasks), media: byId(media), engine: s.engine, stats: s.stats, settings: s.settings }),
  setLoadError: (loadError) => set({ loadError }),
  upsertTasks: (list) => set((st) => ({ tasks: { ...st.tasks, ...byId(list) } })),
  removeTasks: (ids) =>
    set((st) => {
      const tasks = { ...st.tasks };
      for (const id of ids) delete tasks[id];
      return { tasks, selectedId: st.selectedId && ids.includes(st.selectedId) ? null : st.selectedId };
    }),
  upsertMedia: (list) => set((st) => ({ media: { ...st.media, ...byId(list) } })),
  removeMedia: (ids) =>
    set((st) => {
      const media = { ...st.media };
      for (const id of ids) delete media[id];
      return { media };
    }),
  setEngine: (engine) => set({ engine }),
  setStats: (stats) => set({ stats }),
  setSettings: (settings) => set({ settings }),
  navigate: (view, filter) =>
    set((st) => ({ view, filter: filter ?? st.filter, selectedId: view === 'downloads' ? st.selectedId : null, query: view === st.view ? st.query : '' })),
  setQuery: (query) => set({ query }),
  select: (selectedId) => set({ selectedId }),
  toast: (t) => set((st) => ({ toasts: [...st.toasts.slice(-3), { ...t, id: ++toastSeq }] })),
  dismissToast: (id) => set((st) => ({ toasts: st.toasts.filter((x) => x.id !== id) })),
  setPendingExternal: (pendingExternal) => set({ pendingExternal }),
  setPendingConvert: (pendingConvert) => set({ pendingConvert }),
  setAfterAll: (afterAll) => set({ afterAll })
}));

const ACTIVE: TaskStatus[] = ['queued', 'probing', 'downloading', 'paused', 'verifying', 'processing'];

export function matchesFilter(t: TaskView, f: Filter): boolean {
  switch (f) {
    case 'all':
      return true;
    case 'active':
      return ACTIVE.includes(t.status);
    case 'completed':
      return t.status === 'completed';
    case 'failed':
      return t.status === 'failed';
    case 'torrents':
      return t.kind !== 'http';
  }
}

/** Mais recentes primeiro. Ordem estável: a linha não pula de lugar quando muda de situação. */
export function sortTasks(list: TaskView[]): TaskView[] {
  return [...list].sort((a, b) => b.createdAt - a.createdAt || a.id.localeCompare(b.id));
}

export function visibleTasks(tasks: Record<string, TaskView>, filter: Filter, query: string): TaskView[] {
  const q = query.trim().toLocaleLowerCase();
  const list = Object.values(tasks).filter(
    (t) =>
      matchesFilter(t, filter) &&
      (!q || t.fileName.toLocaleLowerCase().includes(q) || t.host.toLocaleLowerCase().includes(q) || t.tags.some((g) => g.toLocaleLowerCase().includes(q)))
  );
  return sortTasks(list);
}

export function counts(tasks: Record<string, TaskView>): Record<Filter, number> {
  const all = Object.values(tasks);
  return {
    all: all.length,
    active: all.filter((t) => matchesFilter(t, 'active')).length,
    completed: all.filter((t) => t.status === 'completed').length,
    failed: all.filter((t) => t.status === 'failed').length,
    torrents: all.filter((t) => t.kind !== 'http').length
  };
}

/** Item da biblioteca: download concluído ou resultado de mídia. */
export interface LibraryItem {
  key: string;
  source: 'download' | 'media';
  id: string;
  name: string;
  directory: string;
  path: string;
  size: number | null;
  category: Category;
  tags: string[];
  sha256: string | null;
  duplicate: boolean;
  finishedAt: number;
  origin: string;
}

const MEDIA_EXT: Record<string, Category> = { mp4: 'video', mkv: 'video', webm: 'video', mov: 'video', mp3: 'audio', m4a: 'audio', flac: 'audio', wav: 'audio', opus: 'audio' };

function baseName(p: string): string {
  return p.split(/[\\/]/).pop() ?? p;
}

export function libraryItems(tasks: Record<string, TaskView>, media: Record<string, MediaView>): LibraryItem[] {
  const out: LibraryItem[] = [];
  for (const t of Object.values(tasks)) {
    if (t.status !== 'completed') continue;
    out.push({
      key: `d:${t.id}`, source: 'download', id: t.id, name: t.fileName, directory: t.directory,
      path: `${t.directory}\\${t.fileName}`, size: t.totalBytes ?? t.completedBytes, category: t.category, tags: t.tags,
      sha256: t.sha256, duplicate: t.duplicate, finishedAt: t.finishedAt ?? t.createdAt, origin: t.host
    });
  }
  for (const m of Object.values(media)) {
    if (m.status !== 'completed' || !m.outputPath) continue;
    const name = baseName(m.outputPath);
    const ext = name.includes('.') ? name.split('.').pop()!.toLowerCase() : '';
    out.push({
      key: `m:${m.id}`, source: 'media', id: m.id, name, directory: m.outputDir, path: m.outputPath, size: m.totalBytes,
      category: MEDIA_EXT[ext] ?? 'other', tags: [], sha256: null, duplicate: false, finishedAt: m.finishedAt ?? m.createdAt,
      origin: m.kind === 'convert' ? m.label : (() => { try { return new URL(m.source).host; } catch { return m.source; } })()
    });
  }
  return out.sort((a, b) => b.finishedAt - a.finishedAt);
}
