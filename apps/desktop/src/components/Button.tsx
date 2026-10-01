import { LoaderCircle } from 'lucide-react';
import type { ButtonHTMLAttributes, ReactNode } from 'react';

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
  size?: 'md' | 'sm';
  icon?: ReactNode;
  /**
   * Shows a spinner and ignores activation (`aria-busy`, `aria-disabled`). The
   * button stays focusable: a `disabled` element would lose the keyboard focus
   * to <body> while the action runs.
   */
  busy?: boolean;
}

export function Button({ variant = 'secondary', size = 'md', icon, busy = false, disabled, className, children, type = 'button', onClick, ...rest }: ButtonProps) {
  const classes = ['btn', `btn--${variant}`, size === 'sm' ? 'btn--sm' : '', className ?? ''].filter(Boolean).join(' ');
  return (
    <button
      type={type}
      className={classes}
      disabled={disabled}
      aria-disabled={busy && !disabled ? true : undefined}
      aria-busy={busy || undefined}
      onClick={busy ? (event) => event.preventDefault() : onClick}
      {...rest}
    >
      {busy ? <LoaderCircle className="btn__icon spin" aria-hidden="true" /> : icon ? <span className="btn__icon" aria-hidden="true">{icon}</span> : null}
      <span>{children}</span>
    </button>
  );
}
