import { describe, expect, expectTypeOf, it } from 'vitest';

import { de, type MessageKey } from './de';
import { en } from './en';
import { createI18n, resolveLanguage } from './index';

describe('i18n dictionaries', () => {
  it('have identical keys at the type level', () => {
    expectTypeOf<keyof typeof en>().toEqualTypeOf<MessageKey>();
    expectTypeOf<keyof typeof de>().toEqualTypeOf<MessageKey>();
  });

  it('have identical keys at runtime', () => {
    expect(Object.keys(en).sort()).toEqual(Object.keys(de).sort());
  });

  it('use the same message shape (plain or plural) and the same placeholders', () => {
    const placeholders = (text: string) => [...text.matchAll(/\{(\w+)\}/g)].map((m) => m[1]).sort();
    for (const key of Object.keys(de) as MessageKey[]) {
      const g = de[key];
      const e = en[key];
      const gTexts = typeof g === 'string' ? [g] : [g.one, g.other];
      const eTexts = typeof e === 'string' ? [e] : [e.one, e.other];
      expect(typeof e === 'string', key).toBe(typeof g === 'string');
      const gNames = new Set(gTexts.flatMap(placeholders));
      const eNames = new Set(eTexts.flatMap(placeholders));
      expect([...eNames].sort(), key).toEqual([...gNames].sort());
      for (const text of [...gTexts, ...eTexts]) expect(text.trim().length, key).toBeGreaterThan(0);
    }
  });

  it('uses real German umlauts instead of ASCII replacements', () => {
    const ascii = /\b(fuer|ueber|Ueber|koennen|muessen|waehrend|Aenderung\w*|Pruefung|pruefen|geprueft|loeschen|schliessen|Schliessen|Groesse|Uebersicht|Aktivitaet|naechste\w*|zurueck|Neustart\w* ausloesen|veroeffentlicht|verfuegbar)\b/;
    for (const [key, message] of Object.entries(de)) {
      const texts = typeof message === 'string' ? [message] : [message.one, message.other];
      for (const text of texts) expect(ascii.test(text), `${key}: ${text}`).toBe(false);
    }
  });

  it('interpolates parameters and selects plural forms', () => {
    const { t } = createI18n('de');
    expect(t('dashboard.status.updates', { count: 1 })).toBe('1 Update verfügbar');
    expect(t('dashboard.status.updates', { count: 12 })).toBe('12 Updates verfügbar');
    expect(createI18n('en').t('dashboard.status.updates', { count: 1 })).toBe('1 update available');
    expect(t('operation.progress', { done: 3, total: 1200 })).toBe('3 von 1.200 Paketen');
  });

  it('resolves the language preference with German as default', () => {
    expect(resolveLanguage('system', 'en')).toBe('en');
    expect(resolveLanguage('system', 'de')).toBe('de');
    expect(resolveLanguage('system', null)).toBe('de');
    expect(resolveLanguage('system', 'fr')).toBe('de');
    expect(resolveLanguage('en', 'de')).toBe('en');
  });
});
