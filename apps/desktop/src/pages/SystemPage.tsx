import { ClipboardCopy, Cpu, HardDrive, Layers, MemoryStick, Monitor, Package, Server, TriangleAlert } from 'lucide-react';
import { useState } from 'react';

import { api } from '../api/client';
import { Badge } from '../components/Badge';
import { Button } from '../components/Button';
import { Card } from '../components/Card';
import { DiagnosticDialog } from '../components/dialogs/DiagnosticDialog';
import { KeyValue, KeyValueList } from '../components/KeyValue';
import { Notice } from '../components/Notice';
import { PageHeader } from '../components/PageHeader';
import { ErrorState, LoadingState } from '../components/StateViews';
import { useI18n } from '../i18n';
import { useStatus } from '../state/Status';
import { useResource } from '../state/useResource';
import { HealthCenter } from './system/HealthCenter';
import { HintsSection } from './system/HintsSection';
import { HyprlandSection } from './system/HyprlandSection';

export function SystemPage() {
  const { t, fmt } = useI18n();
  const status = useStatus();
  const [diagnosticOpen, setDiagnosticOpen] = useState(false);
  const system = useResource('system', () => api.getSystemInfo(), status.activityVersion);
  const health = useResource('health', () => api.getHealth(), status.activityVersion);
  const info = system.data;

  return (
    <div className="page">
      <PageHeader
        title={t('system.title')}
        subtitle={t('system.subtitle')}
        actions={
          <Button variant="primary" icon={<ClipboardCopy />} onClick={() => setDiagnosticOpen(true)}>
            {t('system.diagnose')}
          </Button>
        }
      />
      {system.error ? <ErrorState error={system.error} onRetry={system.reload} /> : null}
      {!info && !system.error ? <LoadingState /> : null}
      {info ? (
        <>
          {!info.os.isArchBased ? (
            <Notice tone="danger" title={t('state.notArchTitle')}>
              <p>{t('state.notArchText')}</p>
            </Notice>
          ) : null}
          <div className="card-grid">
            <Card title={t('system.section.os')} icon={<Server />}>
              <KeyValueList>
                <KeyValue label={t('system.os.name')}>{info.os.prettyName}</KeyValue>
                <KeyValue label={t('system.os.id')}>
                  <code>{info.os.id}</code>
                </KeyValue>
                {info.os.buildId ? <KeyValue label={t('system.os.build')}>{info.os.buildId}</KeyValue> : null}
                <KeyValue label={t('system.os.cachyos')}>{info.os.isCachyos ? t('system.os.cachyosYes') : t('system.os.cachyosNo')}</KeyValue>
                <KeyValue label={t('system.os.archBased')}>{info.os.isArchBased ? t('common.yes') : t('common.no')}</KeyValue>
              </KeyValueList>
            </Card>
            <Card title={t('system.section.kernel')} icon={<Layers />}>
              <KeyValueList>
                <KeyValue label={t('system.kernel.release')}>
                  <span className="mono">{info.kernel.release}</span>
                </KeyValue>
                <KeyValue label={t('system.kernel.package')}>{info.kernel.package ?? t('common.unknown')}</KeyValue>
                <KeyValue label={t('system.kernel.uptime')}>{fmt.duration(info.kernel.uptimeSeconds)}</KeyValue>
              </KeyValueList>
              {info.kernel.modulesMissing ? (
                <p className="status-line status-line--warning">
                  <TriangleAlert aria-hidden="true" />
                  {t('system.kernel.modulesMissing')}
                </p>
              ) : null}
            </Card>
            <Card title={t('system.section.cpu')} icon={<Cpu />}>
              <KeyValueList>
                <KeyValue label={t('system.cpu.model')}>{info.cpu.model}</KeyValue>
                {info.cpu.vendor ? <KeyValue label={t('system.cpu.vendor')}>{info.cpu.vendor}</KeyValue> : null}
                <KeyValue label={t('system.cpu.cores')}>{t('system.cpu.coresValue', { cores: info.cpu.cores, threads: info.cpu.threads })}</KeyValue>
                <KeyValue label={t('system.cpu.isa')}>{info.cpu.isaLevel ?? t('common.unknown')}</KeyValue>
              </KeyValueList>
            </Card>
            <Card title={t('system.section.gpu')} icon={<Monitor />}>
              {info.gpus.length === 0 ? (
                <p className="muted">{t('system.gpu.none')}</p>
              ) : (
                info.gpus.map((gpu) => (
                  <KeyValueList key={gpu.pciId}>
                    <KeyValue label={gpu.vendor}>{gpu.model}</KeyValue>
                    <KeyValue label={t('system.gpu.driver')}>{gpu.driver ?? t('common.none')}</KeyValue>
                    <KeyValue label={t('system.gpu.pci')}>
                      <code>{gpu.pciId}</code>
                    </KeyValue>
                  </KeyValueList>
                ))
              )}
            </Card>
            <Card title={t('system.section.memory')} icon={<MemoryStick />}>
              <KeyValueList>
                <KeyValue label={t('system.memory.total')}>{fmt.bytes(info.memory.totalBytes)}</KeyValue>
                <KeyValue label={t('system.memory.available')}>{fmt.bytes(info.memory.availableBytes)}</KeyValue>
                <KeyValue label={t('system.memory.swap')}>
                  {info.memory.swapTotalBytes > 0
                    ? t('system.memory.swapValue', { free: fmt.bytes(info.memory.swapFreeBytes), total: fmt.bytes(info.memory.swapTotalBytes) })
                    : t('system.memory.noSwap')}
                </KeyValue>
              </KeyValueList>
            </Card>
            <Card title={t('system.section.disks')} icon={<HardDrive />}>
              <ul className="disk-list">
                {info.disks.map((disk) => {
                  const used = disk.totalBytes > 0 ? disk.totalBytes - disk.availableBytes : 0;
                  return (
                    <li key={disk.mountPoint} className="disk-list__item">
                      <span className="mono">{disk.mountPoint}</span>
                      <meter
                        className="meter"
                        min={0}
                        max={disk.totalBytes || 1}
                        low={(disk.totalBytes || 1) * 0.8}
                        high={(disk.totalBytes || 1) * 0.9}
                        optimum={0}
                        value={used}
                        aria-label={t('system.disk.usageLabel', { mount: disk.mountPoint })}
                      />
                      <span className="muted">
                        {t('system.disk.value', { available: fmt.bytes(disk.availableBytes), total: fmt.bytes(disk.totalBytes), filesystem: disk.filesystem })}
                      </span>
                    </li>
                  );
                })}
              </ul>
            </Card>
            <Card title={t('system.section.session')} icon={<Monitor />}>
              <KeyValueList>
                <KeyValue label={t('system.session.kind')}>{t(`session.${info.session.kind}`)}</KeyValue>
                <KeyValue label={t('system.session.desktop')}>{info.session.desktop ?? t('common.unknown')}</KeyValue>
                <KeyValue label={t('system.session.type')}>{info.session.sessionType ?? t('common.unknown')}</KeyValue>
              </KeyValueList>
            </Card>
            <Card title={t('system.section.pacman')} icon={<Package />}>
              <KeyValueList>
                <KeyValue label={t('system.pacman.version')}>{info.pacman.pacmanVersion ?? t('common.unknown')}</KeyValue>
                <KeyValue label={t('system.pacman.backend')}>
                  {info.pacman.backend.state === 'ready' ? (
                    t('system.pacman.backendReady', { version: info.pacman.backend.libalpmVersion, builtAgainst: info.pacman.backend.builtAgainst })
                  ) : (
                    <>
                      {t('system.pacman.backendUnavailable')}
                      <span className="block">{t(`backend.problem.${info.pacman.backend.problem}`)}</span>
                      <span className="muted block break">{t('backend.technicalReason', { reason: info.pacman.backend.reason })}</span>
                    </>
                  )}
                </KeyValue>
                <KeyValue label={t('system.pacman.lock')}>
                  {info.pacman.lock.state === 'free' ? (
                    <Badge tone="success">{t('system.pacman.lockFree')}</Badge>
                  ) : (
                    <Badge tone="warning">{t('system.pacman.lockLocked')}</Badge>
                  )}
                </KeyValue>
                {info.pacman.backend.state === 'ready' ? (
                  <>
                    <KeyValue label={t('system.pacman.installed')}>{fmt.number(info.pacman.installedCount)}</KeyValue>
                    <KeyValue label={t('system.pacman.foreign')}>{fmt.number(info.pacman.foreignCount)}</KeyValue>
                  </>
                ) : null}
                <KeyValue label={t('system.pacman.repositories')}>{info.pacman.repositories.join(', ') || t('common.none')}</KeyValue>
                <KeyValue label={t('system.pacman.lastUpgrade')}>
                  {info.pacman.lastFullUpgrade !== null ? fmt.dateTime(info.pacman.lastFullUpgrade) : t('common.unknown')}
                </KeyValue>
              </KeyValueList>
            </Card>
          </div>
        </>
      ) : null}

      <div className="card-grid">
        <HyprlandSection />
        {health.error ? <ErrorState error={health.error} onRetry={health.reload} /> : null}
        {!health.data && !health.error ? (
          <section id="health" className="card card--wide">
            <LoadingState />
          </section>
        ) : null}
        {health.data ? <HealthCenter report={health.data} /> : null}
        <HintsSection updaters={health.data?.externalUpdaters ?? null} />
      </div>

      {diagnosticOpen ? <DiagnosticDialog onClose={() => setDiagnosticOpen(false)} /> : null}
    </div>
  );
}
