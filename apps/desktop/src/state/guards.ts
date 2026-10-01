import type { AppInfo } from '../bindings/AppInfo';
import type { Dashboard } from '../bindings/Dashboard';
import type { MessageKey } from '../i18n';
import { useI18n } from '../i18n';
import { useAppState } from './AppState';
import { useOperations } from './Operations';
import { useStatus } from './Status';

export type BlockReason =
  | 'backendUnavailable'
  | 'helperMissing'
  | 'operationActive'
  | 'resultPending'
  | 'locked'
  | 'offlinePrepared';

export interface GuardInput {
  appInfo: Pick<AppInfo, 'backend' | 'helperAvailable'>;
  dashboard: Pick<Dashboard, 'lock' | 'offlineUpdatePrepared'> | null;
  operationActive: boolean;
  resultPending: boolean;
}

/**
 * Reason why no package changing action may start right now, in order of
 * relevance. Nothing is ever started in parallel to another operation or a
 * foreign pacman lock.
 */
export function packageActionBlock(input: GuardInput): BlockReason | null {
  if (input.appInfo.backend.state === 'unavailable') return 'backendUnavailable';
  if (!input.appInfo.helperAvailable) return 'helperMissing';
  if (input.operationActive) return 'operationActive';
  if (input.resultPending) return 'resultPending';
  if (input.dashboard?.lock.state === 'locked') return 'locked';
  if (input.dashboard?.offlineUpdatePrepared) return 'offlinePrepared';
  return null;
}

export const BLOCK_MESSAGES: Record<BlockReason, MessageKey> = {
  backendUnavailable: 'guard.backendUnavailable',
  helperMissing: 'guard.helperMissing',
  operationActive: 'guard.operationActive',
  resultPending: 'guard.resultPending',
  locked: 'guard.locked',
  offlinePrepared: 'guard.offlinePrepared',
};

export interface Guard {
  reason: BlockReason | null;
  message: string | null;
}

export function usePackageActionGuard(): Guard {
  const { t } = useI18n();
  const { appInfo } = useAppState();
  const { dashboard } = useStatus();
  const { active, resultPending } = useOperations();
  const reason = packageActionBlock({ appInfo, dashboard, operationActive: active, resultPending });
  return { reason, message: reason ? t(BLOCK_MESSAGES[reason]) : null };
}
