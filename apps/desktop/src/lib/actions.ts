// Ações sobre tarefas e mídia, com confirmação quando destroem algo e aviso em caso de erro.

import { create } from 'zustand';
import { api, asDiagnostic } from './ipc';
import { diagnosticText, translate } from './i18n';
import { useApp } from './store';
import type { MediaView, OpenResult, TaskView } from './types';

interface ConfirmRequest {
  title: string;
  body: string;
  confirmLabel: string;
  cancelLabel: string;
  tone: 'danger' | 'primary';
  /** Com campo de texto: o valor inicial. */
  input?: { label: string; initial: string; mono?: boolean };
  resolve: (value: string | null) => void;
}

export const useConfirm = create<{ request: ConfirmRequest | null; set: (r: ConfirmRequest | null) => void }>((set) => ({
  request: null,
  set: (request) => set({ request })
}));

/** Qual torrent está com a janela "Escolher arquivos" aberta. */
export const useTorrentFiles = create<{ id: string | null; set: (id: string | null) => void }>((set) => ({
  id: null,
  set: (id) => set({ id })
}));

export async function askConfirm(opts: Omit<ConfirmRequest, 'resolve' | 'input'>): Promise<boolean> {
  const v = await new Promise<string | null>((resolve) => useConfirm.getState().set({ ...opts, resolve }));
  return v !== null;
}

export function askText(opts: Omit<ConfirmRequest, 'resolve' | 'tone'> & { input: NonNullable<ConfirmRequest['input']> }): Promise<string | null> {
  return new Promise((resolve) => useConfirm.getState().set({ ...opts, tone: 'primary', resolve }));
}

const lang = () => useApp.getState().settings?.language ?? 'pt-BR';
const tr = (k: Parameters<typeof translate>[1], v?: Record<string, string | number>) => translate(lang(), k, v);

export function reportError(err: unknown) {
  const d = asDiagnostic(err);
  const text = diagnosticText(d, lang());
  useApp.getState().toast({ kind: 'error', title: text.title || tr('toast.error'), body: text.action || d.detail });
}

async function run(fn: () => Promise<unknown>) {
  try {
    await fn();
  } catch (e) {
    reportError(e);
  }
}

async function confirmDangerousOpen(open: (confirmed: boolean) => Promise<OpenResult>) {
  const r = await open(false);
  if (r.result === 'needsConfirmation') {
    const ok = await askConfirm({
      title: tr('confirm.openTitle'),
      body: tr('confirm.openBody', { name: r.fileName }),
      confirmLabel: tr('confirm.openYes'),
      cancelLabel: tr('add.cancel'),
      tone: 'danger'
    });
    if (ok) await open(true);
  }
}

