import { useId, type ReactNode } from 'react';

export interface CardProps {
  title: ReactNode;
  icon?: ReactNode;
  actions?: ReactNode;
  footer?: ReactNode;
  children: ReactNode;
  className?: string;
  /** Heading level inside the page (defaults to h2). */
  level?: 2 | 3;
  id?: string;
}

export function Card({ title, icon, actions, footer, children, className, level = 2, id }: CardProps) {
  const headingId = useId();
  const Heading = level === 2 ? 'h2' : 'h3';
  return (
    <section className={`card ${className ?? ''}`} aria-labelledby={headingId} id={id}>
      <header className="card__header">
        <Heading className="card__title" id={headingId}>
          {icon ? <span className="card__icon" aria-hidden="true">{icon}</span> : null}
          {title}
        </Heading>
        {actions ? <div className="card__actions">{actions}</div> : null}
      </header>
      <div className="card__body">{children}</div>
      {footer ? <footer className="card__footer">{footer}</footer> : null}
    </section>
  );
}
