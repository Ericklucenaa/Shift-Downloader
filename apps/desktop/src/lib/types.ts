// Espelho dos tipos de crates/core (views.rs, media.rs, torrent.rs, diagnostics.rs) — serde camelCase.

export type TaskStatus =
  | 'queued'
  | 'probing'
  | 'downloading'
  | 'paused'
  | 'verifying'
  | 'processing'
  | 'completed'
  | 'failed'
  | 'cancelled';

export type DiagnosticKind =
  | 'dns' | 'connection' | 'timeout' | 'tls' | 'http_auth' | 'forbidden' | 'not_found' | 'expired'
  | 'server_error' | 'server_busy' | 'too_slow' | 'disk_full' | 'permission' | 'file_exists' | 'io'
  | 'resume_unsupported' | 'remote_changed' | 'redirect_blocked' | 'too_many_redirects' | 'checksum'
  | 'invalid_url' | 'engine' | 'unsupported_url' | 'drm' | 'format_unavailable' | 'media_failed'
  | 'tool_missing' | 'unknown';

export interface Diagnostic {
  kind: DiagnosticKind;
  title: string;
  action: string;
  detail: string;
  retryable: boolean;
}

export type Category = 'video' | 'audio' | 'image' | 'document' | 'archive' | 'disk' | 'program' | 'other';

export interface TaskView {
  id: string;
  kind: 'http' | 'torrent' | 'magnet';
  fileName: string;
  directory: string;
  host: string;
  displayUrl: string;
  status: TaskStatus;
  totalBytes: number | null;
  completedBytes: number;
  speed: number;
  connections: number;
  etaSeconds: number | null;
  resumable: boolean | null;
  contentType: string | null;
  error: Diagnostic | null;
  retryCount: number;
  nextRetryAt: number | null;
  createdAt: number;
  startedAt: number | null;
  finishedAt: number | null;
  category: Category;
  tags: string[];
  sha256: string | null;
  expectedSha256: string | null;
  speedLimit: number;
  priority: number;
  pausedReason: 'user' | 'schedule' | 'override' | null;
  uploadSpeed: number;
  uploadedBytes: number;
  seeders: number;
  duplicate: boolean;
}

export type EngineState =
  | { state: 'starting' }
  | { state: 'ready'; version: string; restarts: number }
  | { state: 'restarting'; detail: string }
  | { state: 'failed'; detail: string }
  | { state: 'stopped' };

export interface Stats {
  downloadSpeed: number;
  uploadSpeed: number;
  active: number;
  queued: number;
  paused: number;
  completed: number;
  failed: number;
  speedHistory: number[];
  scheduleBlocked: boolean;
}

export type Theme = 'system' | 'light' | 'dark';
export type Language = 'pt-BR' | 'en' | 'es';
export type SpeedMode = 'economy' | 'balanced' | 'max';
export type AfterAll = 'nothing' | 'sleep' | 'hibernate' | 'shutdown';

export interface Schedule {
  enabled: boolean;
  start: string;
  end: string;
  days: number[];
}

export interface Settings {
  downloadDir: string;
  maxConcurrent: number;
  connections: number;
  speedLimit: number;
  theme: Theme;
  language: Language;
  organizeByType: boolean;
  notify: boolean;
  speedMode: SpeedMode;
  seedRatio: number;
  schedule: Schedule;
  afterAll: AfterAll;
  browserConfirm: boolean;
  maxMediaJobs: number;
  buttonStyle: 'classic' | 'liquid';
  onboarded: boolean;
  closeToTray: boolean;
  extraTrackers: boolean;
}

export interface TorrentMeta {
  infoHash: string;
  files: TorrentFile[];
  selected: number[];
  wrapper?: string | null;
}

export interface DefaultsStatus {
  magnetDefault: boolean;
  torrentDefault: boolean;
  autostart: boolean;
  installerRequested: boolean;
}

export interface ProbeView {
  probeId: string;
  host: string;
  displayUrl: string;
  fileName: string;
  directory: string;
  totalBytes: number | null;
  resumable: boolean | null;
  contentType: string | null;
  redirects: number;
  freeSpace: number | null;
  renamed: boolean;
  category: Category;
}

