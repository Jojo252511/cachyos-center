import type { PlanEntry } from '../../bindings/PlanEntry';
import { useI18n } from '../../i18n';
import { Badge } from '../Badge';
import { FlagBadges } from './FlagBadges';

export function versionText(entry: Pick<PlanEntry, 'oldVersion' | 'newVersion'>): string {
  if (entry.oldVersion && entry.newVersion && entry.oldVersion !== entry.newVersion) return `${entry.oldVersion} → ${entry.newVersion}`;
  return entry.newVersion ?? entry.oldVersion ?? '–';
}

/** "old → new" that may wrap at the arrow but never inside a version. */
export function VersionChange({ from, to }: { from: string | null; to: string | null }) {
  if (from && to && from !== to) {
    return (
      <>
        <span className="version-change__part">{from}</span> <span className="version-change__part">→ {to}</span>
      </>
    );
  }
  return <span className="version-change__part">{to ?? from ?? '–'}</span>;
}

/** Table of plan entries (package, action, repository, version, download, flags). */
export function PlanTable({ entries, caption }: { entries: readonly PlanEntry[]; caption?: string }) {
  const { t, fmt } = useI18n();
  return (
    <div className="table-wrap">
      <table className="table">
        {caption ? <caption className="sr-only">{caption}</caption> : null}
        <thead>
          <tr>
            <th scope="col">{t('plan.column.package')}</th>
            <th scope="col">{t('plan.column.action')}</th>
            <th scope="col">{t('plan.column.repository')}</th>
            <th scope="col">{t('plan.column.version')}</th>
            <th scope="col" className="num">
              {t('plan.column.download')}
            </th>
            <th scope="col">{t('plan.column.notes')}</th>
          </tr>
        </thead>
        <tbody>
          {entries.map((entry) => (
            <tr key={`${entry.action}-${entry.name}`}>
              <th scope="row" className="table__name">
                {entry.name}
                {entry.requested ? (
                  <>
                    {' '}
                    <Badge tone="accent">{t('plan.requested')}</Badge>
                  </>
                ) : null}
              </th>
              <td>{t(`plan.action.${entry.action}`)}</td>
              <td>{entry.repository ?? '–'}</td>
              <td className="mono version-cell">
                <VersionChange from={entry.oldVersion} to={entry.newVersion} />
              </td>
              <td className="num">
                {entry.action === 'remove'
                  ? '–'
                  : entry.downloadSize === null
                    ? t('plan.downloadUnknown')
                    : entry.downloadSize === 0
                      ? t('plan.cached')
                      : fmt.bytes(entry.downloadSize)}
              </td>
              <td>
                <FlagBadges flags={entry.flags} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
