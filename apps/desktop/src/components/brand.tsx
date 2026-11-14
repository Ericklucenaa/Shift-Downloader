// Marca do SHIFT: o "H" de um câmbio manual, com a marcha engatada em laranja.
import clsx from 'clsx';
import type { FileKind } from '../lib/format';
import { Archive, AppWindow, Disc3, FileText, Film, Image, Music, File } from 'lucide-react';

export function Mark({ size = 22, className }: { size?: number; className?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" className={className} aria-hidden="true">
      <rect x="0.5" y="0.5" width="23" height="23" rx="5" fill="var(--ink)" />
      <path d="M7 6.5v11M12 6.5v11M17 6.5v5.5M7 12h10" stroke="var(--bg)" strokeWidth="1.8" strokeLinecap="round" fill="none" />
      <circle cx="17" cy="17" r="2.1" fill="var(--accent)" />
    </svg>
  );
}

export function Wordmark() {
  return (
    <div className="flex items-center gap-2.5">
      <Mark />
      <span className="num text-[14px] font-medium tracking-[0.18em] text-ink">SHIFT</span>
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
