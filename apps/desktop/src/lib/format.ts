// Formatação de números para a interface. Unidades binárias (1 KB = 1024 B),
// como o Explorador do Windows, para os tamanhos baterem com o que o usuário vê lá.

const UNITS = ['B', 'KB', 'MB', 'GB', 'TB'];

export function bytes(n: number | null | undefined, locale = 'pt-BR'): string {
  if (n == null || !Number.isFinite(n) || n < 0) return '—';
  if (n < 1024) return `${n} B`;
  let v = n;
  let i = 0;
  while (v >= 1024 && i < UNITS.length - 1) {
    v /= 1024;
    i++;
  }
  const digits = v >= 100 ? 0 : v >= 10 ? 1 : 2;
  return `${v.toLocaleString(locale, { minimumFractionDigits: digits, maximumFractionDigits: digits })} ${UNITS[i]}`;
}

export function speed(bps: number, locale = 'pt-BR'): string {
  if (!bps) return '—';
  return `${bytes(bps, locale)}/s`;
}

/** 3725 → "1 h 02 min"; 75 → "1 min 15 s"; 9 → "9 s". */
export function duration(seconds: number | null | undefined): string {
  if (seconds == null || !Number.isFinite(seconds) || seconds < 0) return '—';
  const s = Math.round(seconds);
  if (s < 60) return `${s} s`;
  const m = Math.floor(s / 60);
  if (m < 60) return `${m} min ${String(s % 60).padStart(2, '0')} s`;
  const h = Math.floor(m / 60);
  if (h < 48) return `${h} h ${String(m % 60).padStart(2, '0')} min`;
  return `${Math.floor(h / 24)} d ${h % 24} h`;
}

export function percent(done: number, total: number | null): number | null {
  if (!total || total <= 0) return null;
  return Math.max(0, Math.min(100, (done / total) * 100));
}

export function percentLabel(p: number | null, locale = 'pt-BR'): string {
  if (p == null) return '';
  const digits = p > 0 && p < 10 ? 1 : 0;
  return `${p.toLocaleString(locale, { minimumFractionDigits: digits, maximumFractionDigits: digits })}%`;
}

export function dateTime(ms: number | null | undefined, locale = 'pt-BR'): string {
  if (!ms) return '—';
  return new Date(ms).toLocaleString(locale, { day: '2-digit', month: 'short', hour: '2-digit', minute: '2-digit' });
}

export function time(ms: number, locale = 'pt-BR'): string {
  return new Date(ms).toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit', second: '2-digit' });
}

/** Extensão em minúsculas, sem ponto ("" se não houver). */
export function extension(name: string): string {
  const i = name.lastIndexOf('.');
  return i > 0 && i < name.length - 1 ? name.slice(i + 1).toLowerCase() : '';
}

export type FileKind = 'archive' | 'video' | 'audio' | 'image' | 'document' | 'disk' | 'program' | 'other';

const KINDS: Record<Exclude<FileKind, 'other'>, string[]> = {
  archive: ['zip', '7z', 'rar', 'tar', 'gz', 'bz2', 'xz', 'zst', 'tgz'],
  video: ['mp4', 'mkv', 'webm', 'mov', 'avi', 'm4v', 'ts', 'wmv'],
  audio: ['mp3', 'flac', 'wav', 'm4a', 'ogg', 'opus', 'aac'],
  image: ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'avif', 'heic', 'bmp'],
  document: ['pdf', 'doc', 'docx', 'xls', 'xlsx', 'ppt', 'pptx', 'txt', 'csv', 'epub', 'md', 'odt'],
  disk: ['iso', 'img', 'vhd', 'vhdx', 'dmg'],
  program: ['exe', 'msi', 'msix', 'appx', 'apk', 'deb', 'rpm', 'appimage', 'bat', 'ps1']
};

export function fileKind(name: string): FileKind {
  const ext = extension(name);
  for (const [k, list] of Object.entries(KINDS)) if (list.includes(ext)) return k as FileKind;
  return 'other';
}
