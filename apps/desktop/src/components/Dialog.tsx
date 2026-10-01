import { X } from 'lucide-react';
import { Dialog as RadixDialog } from 'radix-ui';
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react';

import { useI18n } from '../i18n';
import { focusWithSuccessor, successorsOf, watchFocusLoss, type Successors } from '../lib/focus';

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

/**
 * Modal dialog with focus trap and labelled title (Radix). The dialogs are
 * opened from application state, not from a Radix trigger, so the element
 * that had the focus is remembered here and gets it back on close.
 */
export function AppDialog({ open, onOpenChange, title, description, variant = 'default', footer, children, locked = false }: AppDialogProps) {
  const { t } = useI18n();
  const returnFocus = useRef<HTMLElement | null>(null);
  const returnSuccessors = useRef<Successors>(successorsOf(null));
  // State, not a ref: the portal mounts the content only after the first render.
  const [content, setContent] = useState<HTMLDivElement | null>(null);
  // Layout effects run before Radix moves the focus into the dialog.
  useLayoutEffect(() => {
    if (open) {
      const active = document.activeElement;
      returnFocus.current = active instanceof HTMLElement ? active : null;
      returnSuccessors.current = successorsOf(returnFocus.current);
    }
  }, [open]);
  // A control in the dialog that is disabled while it has the focus hands it to the dialog
  // (removed controls are handled by the focus trap of Radix).
  useEffect(() => (open && content ? watchFocusLoss(content, content) : undefined), [open, content]);
  const handleOpenChange = (next: boolean) => {
    if (!next && locked) return;
    onOpenChange(next);
  };
  return (
    <RadixDialog.Root open={open} onOpenChange={handleOpenChange}>
      <RadixDialog.Portal>
        <RadixDialog.Overlay className="dialog-overlay" />
        <RadixDialog.Content
          ref={setContent}
          className={`dialog dialog--${variant}`}
          onCloseAutoFocus={(event) => {
            event.preventDefault();
            // The opener may be gone or disabled by now (e.g. „Installieren“ while the installation runs).
            focusWithSuccessor(returnFocus.current, returnSuccessors.current);
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
