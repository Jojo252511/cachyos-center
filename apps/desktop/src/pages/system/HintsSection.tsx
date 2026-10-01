import { BookOpen } from 'lucide-react';

import { DOC_URLS } from '../../api/errors';
import type { ExternalUpdater } from '../../bindings/ExternalUpdater';
import { Badge } from '../../components/Badge';
import { Card } from '../../components/Card';
import { ExternalLink } from '../../components/ExternalLink';
import { useI18n } from '../../i18n';

/** Links to the official CachyOS/Arch documentation and the detected update services. */
export function HintsSection({ updaters }: { updaters: readonly ExternalUpdater[] | null }) {
  const { t } = useI18n();
  return (
    <Card title={t('hints.title')} icon={<BookOpen />} className="card--wide">
      <p className="muted">{t('hints.text')}</p>
      <ul className="link-list">
        <li>
          <ExternalLink url={DOC_URLS.cachyosPostInstall}>{t('hints.cachyosPostInstall')}</ExternalLink>
        </li>
        <li>
          <ExternalLink url={DOC_URLS.archSystemMaintenance}>{t('hints.archMaintenance')}</ExternalLink>
        </li>
        <li>
          <ExternalLink url={DOC_URLS.archNews}>{t('hints.archNews')}</ExternalLink>
        </li>
      </ul>
      <h3 className="card__subtitle">{t('health.updaters')}</h3>
      {updaters === null ? (
        <p className="muted">{t('common.unknown')}</p>
      ) : updaters.length === 0 ? (
        <p className="muted">{t('health.updaters.none')}</p>
      ) : (
        <ul className="updater-list">
          {updaters.map((updater) => (
            <li key={`${updater.scope}-${updater.name}`} className="updater-list__item">
              <code>{updater.name}</code>
              <Badge tone="neutral">{t(updater.scope === 'user' ? 'health.updaters.scope.user' : 'health.updaters.scope.system')}</Badge>
              <Badge tone={updater.active ? 'warning' : 'neutral'}>{updater.active ? t('common.active') : t('common.inactive')}</Badge>
              <Badge tone="info">{t('health.updaters.external')}</Badge>
              <span className="muted">{updater.description}</span>
            </li>
          ))}
        </ul>
      )}
    </Card>
  );
}
