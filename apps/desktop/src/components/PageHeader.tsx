import type { ReactNode } from 'react';

/** Page title (focus target after navigation), subtitle and the page actions. */
export function PageHeader({ title, subtitle, actions }: { title: ReactNode; subtitle?: ReactNode; actions?: ReactNode }) {
  return (
    <header className="page-header">
      <div className="page-header__text">
        <h1 className="page-header__title" id="page-title" tabIndex={-1}>
          {title}
        </h1>
        {subtitle ? <p className="page-header__subtitle">{subtitle}</p> : null}
      </div>
      {/* An action that disappears (e.g. „Jetzt prüfen“ replaced by „Updates ansehen“) hands the focus to the next one. */}
      {actions ? (
        <div className="page-header__actions" data-focus-group>
          {actions}
        </div>
      ) : null}
    </header>
  );
}
