import { Info } from 'lucide-react';

import { DOC_URLS } from '../../api/errors';
import { Badge } from '../../components/Badge';
import { Card } from '../../components/Card';
import { ExternalLink } from '../../components/ExternalLink';
import { KeyValue, KeyValueList } from '../../components/KeyValue';
import { useI18n } from '../../i18n';
import { useAppState } from '../../state/AppState';

export function AboutSection() {
  const { t } = useI18n();
  const { appInfo } = useAppState();
  return (
    <Card title={t('settings.about')} icon={<Info />}>
      <KeyValueList>
        <KeyValue label={t('settings.about.version')}>
          {appInfo.version}
          {appInfo.developmentMode ? (
            <>
              {' '}
              <Badge tone="neutral" title={t('settings.about.devModeHint')}>
                {t('settings.about.devMode')}
              </Badge>
              <span className="sr-only"> ({t('settings.about.devModeHint')})</span>
            </>
          ) : null}
        </KeyValue>
        <KeyValue label={t('settings.about.appId')}>
          <code>{appInfo.appId}</code>
        </KeyValue>
        <KeyValue label={t('settings.about.helper')}>
          {appInfo.helperAvailable ? t('settings.about.helperOk') : t('settings.about.helperMissing')}
          {appInfo.helperError ? <span className="muted block">{appInfo.helperError}</span> : null}
        </KeyValue>
        <KeyValue label={t('settings.about.backend')}>
          {appInfo.backend.state === 'ready'
            ? t('system.pacman.backendReady', { version: appInfo.backend.libalpmVersion, builtAgainst: appInfo.backend.builtAgainst })
            : t('system.pacman.backendUnavailable', { reason: appInfo.backend.reason })}
        </KeyValue>
      </KeyValueList>
      <p>
        <ExternalLink url={DOC_URLS.project}>{t('settings.about.repo')}</ExternalLink>
      </p>
      <p>{t('settings.about.community')}</p>
      <p className="muted">{t('settings.about.license')}</p>
    </Card>
  );
}
