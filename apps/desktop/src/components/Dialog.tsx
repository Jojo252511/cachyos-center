import { X } from 'lucide-react';
import { Dialog as RadixDialog } from 'radix-ui';
import { useLayoutEffect, useRef, type ReactNode } from 'react';

import { useI18n } from '../i18n';

export interface AppDialogProps {
  open: boolean;
  onOpenChange(open: boolean): void;
  title: ReactNode;
  description?: ReactNode;
  /** Transaction dialogs enforce a minimum width and become a full-screen sheet below it. */
  variant?: 'transaction' | 'default';
  footer?: ReactNode;
  children?: ReactNode;
  /** Prevents closing while a start request is in flight. */
  locked?: boolean;
}

/** Focuses `element` when it can still take the focus, otherwise the main content. */
function restoreFocus(element: HTMLElement | null): void {
  const usable =
    element !== null &&
    element.isConnected &&
    !(element instanceof HTMLButtonElement && element.disabled) &&
    element !== document.body;
  if (usable) {
    element.focus();
    if (document.activeElement === element) return;
  }
  document.getElementById('main-content')?.focus();
}

/**
 * Modal dialog with focus trap and labelled title (Radix). The dialogs are
 * opened from application state, not from a Radix trigger, so the element
 * that had the focus is remembered here and gets it back on close.
 */
export function AppDialog({ open, onOpenChange, title, description, variant = 'default', footer, children, locked = false }: AppDialogProps) {
  const { t } = useI18n();
  const returnFocus = useRef<HTMLElement | null>(null);
  // Layout effects run before Radix moves the focus into the dialog.
  useLayoutEffect(() => {
    if (open) {
      const active = document.activeElement;
      returnFocus.current = active instanceof HTMLElement ? active : null;
    }
  }, [open]);
  const handleOpenChange = (next: boolean) => {
    if (!next && locked) return;
    onOpenChange(next);
  };
  return (
    <RadixDialog.Root open={open} onOpenChange={handleOpenChange}>
      <RadixDialog.Portal>
        <RadixDialog.Overlay className="dialog-overlay" />
        <RadixDialog.Content
          className={`dialog dialog--${variant}`}
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            restoreFocus(returnFocus.current);
            returnFocus.current = null;
          }}
          // Without a description the default reference would point to nothing.
          {...(description ? {} : { 'aria-describedby': undefined })}
        >
          <header className="dialog__header">
            <RadixDialog.Title className="dialog__title">{title}</RadixDialog.Title>
            <RadixDialog.Close asChild>
              <button type="button" className="icon-button" aria-label={t('common.close')} disabled={locked}>
                <X aria-hidden="true" />
              </button>
            </RadixDialog.Close>
          </header>
          {description ? <RadixDialog.Description className="dialog__description">{description}</RadixDialog.Description> : null}
          <div className="dialog__body">{children}</div>
          {footer ? <footer className="dialog__footer">{footer}</footer> : null}
        </RadixDialog.Content>
      </RadixDialog.Portal>
    </RadixDialog.Root>
  );
}
