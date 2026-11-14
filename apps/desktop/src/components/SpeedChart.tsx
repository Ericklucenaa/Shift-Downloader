import { speed } from '../lib/format';
import type { Language } from '../lib/types';

/**
 * Velocidade real, uma amostra por segundo (vem do núcleo). Sem dados inventados:
 * enquanto não há amostras, mostra a linha de base vazia.
 */
export function SpeedChart({ samples, height = 160, lang }: { samples: number[]; height?: number; lang: Language }) {
  const W = 600;
  const H = height;
  const N = 300;
  const data = samples.slice(-N);
  const peak = Math.max(0, ...data);
  // Escala "redonda" para a grade não mudar a cada segundo.
  const top = niceCeil(peak || 1);
  const x = (i: number) => W - (data.length - 1 - i) * (W / (N - 1));
  const y = (v: number) => H - 4 - (v / top) * (H - 16);
  const line = data.map((v, i) => `${i === 0 ? 'M' : 'L'}${x(i).toFixed(1)},${y(v).toFixed(1)}`).join('');
  const area = data.length > 1 ? `${line}L${x(data.length - 1)},${H}L${x(0)},${H}Z` : '';

  return (
    <svg viewBox={`0 0 ${W} ${H}`} preserveAspectRatio="none" className="block h-full w-full" role="img" aria-label={`pico ${speed(peak, lang)}`}>
      {[0.25, 0.5, 0.75].map((f) => (
        <line key={f} x1="0" x2={W} y1={y(top * f)} y2={y(top * f)} stroke="var(--line)" strokeWidth="1" vectorEffect="non-scaling-stroke" strokeDasharray="2 4" />
      ))}
      <line x1="0" x2={W} y1={H - 0.5} y2={H - 0.5} stroke="var(--line-strong)" strokeWidth="1" vectorEffect="non-scaling-stroke" />
      {peak > 0 && area && <path d={area} fill="var(--accent)" fillOpacity="0.09" />}
      {peak > 0 && data.length > 1 && <path d={line} fill="none" stroke="var(--accent)" strokeWidth="1.6" vectorEffect="non-scaling-stroke" strokeLinejoin="round" />}
    </svg>
  );
}

/** Escala do eixo (topo da grade), para o gráfico ter leitura e não só forma. */
export function chartTop(samples: number[]): number {
  return niceCeil(Math.max(0, ...samples.slice(-300)) || 1);
}

export function niceCeil(v: number): number {
  const p = 2 ** Math.ceil(Math.log2(Math.max(v, 1024)));
  return p;
}
