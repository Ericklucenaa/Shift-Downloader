import type { Theme } from './types';

const media = typeof matchMedia === 'function' ? matchMedia('(prefers-color-scheme: dark)') : null;
let current: Theme = 'system';

function paint() {
  const dark = current === 'dark' || (current === 'system' && !!media?.matches);
  document.documentElement.dataset.theme = dark ? 'dark' : 'light';
}

media?.addEventListener('change', () => current === 'system' && paint());

/** Estilo dos botões principais (classic | liquid). */
export function applyButtons(style: string) {
  document.documentElement.dataset.buttons = style === 'classic' ? 'classic' : 'liquid';
  try {
    localStorage.setItem('shift.buttons', style);
  } catch {
    /* só nesta sessão */
  }
}

/** Aplica e lembra o tema (o localStorage só evita o "piscar" na próxima abertura). */
export function applyTheme(theme: Theme) {
  current = theme;
  try {
    localStorage.setItem('shift.theme', theme);
  } catch {
    /* armazenamento indisponível: o tema vale só nesta sessão */
  }
  paint();
}
