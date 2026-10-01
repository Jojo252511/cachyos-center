import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from 'react';

import { api, type BackendInfo } from '../api/client';
import { normalizeError } from '../api/errors';
import type { AppError } from '../bindings/AppError';
import type { AppInfo } from '../bindings/AppInfo';
import type { Settings } from '../bindings/Settings';
import type { ThemePreference } from '../bindings/ThemePreference';
import { I18nProvider, initialLanguage, resolveLanguage, type Language } from '../i18n';
import { BootScreen, StartupError } from '../components/BootScreen';

export type ResolvedTheme = 'dark' | 'light';

export interface AppStateValue {
  appInfo: AppInfo;
  settings: Settings;
  backend: BackendInfo;
  theme: ResolvedTheme;
  language: Language;
  /** Saves user preferences immediately; rejects with `AppError` and reverts on failure. */
  saveSettings(patch: Partial<Settings>): Promise<void>;
  refreshAppInfo(): Promise<void>;
}

const AppStateContext = createContext<AppStateValue | null>(null);

export function useAppState(): AppStateValue {
  const value = useContext(AppStateContext);
  if (!value) throw new Error('useAppState outside of AppStateProvider');
  return value;
}

/**
 * `system` follows an explicit desktop color scheme reported by the backend
 * (XDG portal). Without one, dark is the strong default: WebKitGTK reports
 * `prefers-color-scheme: light` whenever no preference exists, so the media
 * query is no reliable signal.
 */
export function resolveTheme(preference: ThemePreference, systemColorScheme: string): ResolvedTheme {
  if (preference === 'dark' || preference === 'light') return preference;
  if (systemColorScheme === 'dark' || systemColorScheme === 'light') return systemColorScheme;
  return 'dark';
}

interface Boot {
  appInfo: AppInfo;
  backend: BackendInfo;
}

/** Sequence of settings saves: only the response of the latest save is applied. */
let saveSequence = 0;

export function AppStateProvider({ children }: { children: ReactNode }) {
  const [boot, setBoot] = useState<Boot | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [bootError, setBootError] = useState<AppError | null>(null);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    let active = true;
    Promise.all([api.getAppInfo(), api.getSettings(), api.backendInfo()]).then(
      ([appInfo, loadedSettings, backend]) => {
        if (!active) return;
        setSettings(loadedSettings);
        setBoot({ appInfo, backend });
      },
      (error: unknown) => {
        if (active) setBootError(normalizeError(error));
      },
    );
    return () => {
      active = false;
    };
  }, [attempt]);

  const retry = useCallback(() => {
    setBootError(null);
    setAttempt((value) => value + 1);
  }, []);

  const saveSettings = useCallback(
    async (patch: Partial<Settings>) => {
      if (!settings) return;
      const previous = settings;
      const next = { ...settings, ...patch };
      const sequence = ++saveSequence;
      setSettings(next);
      try {
        const saved = await api.saveSettings(next);
        if (sequence === saveSequence) setSettings(saved);
      } catch (error) {
        if (sequence === saveSequence) setSettings(previous);
        throw normalizeError(error);
      }
    },
    [settings],
  );

  const refreshAppInfo = useCallback(async () => {
    const appInfo = await api.getAppInfo();
    setBoot((previous) => (previous ? { ...previous, appInfo } : previous));
  }, []);

  const theme = resolveTheme(settings?.theme ?? 'system', boot?.appInfo.systemColorScheme ?? 'unknown');
  const language = settings ? resolveLanguage(settings.language, boot?.appInfo.systemLanguage ?? null) : initialLanguage();
  const density = settings?.density ?? 'comfortable';

  useEffect(() => {
    const root = document.documentElement;
    root.dataset.theme = theme;
    root.dataset.density = density;
    root.lang = language;
  }, [theme, density, language]);

  const value = useMemo<AppStateValue | null>(
    () =>
      boot && settings
        ? { appInfo: boot.appInfo, backend: boot.backend, settings, theme, language, saveSettings, refreshAppInfo }
        : null,
    [boot, settings, theme, language, saveSettings, refreshAppInfo],
  );

  return (
    <I18nProvider lang={language}>
      {value ? (
        <AppStateContext.Provider value={value}>{children}</AppStateContext.Provider>
      ) : bootError ? (
        <StartupError error={bootError} onRetry={retry} />
      ) : (
        <BootScreen />
      )}
    </I18nProvider>
  );
}