export const actions = {
  pause: (t: TaskView) => run(() => api.pause(t.id)),
  resume: (t: TaskView) => run(() => api.resume(t.id)),
  retry: (t: TaskView) => run(() => api.retry(t.id)),
  pauseAll: () => run(() => api.pauseAll()),
  resumeAll: () => run(() => api.resumeAll()),
  remove: (t: TaskView) => run(() => api.remove(t.id)),
  reveal: (t: TaskView) => run(() => api.revealFile(t.id)),
  moveTop: (t: TaskView) => run(() => api.moveInQueue(t.id, 'top')),
  moveBottom: (t: TaskView) => run(() => api.moveInQueue(t.id, 'bottom')),
  setTags: (t: TaskView, tags: string[]) => run(async () => useApp.getState().upsertTasks([await api.setTags(t.id, tags)])),
  setLimit: (t: TaskView, kbps: number) => run(() => api.setSpeedLimit(t.id, Math.max(0, Math.round(kbps)) * 1024)),

  async cancel(t: TaskView) {
    const ok = await askConfirm({
      title: tr('confirm.cancelTitle'),
      body: tr('confirm.cancelBody', { name: t.fileName }),
      confirmLabel: tr('confirm.cancelYes'),
      cancelLabel: tr('confirm.keep'),
      tone: 'danger'
    });
    if (ok) await run(() => api.cancel(t.id));
  },

  open: (t: TaskView) => run(() => confirmDangerousOpen((c) => api.openFile(t.id, c))),

  async copyLink(t: TaskView) {
    await run(async () => {
      await navigator.clipboard.writeText(await api.sourceUrl(t.id));
      useApp.getState().toast({ kind: 'info', title: tr('toast.copied') });
    });
  },

  async changeUrl(t: TaskView) {
    const url = await askText({
      title: tr('detail.changeUrlTitle'),
      body: tr('detail.changeUrlHelp'),
      confirmLabel: tr('detail.apply'),
      cancelLabel: tr('add.cancel'),
      input: { label: 'URL', initial: '', mono: true }
    });
    if (url && url.trim()) await run(async () => useApp.getState().upsertTasks([await api.changeUrl(t.id, url.trim())]));
  },

  async rename(t: TaskView) {
    const name = await askText({
      title: tr('detail.renameTitle'),
      body: '',
      confirmLabel: tr('detail.apply'),
      cancelLabel: tr('add.cancel'),
      input: { label: tr('add.fileName'), initial: t.fileName }
    });
    if (name && name.trim() && name !== t.fileName) await run(async () => useApp.getState().upsertTasks([await api.rename(t.id, name.trim())]));
  },

  async moveTo(t: TaskView) {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const dir = await open({ directory: true, defaultPath: t.directory, title: tr('menu.moveTitle') });
    if (typeof dir !== 'string') return;
    await run(async () => {
      useApp.getState().upsertTasks([await api.moveDownload(t.id, dir)]);
      useApp.getState().toast({ kind: 'info', title: tr('menu.moved'), body: dir });
    });
  },

  async deleteFile(t: TaskView) {
    const ok = await askConfirm({
      title: tr('menu.deleteTitle'),
      body: tr('menu.deleteBody', { name: t.fileName }),
      confirmLabel: tr('menu.deleteYes'),
      cancelLabel: tr('confirm.keep'),
      tone: 'danger'
    });
    if (ok) await run(() => api.deleteFile(t.id));
  },

  chooseFiles: (t: TaskView) => useTorrentFiles.getState().set(t.id),

  async convert(t: TaskView, preset: 'mp3' | 'mp4') {
    await run(async () => {
      useApp.getState().upsertMedia([await api.mediaAddConvert(`${t.directory}\\${t.fileName}`, preset, {})]);
      useApp.getState().toast({ kind: 'info', title: tr('menu.converting'), body: tr('menu.convertingBody') });
    });
  },

  async scan(t: TaskView) {
    useApp.getState().toast({ kind: 'info', title: tr('menu.scanning'), body: t.fileName });
    await run(async () => {
      const r = await api.scanFile(t.id);
      useApp.getState().toast(
        r.clean ? { kind: 'info', title: tr('menu.scanClean'), body: t.fileName } : { kind: 'error', title: tr('menu.scanThreat'), body: r.detail || t.fileName }
      );
    });
  },

  toggleTag: (t: TaskView, tag: string) => actions.setTags(t, t.tags.includes(tag) ? t.tags.filter((x) => x !== tag) : [...t.tags, tag]),

  async newTag(t: TaskView) {
    const tag = await askText({
      title: tr('menu.newTagTitle'),
      body: '',
      confirmLabel: tr('detail.apply'),
      cancelLabel: tr('add.cancel'),
      input: { label: tr('detail.tags'), initial: '' }
    });
    if (tag && tag.trim()) await actions.setTags(t, [...t.tags, tag.trim()]);
  },

  /** Ação principal da linha: depende da situação. */
  primary(t: TaskView) {
    if (t.status === 'downloading' || t.status === 'queued') return actions.pause(t);
    if (t.status === 'paused') return actions.resume(t);
    if (t.status === 'failed') return actions.retry(t);
    if (t.status === 'completed') return actions.open(t);
  }
};

export const mediaActions = {
  pause: (m: MediaView) => run(() => api.mediaPause(m.id)),
  resume: (m: MediaView) => run(() => api.mediaResume(m.id)),
  remove: (m: MediaView) => run(() => api.mediaRemove(m.id)),
  reveal: (m: MediaView) => run(() => api.mediaReveal(m.id)),
  open: (m: MediaView) => run(() => confirmDangerousOpen((c) => api.mediaOpen(m.id, c))),
  async cancel(m: MediaView) {
    const ok = await askConfirm({
      title: tr('confirm.cancelTitle'),
      body: tr('confirm.cancelBody', { name: m.title }),
      confirmLabel: tr('confirm.cancelYes'),
      cancelLabel: tr('confirm.keep'),
      tone: 'danger'
    });
    if (ok) await run(() => api.mediaCancel(m.id));
  }
};
