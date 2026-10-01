import { LoaderCircle } from 'lucide-react';
import type { ButtonHTMLAttributes, ReactNode } from 'react';

export interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
  size?: 'md' | 'sm';
  icon?: ReactNode;
  /**
   * Shows a spinner and ignores activation (`aria-busy`, `aria-disabled`). The
   * button stays focusable, even when `disabled` is set as well: a disabled
   * element would lose the keyboard focus to <body> while the action runs
   * (e.g. during the Polkit dialog of a start).
   */
  busy?: boolean;
}

export function Button({ variant = 'secondary', size = 'md', icon, busy = false, disabled, className, children, type = 'button', onClick, ...rest }: ButtonProps) {
  const classes = ['btn', `btn--${variant}`, size === 'sm' ? 'btn--sm' : '', className ?? ''].filter(Boolean).join(' ');
  return (
    <button
      type={type}
      className={classes}
      disabled={disabled && !busy}
      aria-disabled={busy || undefined}
      aria-busy={busy || undefined}
      onClick={busy ? (event) => event.preventDefault() : onClick}
      {...rest}
    >
      {busy ? <LoaderCircle className="btn__icon spin" aria-hidden="true" /> : icon ? <span className="btn__icon" aria-hidden="true">{icon}</span> : null}
      <span>{children}</span>
    </button>
  );
}
