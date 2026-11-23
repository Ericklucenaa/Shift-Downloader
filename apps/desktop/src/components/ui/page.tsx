// Cabeçalho de página no padrão do Shift Control: rótulo do grupo, título grande e legenda opcional.
import type { ReactNode } from 'react';

export function PageTitle({ eyebrow, title, caption, children }: { eyebrow?: ReactNode; title: ReactNode; caption?: ReactNode; children?: ReactNode }) {
  return (
    <div className="flex flex-wrap items-end gap-x-4 gap-y-2">
      <div className="min-w-0 flex-1">
        {eyebrow && <div className="page-eyebrow">{eyebrow}</div>}
        <h1 className="page-title mt-0.5">{title}</h1>
        {caption && <p className="page-caption mt-1">{caption}</p>}
      </div>
      {children && <div className="flex flex-wrap items-center gap-2">{children}</div>}
    </div>
  );
}
