import { Monitor } from 'lucide-react';
import { useEffect } from 'react';

import { api } from '../../api/client';
import { Badge } from '../../components/Badge';
import { Card } from '../../components/Card';
import { CopyButton } from '../../components/CopyButton';
import { KeyValue, KeyValueList } from '../../components/KeyValue';
import { Notice } from '../../components/Notice';
import { ErrorState, LoadingState } from '../../components/StateViews';
import { useI18n } from '../../i18n';
import { useResource } from '../../state/useResource';

/** Hyprland session details; refreshed on the `hyprland-changed` event. */
export function HyprlandSection() {
  const { t, fmt } = useI18n();
  const info = useResource('hyprland', () => api.getHyprlandInfo());
  const { reload } = info;

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    api.onHyprlandChanged(() => reload()).then(
      (fn) => (disposed ? fn() : (unlisten = fn)),
      () => undefined,
    );
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [reload]);

  const data = info.data;
  return (
    <Card title={t('system.section.hyprland')} icon={<Monitor />} className="card--wide">
      {info.error ? <ErrorState error={info.error} onRetry={reload} /> : null}
      {!data && !info.error ? <LoadingState /> : null}
      {data && !data.available ? (
        <Notice tone="info" title={t(`system.hyprland.unavailable.${data.reason ?? 'noSession'}`)}>
          <p>{t('system.hyprland.unavailableText')}</p>
        </Notice>
      ) : null}
      {data?.available ? (
        <>
          <KeyValueList>
            <KeyValue label={t('system.hyprland.version')}>{data.version ?? t('common.unknown')}</KeyValue>
            <KeyValue label={t('system.hyprland.activeWorkspace')}>{data.activeWorkspace ?? t('common.unknown')}</KeyValue>
            <KeyValue label={t('system.hyprland.workspaceCount')}>{data.workspaceCount !== null ? fmt.number(data.workspaceCount) : t('common.unknown')}</KeyValue>
            <KeyValue label={t('system.hyprland.appClass')}>
              <code>{data.appClass}</code> <CopyButton text={data.appClass} />
            </KeyValue>
          </KeyValueList>
          <h3 className="card__subtitle">{t('system.hyprland.monitors')}</h3>
          <ul className="monitor-list">
            {data.monitors.map((monitor) => (
              <li key={monitor.name} className="monitor-list__item">
                <span className="mono">{monitor.name}</span>
                <span>{monitor.description}</span>
                <span className="muted">
                  {t('system.hyprland.monitorValue', {
                    width: monitor.width,
                    height: monitor.height,
                    rate: fmt.number(Math.round(monitor.refreshRate * 100) / 100),
                    scale: fmt.number(monitor.scale),
                  })}
                </span>
                {monitor.activeWorkspace ? <span className="muted">{t('system.hyprland.monitorWorkspace', { name: monitor.activeWorkspace })}</span> : null}
                {monitor.focused ? <Badge tone="accent">{t('system.hyprland.focused')}</Badge> : null}
              </li>
            ))}
          </ul>
        </>
      ) : null}
      <p className="muted">{t('system.hyprland.readonly')}</p>
    </Card>
  );
}
