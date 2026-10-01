import { Bell, FileClock, Newspaper, Palette, RefreshCw } from 'lucide-react';
import { useState } from 'react';

import { normalizeError } from '../api/errors';
import type { AppError } from '../bindings/AppError';
import type { Density } from '../bindings/Density';
import type { LanguagePreference } from '../bindings/LanguagePreference';
import type { Settings } from '../bindings/Settings';
import type { ThemePreference } from '../bindings/ThemePreference';
import { Card } from '../components/Card';
import { ErrorPanel } from '../components/ErrorPanel';
import { ChipGroup, SelectField, Toggle } from '../components/Form';
import { PageHeader } from '../components/PageHeader';
import { useI18n } from '../i18n';
import { useAppState } from '../state/AppState';
import { AboutSection } from './settings/AboutSection';
import { AutoUpdateSection } from './settings/AutoUpdateSection';
import { McpSection } from './settings/McpSection';

const CHECK_INTERVALS = [0, 1, 3, 6, 12, 24] as const;
const RETENTION_DAYS = [7, 30, 90, 180, 365] as const;

type SaveState = { kind: 'idle' } | { kind: 'saving' } | { kind: 'saved' } | { kind: 'error'; error: AppError };

function LabeledChips<T extends string>({ label, value, options, onChange }: { label: string; value: T; options: readonly { value: T; label: string }[]; onChange(value: T): void }) {
  return (
    <div className="field">
      <p className="field__label" aria-hidden="true">
        {label}
      </p>
      <ChipGroup label={label} value={value} options={options} onValueChange={onChange} />
    </div>
  );
}

export function SettingsPage() {
  const { t } = useI18n();
  const { settings, saveSettings } = useAppState();
  const [saveState, setSaveState] = useState<SaveState>({ kind: 'idle' });

  const save = async (patch: Partial<Settings>) => {
    setSaveState({ kind: 'saving' });
    try {
      await saveSettings(patch);
      setSaveState({ kind: 'saved' });
    } catch (error) {
      setSaveState({ kind: 'error', error: normalizeError(error) });
    }
  };

  return (
    <div className="page">
      <PageHeader title={t('settings.title')} subtitle={t('settings.subtitle')} />
      <div className="settings-status">
        <p className="muted">{t('settings.savedImmediately')}</p>
        <p role="status" className="settings-status__live">
          {saveState.kind === 'saving' ? t('common.saving') : saveState.kind === 'saved' ? t('common.saved') : ''}
        </p>
      </div>
      {saveState.kind === 'error' ? <ErrorPanel error={saveState.error} announce title={t('settings.saveFailed')} /> : null}

      <div className="settings-grid">
        <Card title={t('settings.appearance')} icon={<Palette />}>
          <LabeledChips<ThemePreference>
            label={t('settings.theme')}
            value={settings.theme}
            onChange={(theme) => void save({ theme })}
            options={(['system', 'dark', 'light'] as const).map((value) => ({ value, label: t(`settings.theme.${value}`) }))}
          />
          <LabeledChips<LanguagePreference>
            label={t('settings.language')}
            value={settings.language}
            onChange={(language) => void save({ language })}
            options={(['system', 'de', 'en'] as const).map((value) => ({ value, label: t(`settings.language.${value}`) }))}
          />
          <LabeledChips<Density>
            label={t('settings.density')}
            value={settings.density}
            onChange={(density) => void save({ density })}
            options={(['comfortable', 'compact'] as const).map((value) => ({ value, label: t(`settings.density.${value}`) }))}
          />
        </Card>

        <Card title={t('settings.notifications')} icon={<Bell />}>
          <Toggle
            label={t('settings.notifications.label')}
            hint={t('settings.notifications.hint')}
            checked={settings.notifications}
            onCheckedChange={(notifications) => void save({ notifications })}
          />
        </Card>

        <Card title={t('settings.check')} icon={<RefreshCw />}>
          <SelectField
            label={t('settings.check.interval')}
            hint={t('settings.check.hint')}
            value={settings.checkIntervalHours}
            onChange={(checkIntervalHours) => void save({ checkIntervalHours })}
            options={CHECK_INTERVALS.map((hours) => ({ value: hours, label: hours === 0 ? t('settings.check.off') : t('settings.check.hours', { count: hours }) }))}
          />
        </Card>

        <AutoUpdateSection />

        <Card title={t('settings.retention')} icon={<FileClock />}>
          <SelectField
            label={t('settings.retention.label')}
            value={settings.logRetentionDays}
            onChange={(logRetentionDays) => void save({ logRetentionDays })}
            options={RETENTION_DAYS.map((days) => ({ value: days, label: t('settings.retention.days', { count: days }) }))}
          />
        </Card>

        <Card title={t('settings.news')} icon={<Newspaper />}>
          <Toggle label={t('settings.news.label')} hint={t('settings.news.hint')} checked={settings.newsEnabled} onCheckedChange={(newsEnabled) => void save({ newsEnabled })} />
        </Card>

        <McpSection enabled={settings.mcpEnabled} onToggle={(mcpEnabled) => void save({ mcpEnabled })} />

        <AboutSection />
      </div>
    </div>
  );
}
