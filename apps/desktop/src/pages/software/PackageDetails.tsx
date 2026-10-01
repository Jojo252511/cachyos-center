import { Download, Trash, X } from 'lucide-react';
import { useEffect, useRef } from 'react';

import { api } from '../../api/client';
import type { PackageRef } from '../../bindings/PackageRef';
import type { PackageRecord } from '../../bindings/PackageRecord';
import { Badge } from '../../components/Badge';
import { Button } from '../../components/Button';
import { CopyButton } from '../../components/CopyButton';
import { ExternalLink } from '../../components/ExternalLink';
import { KeyValue, KeyValueList } from '../../components/KeyValue';
import { Notice } from '../../components/Notice';
import { ErrorState, LoadingState } from '../../components/StateViews';
import { GuardNotice } from '../../components/StatusNotices';
import { TerminalCommand } from '../../components/TerminalCommand';
import { useI18n } from '../../i18n';
import { isOpenableUrl } from '../../lib/links';
import { usePackageActionGuard } from '../../state/guards';
import { useStatus } from '../../state/Status';
import { useResource } from '../../state/useResource';

function NameList({ names }: { names: readonly string[] }) {
  const { t } = useI18n();
  if (names.length === 0) return <span className="muted">{t('common.none')}</span>;
  return (
    <ul className="package-chips">
      {names.map((name) => (
        <li key={name} className="mono">
          {name}
        </li>
      ))}
    </ul>
  );
}

export interface PackageDetailsProps {
  reference: PackageRef;
  onClose(): void;
  onInstall(record: PackageRecord): void;
  onRemove(record: PackageRecord): void;
}

