// Marca do Shift Downloader: o ícone oficial do app (o "S" com a seta de download), o mesmo do
// executável, do instalador e da extensão. Fonte: assets/icon-source.png.
import clsx from 'clsx';
import type { FileKind } from '../lib/format';
import { Archive, AppWindow, Disc3, FileText, Film, Image, Music, File } from 'lucide-react';
import logo from '../assets/logo.png';

export function Mark({ size = 28, className }: { size?: number; className?: string }) {
  return <img src={logo} width={size} height={size} alt="" aria-hidden="true" draggable={false} className={clsx('shrink-0 select-none', className)} />;
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
