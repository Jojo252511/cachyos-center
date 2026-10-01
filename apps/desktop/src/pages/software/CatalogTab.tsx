import { Search } from 'lucide-react';
import { useId, useState } from 'react';

import { api } from '../../api/client';
import type { CatalogInstallFilter } from '../../bindings/CatalogInstallFilter';
import type { PackageSummary } from '../../bindings/PackageSummary';
import { SelectField } from '../../components/Form';
import { EmptyState, ErrorState, LoadingState } from '../../components/StateViews';
import { useI18n } from '../../i18n';
import { usePackageActionGuard } from '../../state/guards';
import { useStatus } from '../../state/Status';
import { useDebounced } from '../../state/useDebounced';
import { useResource } from '../../state/useResource';
import { PackageRow } from './PackageRow';

const LIMIT = 100;

export function CatalogTab({
  selected,
  onSelect,
  onInstall,
}: {
  selected: string | null;
  onSelect(pkg: PackageSummary): void;
  onInstall(pkg: PackageSummary): void;
}) {
  const { t } = useI18n();
  const status = useStatus();
  const guard = usePackageActionGuard();
  const searchId = useId();
  const [query, setQuery] = useState('');
  const [repository, setRepository] = useState<string>('');
  const [installFilter, setInstallFilter] = useState<CatalogInstallFilter>('any');
  const debounced = useDebounced(query.trim(), 300);
  const repositories = useResource('repositories', () => api.listRepositories());
  const searchable = debounced.length >= 2 || repository !== '';
  const results = useResource(
    searchable ? `catalog:${repository}:${installFilter}:${debounced}` : null,
    () => api.searchPackages({ query: debounced, repository: repository === '' ? null : repository, installFilter, limit: LIMIT }),
    status.activityVersion,
  );

  return (
    <div className="list-pane">
      <p className="muted">{t('software.catalog.hint')}</p>
      <div className="toolbar toolbar--wrap">
        <div className="search-field">
          <label htmlFor={searchId} className="sr-only">
            {t('software.catalog.search')}
          </label>
          <Search className="search-field__icon" aria-hidden="true" />
          <input
            id={searchId}
            type="search"
            className="input"
            placeholder={t('software.catalog.searchPlaceholder')}
            value={query}
            maxLength={100}
            onChange={(event) => setQuery(event.target.value)}
          />
        </div>
        <SelectField
          label={t('software.catalog.repository')}
          value={repository}
          onChange={(value) => setRepository(value)}
          options={[
            { value: '', label: t('software.catalog.allRepositories') },
            ...(repositories.data ?? []).map((repo) => ({
              value: repo.name,
              label: t('software.catalog.repositoryOption', { name: repo.name, count: repo.packageCount }),
            })),
          ]}
        />
        <SelectField
          label={t('software.catalog.installFilter')}
          value={installFilter}
          onChange={(value) => setInstallFilter(value)}
          options={(['any', 'installed', 'notInstalled'] as const).map((value) => ({ value, label: t(`software.catalog.install.${value}`) }))}
        />
      </div>
      {repositories.error ? <ErrorState error={repositories.error} onRetry={repositories.reload} /> : null}
      {!searchable ? <EmptyState icon={<Search />} title={t('software.catalog.minChars')} /> : null}
      {searchable && results.error ? <ErrorState error={results.error} onRetry={results.reload} /> : null}
      {searchable && !results.data && !results.error ? <LoadingState /> : null}
      {searchable && results.data ? (
        results.data.length === 0 ? (
          <EmptyState title={t('software.empty')} />
        ) : (
          <>
            <p className="muted" aria-live="polite">
              {t('software.catalog.results', { count: results.data.length })}
              {results.data.length >= LIMIT ? ` ${t('software.catalog.limited', { count: LIMIT })}` : ''}
            </p>
            <ul className={`package-list ${results.loading ? 'is-loading' : ''}`} aria-busy={results.loading}>
              {results.data.map((pkg) => (
                <PackageRow
                  key={`${pkg.repository ?? 'local'}/${pkg.name}`}
                  pkg={pkg}
                  selected={selected === pkg.name}
                  onSelect={() => onSelect(pkg)}
                  onInstall={() => onInstall(pkg)}
                  installDisabled={guard.reason !== null}
                  showInstallState
                />
              ))}
            </ul>
          </>
        )
      ) : null}
    </div>
  );
}
