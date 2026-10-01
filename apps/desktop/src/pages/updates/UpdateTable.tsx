import type { UpdateCandidate } from '../../bindings/UpdateCandidate';
import { FlagBadges } from '../../components/plan/FlagBadges';
import { VersionChange } from '../../components/plan/PlanTable';
import { useI18n } from '../../i18n';

/** Explanatory list of updates (no selection: the upgrade is always complete). */
export function UpdateTable({ updates, caption }: { updates: readonly UpdateCandidate[]; caption: string }) {
  const { t, fmt } = useI18n();
  return (
    <div className="table-wrap">
      <table className="table">
        <caption className="sr-only">{caption}</caption>
        <thead>
          <tr>
            <th scope="col">{t('plan.column.package')}</th>
            <th scope="col">{t('plan.column.repository')}</th>
            <th scope="col">{t('plan.column.version')}</th>
            <th scope="col" className="num">
              {t('plan.column.download')}
            </th>
            <th scope="col">{t('plan.column.notes')}</th>
          </tr>
        </thead>
        <tbody>
          {updates.map((update) => (
            <tr key={`${update.packageId.repository}/${update.packageId.name}`}>
              <th scope="row" className="table__name">
                {update.packageId.name}
              </th>
              <td>{update.packageId.repository}</td>
              <td className="mono version-cell">
                <VersionChange from={update.oldVersion} to={update.newVersion} />
              </td>
              <td className="num">
                {update.downloadSize === null ? t('plan.downloadUnknown') : update.downloadSize === 0 ? t('plan.cached') : fmt.bytes(update.downloadSize)}
              </td>
              <td>
                <FlagBadges flags={update.flags} />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
