import { describe, expect, it } from 'vitest';
import { LOCALES, diagnosticText, translate } from './i18n';
import { ptBR } from './locales/pt-BR';

describe('i18n', () => {
  it('todos os idiomas têm as mesmas chaves, sem textos vazios', () => {
    const base = Object.keys(ptBR).sort();
    for (const [lang, msgs] of Object.entries(LOCALES)) {
      expect(Object.keys(msgs).sort(), lang).toEqual(base);
      for (const [k, v] of Object.entries(msgs)) expect(v.trim(), `${lang}:${k}`).not.toBe('');
    }
  });

  it('interpola variáveis e mantém marcadores desconhecidos', () => {
    expect(translate('pt-BR', 'nav.phase', { n: 3 })).toBe('Fase 3');
    expect(translate('en', 'status.bar.tasks', { active: 2, queued: 1 })).toBe('2 downloading · 1 queued');
    expect(translate('es', 'nav.phase')).toBe('Fase {n}');
  });

  it('diagnóstico traduzido por tipo; pt-BR e mensagens específicas usam o texto do núcleo', () => {
    const d = { kind: 'expired', title: 'O link expirou (410)', action: 'Gere um link novo', detail: '', retryable: false } as const;
    expect(diagnosticText(d, 'pt-BR').title).toBe('O link expirou (410)');
    expect(diagnosticText(d, 'en').title).toBe('The link expired (410)');
    const custom = { ...d, kind: 'unknown', title: 'Pause antes de remover' } as const;
    expect(diagnosticText(custom, 'en').title).toBe('Pause antes de remover');
  });
});
