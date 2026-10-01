import { Lock } from 'lucide-react';

import type { LockStatus } from '../bindings/LockStatus';
import { useI18n } from '../i18n';
import { useAppState } from '../state/AppState';
import { Notice } from './Notice';

/** „Paketverwaltung beschäftigt“: never offers to remove the lock. */
export function LockNotice({ lock }: { lock: LockStatus }) {
  const { t, fmt } = useI18n();
  if (lock.state !== 'locked') return null;
  return (
    <Notice tone="warning" title={t('state.lockedTitle')}>
      <p>{t('state.lockedText')}</p>
      {lock.since !== null ? <p>{t('state.lockedSince', { time: fmt.dateTime(lock.since) })}</p> : null}
      {lock.holderRunning === false ? <p>{t('state.lockedNoHolder')}</p> : null}
    </Notice>
  );
}

/** Reason why a package action is not possible right now. */
export function GuardNotice({ message }: { message: string | null }) {
  if (!message) return null;
  return (
    <p className="guard-note">
      <Lock aria-hidden="true" className="guard-note__icon" />
      <span>{message}</span>
    </p>
  );
}

/** Missing helper (permissions) and unavailable package backend. */
export function PlatformNotices() {
  const { t } = useI18n();
  const { appInfo } = useAppState();
  return (
    <>
      {appInfo.backend.state === 'unavailable' ? (
        <Notice tone="danger" title={t('state.unsupportedTitle')}>
          <p>{t('state.unsupportedText')}</p>
          <p>{t(`backend.problem.${appInfo.backend.problem}`)}</p>
          <p className="muted break">{t('backend.technicalReason', { reason: appInfo.backend.reason })}</p>
        </Notice>
      ) : null}
      {!appInfo.helperAvailable ? (
        <Notice tone="warning" title={t('state.helperMissingTitle')}>
          <p>{t('state.helperMissingText')}</p>
          {appInfo.helperError ? <p className="muted">{appInfo.helperError}</p> : null}
        </Notice>
      ) : null}
    </>
  );
}
