// Marca do Shift Downloader: o "H" de câmbio do SHIFT original, agora com a seta de download e o
// acento turquesa da família Shift (o mesmo do Shift Control).
import clsx from 'clsx';
import type { FileKind } from '../lib/format';
import { Archive, AppWindow, Disc3, FileText, Film, Image, Music, File } from 'lucide-react';

export function Mark({ size = 26, className }: { size?: number; className?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" className={className} aria-hidden="true">
      <rect x="0.5" y="0.5" width="23" height="23" rx="6" fill="var(--accent)" />
      <path d="M7.5 6v8.5M12 6v8.5M7.5 10.25H12" stroke="var(--accent-ink)" strokeWidth="1.9" strokeLinecap="round" fill="none" />
      <path d="M16.5 6v8.5M14.2 12.3l2.3 2.4 2.3-2.4" stroke="var(--accent-ink)" strokeWidth="1.9" strokeLinecap="round" strokeLinejoin="round" fill="none" />
      <path d="M6.5 18h11" stroke="var(--accent-ink)" strokeWidth="1.9" strokeLinecap="round" />
    </svg>
  );
}

export function Wordmark() {
  return (
    <div className="flex items-center gap-2.5">
      <Mark />
      <span className="text-[14.5px] font-semibold tracking-[-0.01em] text-ink">Shift Downloader</span>
    </div>
  );
}

const GLYPH: Record<FileKind, typeof File> = {
  archive: Archive,
  video: Film,
  audio: Music,
  image: Image,
  document: FileText,
  disk: Disc3,
  program: AppWindow,
  other: File
};

export function FileGlyph({ kind, className }: { kind: FileKind; className?: string }) {
  const Icon = GLYPH[kind];
  return (
    <span className={clsx('flex size-8 shrink-0 items-center justify-center rounded-md border border-line bg-surface-2 text-ink-2', className)}>
      <Icon size={15} strokeWidth={1.7} />
    </span>
  );
}