export interface EventView {
  at: number;
  kind: string;
  fromStatus: string | null;
  toStatus: string | null;
  detail: string | null;
}

export interface Snapshot {
  tasks: TaskView[];
  engine: EngineState;
  stats: Stats;
  settings: Settings;
}

export interface AddRequest {
  url: string;
  probeId?: string;
  fileName?: string;
  directory?: string;
  startPaused?: boolean;
  expectedSha256?: string;
  speedLimit?: number;
}

export interface BatchResult {
  url: string;
  task: TaskView | null;
  error: Diagnostic | null;
}

export interface TorrentFile {
  index: number;
  path: string;
  length: number;
}

export interface TorrentInfo {
  name: string;
  infoHash: string;
  files: TorrentFile[];
  totalBytes: number;
  singleFile: boolean;
}

export interface TorrentRequest {
  source: string;
  selected?: number[];
  directory?: string;
  startPaused?: boolean;
}

export interface HostStat {
  host: string;
  downloads: number;
  bytes: number;
  avgSpeed: number;
  failures: number;
  connectionLimit: number | null;
  activeConnections: number;
}

export interface Suggestion {
  id: string;
  level: 'info' | 'warn';
  title: string;
  body: string;
}

export interface Insights {
  hosts: HostStat[];
  suggestions: Suggestion[];
}

export type OpenResult = { result: 'opened' } | { result: 'needsConfirmation'; fileName: string };

// ---- mídia ----

export type MediaStatus = 'queued' | 'running' | 'paused' | 'completed' | 'failed' | 'cancelled';

export interface MediaView {
  id: string;
  kind: 'download' | 'convert';
  title: string;
  source: string;
  status: MediaStatus;
  progress: number;
  phase: '' | 'video' | 'audio' | 'processing' | 'converting';
  speed: number | null;
  eta: number | null;
  totalBytes: number | null;
  outputPath: string | null;
  outputDir: string;
  thumbnail: string | null;
  duration: number | null;
  label: string;
  item: number | null;
  items: number | null;
  error: Diagnostic | null;
  createdAt: number;
  finishedAt: number | null;
}

export interface MediaChoice {
  id: string;
  label: string;
  kind: 'video' | 'audio';
  approxSize: number | null;
}

export interface MediaInfo {
  id: string;
  title: string;
  uploader: string | null;
  duration: number | null;
  thumbnail: string | null;
  webpageUrl: string;
  extractor: string;
  isPlaylist: boolean;
  entries: number;
  choices: MediaChoice[];
  hasSubtitles: boolean;
}

export interface MediaDownloadRequest {
  url: string;
  choice: string;
  choiceLabel?: string;
  title: string;
  videoId?: string;
  thumbnail?: string | null;
  duration?: number | null;
  playlist?: boolean;
  subtitles?: boolean;
  directory?: string;
}

export interface FileInfo {
  duration: number | null;
  size: number | null;
  format: string | null;
  video: string | null;
  width: number | null;
  height: number | null;
  audio: string | null;
}

export interface PresetView {
  id: string;
  label: string;
  ext: string;
  audioOnly: boolean;
  remux: boolean;
  approxSize: number | null;
}

export interface FileProbe {
  path: string;
  name: string;
  info: FileInfo;
  presets: PresetView[];
  hwEncoder: string | null;
}

export interface ConvertOptions {
  height?: number | null;
  crf?: number | null;
  audioKbps?: number | null;
  start?: number | null;
  end?: number | null;
}

export interface ToolsInfo {
  available: boolean;
  ytdlp: string | null;
  ffmpeg: string | null;
  hwEncoder: string | null;
}

export interface BrowserStatus {
  registered: string[];
  extensionDir: string | null;
  extensionId: string;
  error: string | null;
}

export interface ExternalAdd {
  url: string;
  kind?: 'file' | 'media' | 'torrent-file' | null;
  fileName?: string | null;
  referrer?: string | null;
}

export interface UpdateInfo {
  version: string;
  current: string;
  notes: string | null;
  date: string | null;
}
