import { Bot } from 'lucide-react';

import { api } from '../../api/client';
import { Card } from '../../components/Card';
import { CopyButton } from '../../components/CopyButton';
import { Toggle } from '../../components/Form';
import { KeyValue, KeyValueList } from '../../components/KeyValue';
import { Notice } from '../../components/Notice';
import { ErrorState, LoadingState } from '../../components/StateViews';
import { useI18n, type MessageKey } from '../../i18n';
import { useResource } from '../../state/useResource';

const TOOL_KEYS: Record<string, MessageKey> = {
  system_get_summary: 'settings.mcp.tool.system_get_summary',
  updates_list: 'settings.mcp.tool.updates_list',
  packages_search: 'settings.mcp.tool.packages_search',
  packages_installed: 'settings.mcp.tool.packages_installed',
  operations_recent: 'settings.mcp.tool.operations_recent',
  health_get: 'settings.mcp.tool.health_get',
};

/** Local read-only MCP server: opt-in, data preview and host configuration to copy. */
export function McpSection({ enabled, onToggle }: { enabled: boolean; onToggle(enabled: boolean): void }) {
  const { t } = useI18n();
  const setup = useResource(`mcp:${enabled}`, () => api.getMcpSetup());
  return (
    <Card title={t('settings.mcp')} icon={<Bot />}>
      <p>{t('settings.mcp.intro')}</p>
      <Toggle label={t('settings.mcp.label')} hint={enabled ? undefined : t('settings.mcp.disabledNote')} checked={enabled} onCheckedChange={onToggle} />
      <p className="card__subtitle">{t('settings.mcp.dataTitle')}</p>
      <ul className="plain-list bullets">
        <li>{t('settings.mcp.data.summary')}</li>
        <li>{t('settings.mcp.data.updates')}</li>
        <li>{t('settings.mcp.data.packages')}</li>
        <li>{t('settings.mcp.data.operations')}</li>
        <li>{t('settings.mcp.data.health')}</li>
      </ul>
      <Notice tone="warning">{t('settings.mcp.privacy')}</Notice>
      {setup.error ? <ErrorState error={setup.error} onRetry={setup.reload} /> : null}
      {!setup.data && !setup.error ? <LoadingState /> : null}
      {setup.data ? (
        <>
          <KeyValueList>
            <KeyValue label={t('settings.mcp.tools')}>
              <ul className="plain-list">
                {setup.data.tools.map((tool) => (
                  <li key={tool}>
                    <code>{tool}</code>
                    {TOOL_KEYS[tool] ? <span className="muted"> – {t(TOOL_KEYS[tool])}</span> : null}
                  </li>
                ))}
              </ul>
            </KeyValue>
            <KeyValue label={t('settings.mcp.binary')}>
              {setup.data.binaryPath ? <code className="break">{setup.data.binaryPath}</code> : <span className="status-line status-line--warning">{t('settings.mcp.binaryMissing')}</span>}
            </KeyValue>
          </KeyValueList>
          <p className="card__subtitle">{t('settings.mcp.config')}</p>
          <pre className="mono-block" tabIndex={0} aria-label={t('settings.mcp.config')}>
            {setup.data.hostConfig}
          </pre>
          <CopyButton text={setup.data.hostConfig} label={t('settings.mcp.copyConfig')} doneLabel={t('settings.mcp.configCopied')} />
          <p className="field__hint">{t('settings.mcp.manual')}</p>
        </>
      ) : null}
    </Card>
  );
}
