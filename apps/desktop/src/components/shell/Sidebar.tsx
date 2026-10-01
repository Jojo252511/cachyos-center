import { CircleArrowUp, CircleCheck, Cpu, FileClock, LayoutDashboard, LoaderCircle, Menu, OctagonAlert, Package, Settings, X } from 'lucide-react';
import { useState, type KeyboardEvent, type ReactNode } from 'react';

import { useI18n, type MessageKey } from '../../i18n';
import { stateLabel } from '../operation/OperationView';
import { useOperations } from '../../state/Operations';
import { hrefFor, type PageId, type Route } from '../../state/router';
import { useStatus } from '../../state/Status';
import { Logo } from './Logo';

const NAV: readonly { page: PageId; label: MessageKey; icon: ReactNode }[] = [
  { page: 'overview', label: 'nav.overview', icon: <LayoutDashboard /> },
  { page: 'updates', label: 'nav.updates', icon: <CircleArrowUp /> },
  { page: 'software', label: 'nav.software', icon: <Package /> },
  { page: 'system', label: 'nav.system', icon: <Cpu /> },
  { page: 'activity', label: 'nav.activity', icon: <FileClock /> },
  { page: 'settings', label: 'nav.settings', icon: <Settings /> },
];

/**
 * Left navigation. Below ~900px it collapses into an icon rail; the toggle
 * expands it as an overlay with labels.
 */
export function Sidebar({ route }: { route: Route }) {
  const i18n = useI18n();
  const { t } = i18n;
  const status = useStatus();
  const operations = useOperations();
  const [expanded, setExpanded] = useState(false);
  const updates = status.updates;
  const updateCount = updates?.status === 'fresh' ? updates.updates.length : 0;
  const tracked = operations.tracked;

  const onKeyDown = (event: KeyboardEvent<HTMLElement>) => {
    if (event.key === 'Escape' && expanded) {
      setExpanded(false);
      document.getElementById('sidebar-toggle')?.focus();
    }
  };

  const showOperation = () => {
    operations.setExpanded(true);
    setExpanded(false);
    document.getElementById('operation-panel')?.scrollIntoView?.({ block: 'start' });
  };

  return (
    <nav className={`sidebar ${expanded ? 'sidebar--expanded' : ''}`} aria-label={t('shell.navigation')} onKeyDown={onKeyDown}>
      <div className="sidebar__brand">
        <button
          id="sidebar-toggle"
          type="button"
          className="icon-button sidebar__toggle"
          aria-expanded={expanded}
          aria-controls="sidebar-nav"
          aria-label={expanded ? t('shell.menuClose') : t('shell.menuOpen')}
          onClick={() => setExpanded((value) => !value)}
        >
          {expanded ? <X aria-hidden="true" /> : <Menu aria-hidden="true" />}
        </button>
        <Logo />
        <span className="sidebar__name">{t('app.name')}</span>
      </div>
      <ul className="sidebar__nav" id="sidebar-nav">
        {NAV.map((item) => {
          const label = t(item.label);
          return (
            <li key={item.page}>
              <a
                href={hrefFor(item.page)}
                className="nav-link"
                aria-current={route.page === item.page ? 'page' : undefined}
                title={label}
                onClick={() => setExpanded(false)}
              >
                <span className="nav-link__icon" aria-hidden="true">
                  {item.icon}
                </span>
                <span className="nav-link__label">{label}</span>
                {item.page === 'updates' && updateCount > 0 ? (
                  <>
                    <span className="nav-link__badge" aria-hidden="true">
                      {updateCount}
                    </span>
                    <span className="sr-only">, {t('nav.updatesBadge', { count: updateCount })}</span>
                  </>
                ) : null}
              </a>
            </li>
          );
        })}
      </ul>
      {tracked ? (
        <button type="button" className={`sidebar__operation ${operations.resultPending ? 'sidebar__operation--attention' : ''}`} onClick={showOperation}>
          <span className="sidebar__operation-icon" aria-hidden="true">
            {operations.active ? <LoaderCircle className="spin" /> : operations.resultPending ? <OctagonAlert /> : <CircleCheck />}
          </span>
          <span className="sidebar__operation-label">
            {operations.active && tracked.operation
              ? t('nav.operationRunning', { state: stateLabel(tracked.operation, i18n) })
              : operations.active
                ? t('operation.starting')
                : t('nav.operationResult')}
          </span>
        </button>
      ) : null}
      <p className="sidebar__footer">{t('app.tagline')}</p>
    </nav>
  );
}
