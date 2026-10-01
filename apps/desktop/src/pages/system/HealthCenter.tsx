import { Camera, FolderOpen, HardDrive, HeartPulse, RotateCw, ShieldCheck } from 'lucide-react';
import { useState } from 'react';

import { api } from '../../api/client';
import { normalizeError } from '../../api/errors';
import type { AppError } from '../../bindings/AppError';
import type { HealthReport } from '../../bindings/HealthReport';
import { Badge, SeverityBadge } from '../../components/Badge';
import { Button } from '../../components/Button';
import { Card } from '../../components/Card';
import { ErrorPanel } from '../../components/ErrorPanel';
import { KeyValue, KeyValueList } from '../../components/KeyValue';
import { Notice } from '../../components/Notice';
import { TerminalCommand } from '../../components/TerminalCommand';
import { TimeText } from '../../components/TimeText';
import { useI18n } from '../../i18n';

function YesNo({ value }: { value: boolean }) {
  const { t } = useI18n();
  return <Badge tone={value ? 'success' : 'neutral'}>{value ? t('common.yes') : t('common.no')}</Badge>;
}

/** Health center: findings, configuration files, reboot, cache, snapshots and offline updates. */
export function HealthCenter({ report }: { report: HealthReport }) {
  const { t, fmt } = useI18n();
  const [revealError, setRevealError] = useState<AppError | null>(null);

  const reveal = async (path: string) => {
    setRevealError(null);
    try {
      await api.revealConfigFile(path);
    } catch (error) {
      setRevealError(normalizeError(error));
    }
  };

  return (
    <>
      <Card title={t('health.title')} icon={<HeartPulse />} className="card--wide" id="health">
        <p className="muted">
          {t('health.collected')} <TimeText seconds={report.collectedAt} />
        </p>
        {report.items.length === 0 ? (
          <Notice tone="success">{t('health.none')}</Notice>
        ) : (
          <ul className="finding-list">
            {report.items.map((item) => (
              <li key={`${item.kind}-${item.detail}`} className="finding">
                <SeverityBadge severity={item.severity} />
                <div className="finding__text">
                  <p className="finding__title">
                    {t(`health.kind.${item.kind}`)}
                    {item.count !== null ? <span className="muted"> · {t('health.count', { count: item.count })}</span> : null}
                  </p>
                  <p>{t(`health.text.${item.kind}`)}</p>
                  {item.detail ? <p className="finding__detail mono">{item.detail}</p> : null}
                </div>
              </li>
            ))}
          </ul>
        )}
      </Card>

      <Card title={t('health.configFiles')} icon={<FolderOpen />} className="card--wide">
        <p className="muted">{t('health.configFiles.hint')}</p>
        {report.configFiles.length === 0 ? (
          <p>{t('health.configFiles.none')}</p>
        ) : (
          <ul className="file-list">
            {report.configFiles.map((file) => (
              <li key={file.path} className="file-list__item">
                <code className="break">{file.path}</code>
                <Badge tone={file.kind === 'pacnew' ? 'warning' : 'neutral'}>.{file.kind}</Badge>
                {file.modifiedAt !== null ? <span className="muted">{t('health.configFiles.modified', { time: fmt.dateTime(file.modifiedAt) })}</span> : null}
                <Button size="sm" icon={<FolderOpen />} aria-label={t('health.configFiles.revealLabel', { path: file.path })} onClick={() => void reveal(file.path)}>
                  {t('health.configFiles.reveal')}
                </Button>
              </li>
            ))}
          </ul>
        )}
        {revealError ? <ErrorPanel error={revealError} announce /> : null}
      </Card>

      <Card title={t('health.reboot')} icon={<RotateCw />}>
        <p>{report.rebootRecommended ? t('health.reboot.recommended') : t('health.reboot.notNeeded')}</p>
        {report.rebootReasons.length > 0 ? (
          <>
            <p className="card__subtitle">{t('health.reboot.reasons')}</p>
            <ul className="plain-list mono">
              {report.rebootReasons.map((reason) => (
                <li key={reason}>{reason}</li>
              ))}
            </ul>
          </>
        ) : null}
      </Card>

      <Card title={t('health.cache')} icon={<HardDrive />}>
        <p>{report.packageCacheBytes !== null ? t('health.cache.size', { size: fmt.bytes(report.packageCacheBytes) }) : t('health.cache.unknown')}</p>
        <TerminalCommand command="sudo paccache -r" hint={t('health.cache.hint')} />
      </Card>

      <Card title={t('health.snapshot')} icon={<Camera />}>
        <KeyValueList className="kv--compact">
          <KeyValue label={t('health.snapshot.btrfs')}>
            <YesNo value={report.snapshot.btrfsRoot} />
          </KeyValue>
          <KeyValue label={t('health.snapshot.snapper')}>
            <YesNo value={report.snapshot.snapperInstalled} />
          </KeyValue>
          <KeyValue label={t('health.snapshot.rootConfig')}>{report.snapshot.rootConfig ?? t('common.none')}</KeyValue>
          <KeyValue label={t('health.snapshot.snapPac')}>
            <YesNo value={report.snapshot.snapPacActive} />
          </KeyValue>
          <KeyValue label={t('health.snapshot.request')}>
            <YesNo value={report.snapshot.canRequestSnapshot} />
          </KeyValue>
        </KeyValueList>
        <p className="muted">{t('health.snapshot.limit')}</p>
      </Card>

      <Card title={t('health.offline')} icon={<ShieldCheck />}>
        <KeyValueList className="kv--compact">
          <KeyValue label={t('health.offline.installed')}>
            <YesNo value={report.offlineUpdate.installed} />
          </KeyValue>
          <KeyValue label={t('health.offline.prepared')}>
            <YesNo value={report.offlineUpdate.prepared} />
          </KeyValue>
          <KeyValue label={t('health.offline.prepareTimer')}>
            <YesNo value={report.offlineUpdate.prepareTimerActive} />
          </KeyValue>
          <KeyValue label={t('health.offline.rebootTimer')}>
            <YesNo value={report.offlineUpdate.rebootTimerActive} />
          </KeyValue>
          <KeyValue label={t('health.offline.confIncluded')}>
            <YesNo value={report.offlineUpdate.offlineConfIncluded} />
          </KeyValue>
          <KeyValue label={t('health.offline.verifiable')}>
            <YesNo value={report.offlineUpdate.configurationVerifiable} />
          </KeyValue>
        </KeyValueList>
        {report.offlineUpdate.offlineConfIgnored.length > 0 ? (
          <p>{t('health.offline.held', { packages: report.offlineUpdate.offlineConfIgnored.join(', ') })}</p>
        ) : null}
        <p className="card__subtitle">{t('health.blockers')}</p>
        {report.updateBlockers.length > 0 ? (
          <ul className="plain-list mono">
            {report.updateBlockers.map((blocker) => (
              <li key={blocker}>{blocker}</li>
            ))}
          </ul>
        ) : (
          <p className="muted">{t('health.blockers.none')}</p>
        )}
      </Card>
    </>
  );
}
