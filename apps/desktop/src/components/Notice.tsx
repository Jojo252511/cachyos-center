import { CircleCheck, Info, OctagonAlert, TriangleAlert } from 'lucide-react';
import type { ReactNode } from 'react';

export type NoticeTone = 'info' | 'success' | 'warning' | 'danger';

const ICONS: Record<NoticeTone, ReactNode> = {
  info: <Info aria-hidden="true" />,
  success: <CircleCheck aria-hidden="true" />,
  warning: <TriangleAlert aria-hidden="true" />,
  danger: <OctagonAlert aria-hidden="true" />,
};

export interface NoticeProps {
  tone?: NoticeTone;
  title?: ReactNode;
  children?: ReactNode;
  actions?: ReactNode;
  /** `alert` for errors that appear after a user action, `status` for live updates. */
  role?: 'alert' | 'status';
  className?: string;
}

/** Callout with icon and text; the tone is never conveyed by color alone. */
export function Notice({ tone = 'info', title, children, actions, role, className }: NoticeProps) {
  return (
    <div className={`notice notice--${tone} ${className ?? ''}`} role={role}>
      <span className="notice__icon">{ICONS[tone]}</span>
      <div className="notice__body">
        {title ? <p className="notice__title">{title}</p> : null}
        {children ? <div className="notice__text">{children}</div> : null}
        {actions ? <div className="notice__actions">{actions}</div> : null}
      </div>
    </div>
  );
}
