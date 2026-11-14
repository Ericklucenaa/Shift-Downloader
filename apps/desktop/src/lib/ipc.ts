// Única porta de entrada para o núcleo. Comandos e eventos espelham
// apps/desktop/src-tauri/src/commands.rs e sink.rs.

import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  AddRequest, BatchResult, BrowserStatus, DefaultsStatus, TorrentMeta, ConvertOptions, Diagnostic, EngineState, EventView, ExternalAdd, FileProbe,
  Insights, MediaDownloadRequest, MediaInfo, MediaView, OpenResult, ProbeView, Settings, Snapshot, Stats, TaskView,
  ToolsInfo, TorrentInfo, TorrentRequest
} from './types';

export const api = {
  snapshot: () => invoke<Snapshot>('get_snapshot'),
  probe: (url: string) => invoke<ProbeView>('probe_url', { url }),
  add: (request: AddRequest) => invoke<TaskView>('add_download', { request }),
  addBatch: (urls: string[], directory: string | undefined, startPaused: boolean) =>
    invoke<BatchResult[]>('add_batch', { urls, directory, startPaused }),
  torrentPreview: (path: string) => invoke<TorrentInfo>('torrent_preview', { path }),
  addTorrent: (request: TorrentRequest) => invoke<TaskView>('add_torrent', { request }),
  pause: (id: string) => invoke<void>('pause_download', { id }),
  resume: (id: string) => invoke<void>('resume_download', { id }),
  cancel: (id: string) => invoke<void>('cancel_download', { id }),
  retry: (id: string) => invoke<void>('retry_download', { id }),
  remove: (id: string) => invoke<void>('remove_download', { id }),
  pauseAll: () => invoke<void>('pause_all'),
  resumeAll: () => invoke<void>('resume_all'),
  changeUrl: (id: string, url: string) => invoke<TaskView>('change_url', { id, url }),
  moveInQueue: (id: string, to: 'top' | 'bottom') => invoke<void>('move_in_queue', { id, to }),
  setSpeedLimit: (id: string, bps: number) => invoke<void>('set_speed_limit', { id, bps }),
  rename: (id: string, name: string) => invoke<TaskView>('rename_download', { id, name }),
  setTags: (id: string, tags: string[]) => invoke<TaskView>('set_tags', { id, tags }),
  events: (id: string) => invoke<EventView[]>('task_events', { id }),
  insights: () => invoke<Insights>('insights'),
  resetHostLimits: () => invoke<void>('reset_host_limits'),
  updateSettings: (settings: Settings) => invoke<Settings>('update_settings', { settings }),
  openFile: (id: string, confirmed = false) => invoke<OpenResult>('open_file', { id, confirmed }),
  revealFile: (id: string) => invoke<void>('reveal_file', { id }),
  sourceUrl: (id: string) => invoke<string>('source_url', { id }),

  mediaList: () => invoke<MediaView[]>('media_list'),
  mediaAnalyze: (url: string) => invoke<MediaInfo>('media_analyze', { url }),
  mediaAddDownload: (request: MediaDownloadRequest) => invoke<MediaView>('media_add_download', { request }),
  mediaProbeFile: (path: string) => invoke<FileProbe>('media_probe_file', { path }),
  mediaAddConvert: (path: string, preset: string, options: ConvertOptions) =>
    invoke<MediaView>('media_add_convert', { request: { path, preset, options } }),
  mediaPause: (id: string) => invoke<void>('media_pause', { id }),
  mediaResume: (id: string) => invoke<void>('media_resume', { id }),
  mediaCancel: (id: string) => invoke<void>('media_cancel', { id }),
  mediaRemove: (id: string) => invoke<void>('media_remove', { id }),
  mediaOpen: (id: string, confirmed = false) => invoke<OpenResult>('media_open', { id, confirmed }),
  mediaReveal: (id: string) => invoke<void>('media_reveal', { id }),
  mediaTools: () => invoke<ToolsInfo>('media_tools'),
  mediaUpdateYtdlp: () => invoke<string>('media_update_ytdlp'),

  browserStatus: () => invoke<BrowserStatus>('browser_status'),
  openExtensionFolder: () => invoke<void>('open_extension_folder'),
  powerAction: (action: string) => invoke<void>('power_action', { action }),
  takeLaunchTarget: () => invoke<ExternalAdd | null>('take_launch_target'),
  defaultsStatus: () => invoke<DefaultsStatus>('defaults_status'),
  makeDefault: () => invoke<DefaultsStatus>('make_default'),
  openDefaultApps: () => invoke<void>('open_default_apps'),
  setAutostart: (on: boolean) => invoke<DefaultsStatus>('set_autostart', { on }),
  moveDownload: (id: string, directory: string) => invoke<TaskView>('move_download', { id, directory }),
  deleteFile: (id: string) => invoke<void>('delete_file', { id }),
  torrentFiles: (id: string) => invoke<TorrentMeta>('torrent_files', { id }),
  setTorrentFiles: (id: string, selected: number[]) => invoke<TaskView>('set_torrent_files', { id, selected }),
  scanFile: (id: string, media = false) => invoke<{ clean: boolean; detail: string }>('scan_file', { id, media })
};

export interface CoreEvents {
  onTasks: (tasks: TaskView[]) => void;
  onRemoved: (ids: string[]) => void;
  onEngine: (state: EngineState) => void;
  onStats: (stats: Stats) => void;
  onMedia: (jobs: MediaView[]) => void;
  onMediaRemoved: (ids: string[]) => void;
  onAfterAll: (action: string) => void;
  onExternalAdd: (req: ExternalAdd) => void;
}

export async function subscribe(h: CoreEvents): Promise<UnlistenFn> {
  const offs = await Promise.all([
    listen<TaskView[]>('shift://tasks', (e) => h.onTasks(e.payload)),
    listen<string[]>('shift://removed', (e) => h.onRemoved(e.payload)),
    listen<EngineState>('shift://engine', (e) => h.onEngine(e.payload)),
    listen<Stats>('shift://stats', (e) => h.onStats(e.payload)),
    listen<MediaView[]>('shift://media', (e) => h.onMedia(e.payload)),
    listen<string[]>('shift://media-removed', (e) => h.onMediaRemoved(e.payload)),
    listen<string>('shift://after-all', (e) => h.onAfterAll(e.payload)),
    listen<ExternalAdd>('shift://external-add', (e) => h.onExternalAdd(e.payload))
  ]);
  return () => offs.forEach((off) => off());
}

/** Erros do núcleo chegam como Diagnostic; qualquer outra coisa vira um diagnóstico genérico. */
export function asDiagnostic(err: unknown): Diagnostic {
  if (err && typeof err === 'object' && 'kind' in err && 'title' in err) return err as Diagnostic;
  return { kind: 'unknown', title: String(err), action: '', detail: '', retryable: false };
}
