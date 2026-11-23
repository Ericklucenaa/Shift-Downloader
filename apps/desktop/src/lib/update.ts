// Atualização do próprio app. A busca e a instalação acontecem no Rust (assinatura conferida lá);
// aqui fica só o estado que a barra lateral e as Configurações mostram.

import { getVersion } from '@tauri-apps/api/app';
import { listen } from '@tauri-apps/api/event';
import { useEffect } from 'react';
import { create } from 'zustand';
import { translate } from './i18n';
import { api } from './ipc';
import { useApp } from './store';
import type { UpdateInfo } from './types';

export type UpdatePhase = 'idle' | 'checking' | 'none' | 'available' | 'downloading' | 'installing' | 'error';

interface UpdateState {
  phase: UpdatePhase;
  current: string | null;
  info: UpdateInfo | null;
  downloaded: number;
  total: number | null;
  error: string | null;
}

export const useUpdate = create<UpdateState>(() => ({
  phase: 'idle', current: null, info: null, downloaded: 0, total: null, error: null
}));

const message = (e: unknown) => String((e as { detail?: string })?.detail ?? e);

/** `manual`: o usuário clicou em procurar, então erros e "já está atualizado" aparecem. */
export async function checkForUpdate(manual = false) {
  const st = useUpdate.getState();
  if (st.phase === 'checking' || st.phase === 'downloading' || st.phase === 'installing') return;
  useUpdate.setState({ phase: 'checking', error: null });
  try {
    const info = await api.appUpdateCheck();
    useUpdate.setState({ phase: info ? 'available' : 'none', info });
    if (info) announce(info);
  } catch (e) {
    // Verificação automática sem internet não deve incomodar ninguém.
    useUpdate.setState({ phase: manual ? 'error' : 'idle', error: manual ? message(e) : null });
  }
}

export async function installUpdate() {
  useUpdate.setState({ phase: 'downloading', downloaded: 0, total: null, error: null });
  const off = await listen<{ downloaded: number; total: number | null }>('shift://update-progress', (e) => {
    const { downloaded, total } = e.payload;
    useUpdate.setState({ downloaded, total, phase: total != null && downloaded >= total ? 'installing' : 'downloading' });
  });
  try {
    await api.appUpdateInstall(); // em caso de sucesso o app fecha e reabre já atualizado
  } catch (e) {
    useUpdate.setState({ phase: 'error', error: message(e) });
  } finally {
    off();
  }
}

/** Avisa uma vez por versão, para não repetir a cada verificação periódica. */
function announce(info: UpdateInfo) {
  let seen: string | null = null;
  try { seen = localStorage.getItem('shift.update.seen'); } catch { /* sem armazenamento */ }
  if (seen === info.version) return;
  try { localStorage.setItem('shift.update.seen', info.version); } catch { /* idem */ }
  const app = useApp.getState();
  const lang = app.settings?.language ?? 'pt-BR';
  app.toast({ kind: 'info', title: translate(lang, 'update.toast', { version: info.version }), body: translate(lang, 'update.toastBody') });
}

const FIRST_CHECK_MS = 8_000;
const EVERY_MS = 6 * 60 * 60 * 1000;

/** Procura atualização pouco depois de abrir e depois a cada 6 horas (o app costuma ficar na bandeja). */
export function useAutoUpdateCheck() {
  useEffect(() => {
    getVersion().then((current) => useUpdate.setState({ current })).catch(() => {});
    if (import.meta.env.DEV) return;
    const first = setTimeout(() => void checkForUpdate(), FIRST_CHECK_MS);
    const every = setInterval(() => void checkForUpdate(), EVERY_MS);
    return () => {
      clearTimeout(first);
      clearInterval(every);
    };
  }, []);
}
