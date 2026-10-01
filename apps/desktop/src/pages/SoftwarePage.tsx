import { Tabs } from 'radix-ui';
import { useState } from 'react';

import type { PackageRef } from '../bindings/PackageRef';
import type { PackageSummary } from '../bindings/PackageSummary';
import { InstallDialog, type InstallTarget } from '../components/dialogs/InstallDialog';
import { RemoveDialog } from '../components/dialogs/RemoveDialog';
import { PageHeader } from '../components/PageHeader';
import { LockNotice, PlatformNotices } from '../components/StatusNotices';
import { useI18n } from '../i18n';
import { useAppState } from '../state/AppState';
import { useStatus } from '../state/Status';
import { CatalogTab } from './software/CatalogTab';
import { InstalledTab } from './software/InstalledTab';
import { PackageDetails } from './software/PackageDetails';

type Tab = 'installed' | 'catalog';

export function SoftwarePage() {
  const { t } = useI18n();
  const { appInfo } = useAppState();
  const status = useStatus();
  const [tab, setTab] = useState<Tab>('installed');
  const [selected, setSelected] = useState<PackageRef | null>(null);
  const [installTarget, setInstallTarget] = useState<InstallTarget | null>(null);
  const [removeTarget, setRemoveTarget] = useState<{ name: string; critical: boolean } | null>(null);
  const backendReady = appInfo.backend.state === 'ready';

  const select = (pkg: PackageSummary, fromCatalog: boolean) =>
    setSelected({ name: pkg.name, repository: fromCatalog || pkg.installedVersion === null ? pkg.repository : null });

  return (
    <div className="page">
      <PageHeader title={t('software.title')} subtitle={t('software.subtitle')} />
      <PlatformNotices />
      {status.dashboard ? <LockNotice lock={status.dashboard.lock} /> : null}
      {backendReady ? (
        <div className={`software-layout ${selected ? 'software-layout--with-details' : ''}`}>
          <Tabs.Root
            className="tabs"
            value={tab}
            onValueChange={(value) => {
              setTab(value as Tab);
              setSelected(null);
            }}
          >
            <Tabs.List className="tabs__list" aria-label={t('software.tabs')}>
              <Tabs.Trigger className="tabs__trigger" value="installed">
                {t('software.tab.installed')}
              </Tabs.Trigger>
              <Tabs.Trigger className="tabs__trigger" value="catalog">
                {t('software.tab.catalog')}
              </Tabs.Trigger>
            </Tabs.List>
            <Tabs.Content className="tabs__content" value="installed">
              <InstalledTab selected={selected?.name ?? null} onSelect={(pkg) => select(pkg, false)} />
            </Tabs.Content>
            <Tabs.Content className="tabs__content" value="catalog">
              <CatalogTab
                selected={selected?.name ?? null}
                onSelect={(pkg) => select(pkg, true)}
                onInstall={(pkg) => pkg.repository && setInstallTarget({ repository: pkg.repository, name: pkg.name })}
              />
            </Tabs.Content>
          </Tabs.Root>
          {selected ? (
            <PackageDetails
              key={`${selected.repository ?? ''}/${selected.name}`}
              reference={selected}
              onClose={() => setSelected(null)}
              onInstall={(record) => setInstallTarget({ repository: record.id.repository, name: record.id.name })}
              onRemove={(record) => setRemoveTarget({ name: record.id.name, critical: record.critical })}
            />
          ) : null}
        </div>
      ) : null}
      {installTarget ? <InstallDialog target={installTarget} onClose={() => setInstallTarget(null)} /> : null}
      {removeTarget ? <RemoveDialog name={removeTarget.name} critical={removeTarget.critical} onClose={() => setRemoveTarget(null)} /> : null}
    </div>
  );
}