/** Details of a package (`get_package_details`) with the available actions. */
export function PackageDetails({ reference, onClose, onInstall, onRemove }: PackageDetailsProps) {
  const { t, fmt } = useI18n();
  const status = useStatus();
  const guard = usePackageActionGuard();
  const details = useResource(`details:${reference.repository ?? 'installed'}/${reference.name}`, () => api.getPackageDetails(reference), status.activityVersion);
  const record = details.data;
  const panelRef = useRef<HTMLElement>(null);
  const titleRef = useRef<HTMLHeadingElement>(null);

  // On narrow windows the panel is placed above the list: bring it into view and focus it.
  useEffect(() => {
    panelRef.current?.scrollIntoView?.({ block: 'nearest' });
    titleRef.current?.focus({ preventScroll: true });
  }, []);

  return (
    <aside className="details-panel" aria-labelledby="package-details-title" ref={panelRef}>
      <header className="details-panel__header">
        <h2 className="details-panel__title" id="package-details-title" tabIndex={-1} ref={titleRef}>
          {reference.name}
        </h2>
        <button type="button" className="icon-button" aria-label={t('software.details.close')} onClick={onClose}>
          <X aria-hidden="true" />
        </button>
      </header>
      {details.error ? <ErrorState error={details.error} onRetry={details.reload} /> : null}
      {!record && !details.error ? <LoadingState /> : null}
      {record ? (
        <div className="details-panel__body">
          <p>{record.description}</p>
          <span className="badge-list">
            {record.origin === 'localOrAur' ? (
              <Badge tone="warning">{t('software.origin.localOrAur')}</Badge>
            ) : record.origin === 'flatpak' ? (
              <Badge tone="info">{t('software.origin.flatpak')}</Badge>
            ) : (
              <Badge tone="neutral">{record.id.repository}</Badge>
            )}
            {record.critical ? <Badge tone="danger">{t('software.badge.critical')}</Badge> : null}
            {record.ignored ? <Badge tone="warning">{t('software.badge.ignored')}</Badge> : null}
          </span>

          {record.origin === 'flatpak' ? null : record.origin === 'localOrAur' ? (
            <Notice tone="warning" title={t('software.details.foreignTitle')}>
              <p>{t('software.details.foreignText')}</p>
              <TerminalCommand command={`sudo pacman -Rs ${record.id.name}`} />
            </Notice>
          ) : (
            // Disabled while another operation runs: the title of the details takes the focus.
            <div className="details-panel__actions" data-focus-fallback="package-details-title">
              {record.installedVersion === null ? (
                <Button variant="primary" icon={<Download />} disabled={guard.reason !== null} onClick={() => onInstall(record)}>
                  {t('software.install')}
                </Button>
              ) : (
                <Button variant="danger" icon={<Trash />} disabled={guard.reason !== null} onClick={() => onRemove(record)}>
                  {t('software.remove')}
                </Button>
              )}
              <GuardNotice message={guard.message} />
            </div>
          )}
          {record.critical ? <p className="status-line status-line--danger">{t('software.details.criticalHint')}</p> : null}
          {record.ignored ? <p className="muted">{t('software.details.ignoredHint')}</p> : null}

          <KeyValueList>
            <KeyValue label={t('software.details.installedVersion')}>
              <span className="mono">{record.installedVersion ?? t('software.details.notInstalled')}</span>
            </KeyValue>
            {record.availableVersion ? (
              <KeyValue label={t('software.details.availableVersion')}>
                <span className="mono">{record.availableVersion}</span>
              </KeyValue>
            ) : null}
            <KeyValue label={t('software.details.source')}>
              {record.origin === 'localOrAur' ? t('software.origin.localOrAur') : `${t('software.origin.repo')}: ${record.id.repository}`}
            </KeyValue>
            <KeyValue label={t('software.details.architecture')}>{record.id.architecture}</KeyValue>
            {record.installedSize !== null ? <KeyValue label={t('software.details.installedSize')}>{fmt.bytes(record.installedSize)}</KeyValue> : null}
            {record.downloadSize !== null ? <KeyValue label={t('software.details.downloadSize')}>{fmt.bytes(record.downloadSize)}</KeyValue> : null}
            {record.installReason ? <KeyValue label={t('software.details.reason')}>{t(`software.reason.${record.installReason}`)}</KeyValue> : null}
            {record.url ? (
              <KeyValue label={t('software.details.homepage')}>
                {isOpenableUrl(record.url) ? (
                  <ExternalLink url={record.url}>{record.url}</ExternalLink>
                ) : (
                  <>
                    <code className="break">{record.url}</code> <CopyButton text={record.url} />
                    <span className="field__hint block">{t('software.details.homepageHint')}</span>
                  </>
                )}
              </KeyValue>
            ) : null}
            <KeyValue label={t('software.details.licenses')}>{record.licenses.length > 0 ? record.licenses.join(', ') : t('common.unknown')}</KeyValue>
            {record.packager ? <KeyValue label={t('software.details.packager')}>{record.packager}</KeyValue> : null}
            {record.buildDate !== null ? <KeyValue label={t('software.details.buildDate')}>{fmt.dateTime(record.buildDate)}</KeyValue> : null}
            {record.installDate !== null ? <KeyValue label={t('software.details.installDate')}>{fmt.dateTime(record.installDate)}</KeyValue> : null}
            {record.groups.length > 0 ? <KeyValue label={t('software.details.groups')}>{record.groups.join(', ')}</KeyValue> : null}
            <KeyValue label={t('software.details.dependencies')}>
              <NameList names={record.dependencies} />
            </KeyValue>
            {record.installedVersion !== null ? (
              <KeyValue label={t('software.details.requiredBy')}>
                <NameList names={record.requiredBy} />
              </KeyValue>
            ) : null}
            <KeyValue label={t('software.details.optionalDependencies')}>
              <NameList names={record.optionalDependencies} />
            </KeyValue>
            {record.optionalFor.length > 0 ? (
              <KeyValue label={t('software.details.optionalFor')}>
                <NameList names={record.optionalFor} />
              </KeyValue>
            ) : null}
            {record.provides.length > 0 ? (
              <KeyValue label={t('software.details.provides')}>
                <NameList names={record.provides} />
              </KeyValue>
            ) : null}
            {record.conflicts.length > 0 ? (
              <KeyValue label={t('software.details.conflicts')}>
                <NameList names={record.conflicts} />
              </KeyValue>
            ) : null}
            {record.replaces.length > 0 ? (
              <KeyValue label={t('software.details.replaces')}>
                <NameList names={record.replaces} />
              </KeyValue>
            ) : null}
          </KeyValueList>
        </div>
      ) : null}
    </aside>
  );
}
