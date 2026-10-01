import { ChevronRight, Download } from 'lucide-react';

import type { PackageSummary } from '../../bindings/PackageSummary';
import { Badge } from '../../components/Badge';
import { Button } from '../../components/Button';
import { useI18n } from '../../i18n';

export interface PackageRowProps {
  pkg: PackageSummary;
  selected: boolean;
  onSelect(): void;
  /** Catalog only: install button for packages that are not installed. */
  onInstall?: () => void;
  installDisabled?: boolean;
  showInstallState?: boolean;
}

/** One package in a list: name, version, origin and state badges; opens the details. */
export function PackageRow({ pkg, selected, onSelect, onInstall, installDisabled, showInstallState }: PackageRowProps) {
  const { t } = useI18n();
  const version = pkg.installedVersion ?? pkg.availableVersion ?? '';
  return (
    // The install button disappears once the package is installed: the row button takes the focus.
    <li className={`package-row ${selected ? 'package-row--selected' : ''}`} data-focus-group>
      <button type="button" className="package-row__main" onClick={onSelect} aria-pressed={selected} aria-label={t('software.showDetails', { name: pkg.name })}>
        <span className="package-row__head">
          <span className="package-row__name">{pkg.name}</span>
          <span className="package-row__version mono">{version}</span>
        </span>
        <span className="package-row__description">{pkg.description}</span>
        <span className="badge-list">
          {pkg.origin === 'localOrAur' ? (
            <Badge tone="warning">{t('software.origin.localOrAurShort')}</Badge>
          ) : pkg.origin === 'flatpak' ? (
            <Badge tone="info">{t('software.origin.flatpak')}</Badge>
          ) : pkg.repository ? (
            <Badge tone="neutral">{pkg.repository}</Badge>
          ) : null}
          {pkg.installReason ? <Badge tone="neutral">{t(`software.reason.${pkg.installReason}`)}</Badge> : null}
          {showInstallState && pkg.installedVersion !== null ? <Badge tone="success">{t('software.badge.installed')}</Badge> : null}
          {pkg.updateAvailable ? <Badge tone="accent">{t('software.badge.update')}</Badge> : null}
          {pkg.ignored ? <Badge tone="warning">{t('software.badge.ignored')}</Badge> : null}
        </span>
        <ChevronRight className="package-row__chevron" aria-hidden="true" />
      </button>
      {onInstall && pkg.installedVersion === null ? (
        <div className="package-row__actions">
          <Button size="sm" icon={<Download />} onClick={onInstall} disabled={installDisabled}>
            {t('software.install')}
          </Button>
        </div>
      ) : null}
    </li>
  );
}
