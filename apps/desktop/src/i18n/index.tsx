import { createContext, useContext, useMemo, type ReactNode } from 'react';

import type { LanguagePreference } from '../bindings/LanguagePreference';
import { de, type MessageKey } from './de';
import { en } from './en';
import { createFormatters, type Formatters } from './format';
import type { Language, Message } from './types';

export type { Language, Message } from './types';
export type { MessageKey } from './de';

export type TranslateParams = Readonly<Record<string, string | number>>;
export type Translate = (key: MessageKey, params?: TranslateParams) => string;

export const dictionaries: Readonly<Record<Language, Readonly<Record<MessageKey, Message>>>> = { de, en };

const LOCALES: Record<Language, string> = { de: 'de-DE', en: 'en-US' };

export interface I18n {
  lang: Language;
  locale: string;
  t: Translate;
  fmt: Formatters;
}

export function createTranslate(lang: Language): Translate {
  const dictionary = dictionaries[lang];
  const locale = LOCALES[lang];
  const plural = new Intl.PluralRules(locale);
  const numbers = new Intl.NumberFormat(locale);

  return (key, params) => {
    const message = dictionary[key];
    let text: string;
    if (typeof message === 'string') {
      text = message;
    } else {
      const count = typeof params?.count === 'number' ? params.count : 0;
      text = plural.select(count) === 'one' ? message.one : message.other;
    }
    if (!params) return text;
    return text.replace(/\{(\w+)\}/g, (match, name: string) => {
      const value = params[name];
      if (value === undefined) return match;
      return typeof value === 'number' ? numbers.format(value) : value;
    });
  };
}

export function createI18n(lang: Language): I18n {
  const locale = LOCALES[lang];
  return { lang, locale, t: createTranslate(lang), fmt: createFormatters(locale) };
}

/** German is the default; `system` follows the language reported by the backend. */
export function resolveLanguage(preference: LanguagePreference, systemLanguage: string | null): Language {
  if (preference === 'de' || preference === 'en') return preference;
  return systemLanguage?.toLowerCase().startsWith('en') ? 'en' : 'de';
}

/** Best guess before the settings are loaded. */
export function initialLanguage(): Language {
  if (typeof navigator === 'undefined') return 'de';
  return navigator.language.toLowerCase().startsWith('en') ? 'en' : 'de';
}

const I18nContext = createContext<I18n>(createI18n('de'));

export function I18nProvider({ lang, children }: { lang: Language; children: ReactNode }) {
  const value = useMemo(() => createI18n(lang), [lang]);
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n(): I18n {
  return useContext(I18nContext);
}
