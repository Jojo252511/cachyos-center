import { useEffect, useRef } from 'react';

import { useI18n } from '../../i18n';
import { watchFocusLoss } from '../../lib/focus';
import { ActivityPage } from '../../pages/ActivityPage';
import { DashboardPage } from '../../pages/DashboardPage';
import { SettingsPage } from '../../pages/SettingsPage';
import { SoftwarePage } from '../../pages/SoftwarePage';
import { SystemPage } from '../../pages/SystemPage';
import { UpdatesPage } from '../../pages/UpdatesPage';
import { useRoute, type PageId } from '../../state/router';
import { OperationPanel } from '../operation/OperationPanel';
import { DemoBanner } from './DemoBanner';
import { Sidebar } from './Sidebar';

function Page({ page }: { page: PageId }) {
  switch (page) {
    case 'updates':
      return <UpdatesPage />;
    case 'software':
      return <SoftwarePage />;
    case 'system':
      return <SystemPage />;
    case 'activity':
      return <ActivityPage />;
    case 'settings':
      return <SettingsPage />;
    default:
      return <DashboardPage />;
  }
}

export function AppShell() {
  const { t } = useI18n();
  const route = useRoute();
  const appRef = useRef<HTMLDivElement>(null);
  const mainRef = useRef<HTMLElement>(null);
  const previousPage = useRef(route.page);

  // A re-render that removes or disables the focused element must not drop the focus to <body>.
  useEffect(() => (appRef.current ? watchFocusLoss(appRef.current) : undefined), []);

  // Move focus to the page title after navigation (screen readers announce the new page).
  useEffect(() => {
    if (previousPage.current === route.page) return;
    previousPage.current = route.page;
    mainRef.current?.scrollTo?.({ top: 0 });
    document.getElementById('page-title')?.focus({ preventScroll: true });
  }, [route.page]);

  // Anchors inside a page, e.g. `#/system/health`.
  useEffect(() => {
    if (route.section) document.getElementById(route.section)?.scrollIntoView?.({ block: 'start' });
  }, [route.page, route.section]);

  return (
    <div className="app" ref={appRef}>
      <a
        className="skip-link"
        href="#main-content"
        onClick={(event) => {
          event.preventDefault();
          mainRef.current?.focus();
        }}
      >
        {t('shell.skipToContent')}
      </a>
      <DemoBanner />
      <div className="app__body">
        <Sidebar route={route} />
        <main id="main-content" ref={mainRef} tabIndex={-1} className="main">
          <div className="main__inner">
            <OperationPanel />
            <Page page={route.page} />
          </div>
        </main>
      </div>
    </div>
  );
}
