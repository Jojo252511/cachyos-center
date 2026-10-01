import { ChevronLeft, ChevronRight, Search } from 'lucide-react';
import { useId, useState } from 'react';

import { api } from '../../api/client';
import type { InstalledFilter } from '../../bindings/InstalledFilter';
import type { PackageSummary } from '../../bindings/PackageSummary';
import { Button } from '../../components/Button';
import { ChipGroup } from '../../components/Form';
import { EmptyState, ErrorState, LoadingState } from '../../components/StateViews';
import { useI18n } from '../../i18n';
import { useStatus } from '../../state/Status';
import { useDebounced } from '../../state/useDebounced';
import { useResource } from '../../state/useResource';
import { PackageRow } from './PackageRow';

const PAGE_SIZE = 50;
const FILTERS: readonly InstalledFilter[] = ['all', 'explicit', 'dependency', 'repo', 'localOrAur', 'updateAvailable'];

export function InstalledTab({ selected, onSelect }: { selected: string | null; onSelect(pkg: PackageSummary): void }) {
  const { t, fmt } = useI18n();
  const status = useStatus();
  const searchId = useId();
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState<InstalledFilter>('all');
  const [offset, setOffset] = useState(0);
  const debounced = useDebounced(query.trim(), 250);
  const page = useResource(
    `installed:${filter}:${offset}:${debounced}`,
    () => api.listInstalled({ query: debounced.length > 0 ? debounced : null, filter, offset, limit: PAGE_SIZE }),
    status.activityVersion,
  );
  const data = page.data;

  return (
    <div className="list-pane">
      <div className="toolbar">
        <div className="search-field">
          <label htmlFor={searchId} className="sr-only">
            {t('software.search')}
          </label>
          <Search className="search-field__icon" aria-hidden="true" />
          <input
            id={searchId}
            type="search"
            className="input"
            placeholder={t('software.searchInstalledPlaceholder')}
            value={query}
            maxLength={100}
            onChange={(event) => {
              setQuery(event.target.value);
              setOffset(0);
            }}
          />
        </div>
        <ChipGroup
          label={t('software.filter')}
          value={filter}
          options={FILTERS.map((value) => ({ value, label: t(`software.filter.${value}`) }))}
          onValueChange={(value) => {
            setFilter(value);
            setOffset(0);
          }}
        />
      </div>
      {page.error ? <ErrorState error={page.error} onRetry={page.reload} /> : null}
      {!data && !page.error ? <LoadingState /> : null}
      {data ? (
        data.items.length === 0 ? (
          <EmptyState title={debounced || filter !== 'all' ? t('software.emptyFiltered') : t('software.empty')} />
        ) : (
          <>
            <p className="muted" aria-live="polite">
              {t('software.range', { from: data.offset + 1, to: data.offset + data.items.length, total: data.total })}
            </p>
            <ul className={`package-list ${page.loading ? 'is-loading' : ''}`} aria-busy={page.loading}>
              {data.items.map((pkg) => (
                <PackageRow key={`${pkg.repository ?? 'local'}/${pkg.name}`} pkg={pkg} selected={selected === pkg.name} onSelect={() => onSelect(pkg)} />
              ))}
            </ul>
            {/* On the first or last page the button is disabled: the other one takes the focus. */}
            <nav className="pagination" aria-label={t('software.pagination')} data-focus-group>
              <Button size="sm" icon={<ChevronLeft />} disabled={data.offset === 0} onClick={() => setOffset(Math.max(0, data.offset - PAGE_SIZE))}>
                {t('software.prevPage')}
              </Button>
              <span className="muted">{fmt.number(Math.floor(data.offset / PAGE_SIZE) + 1)} / {fmt.number(Math.max(1, Math.ceil(data.total / PAGE_SIZE)))}</span>
              <Button size="sm" icon={<ChevronRight />} disabled={data.nextOffset === null} onClick={() => data.nextOffset !== null && setOffset(data.nextOffset)}>
                {t('software.nextPage')}
              </Button>
            </nav>
          </>
        )
      ) : null}
    </div>
  );
}
