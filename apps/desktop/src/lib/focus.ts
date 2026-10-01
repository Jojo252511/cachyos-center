/**
 * Keyboard focus after an element disappears (closing a panel, dismissing a
 * result, a button that is no longer rendered). Without this the focus falls
 * back to <body> and keyboard and screen reader users lose their place.
 */

function canTakeFocus(element: HTMLElement | null | undefined): element is HTMLElement {
  if (!element || !element.isConnected || element === document.body) return false;
  return !(element instanceof HTMLButtonElement && element.disabled);
}

/** Focuses the first candidate that can take the focus, else the page title, else the main content. */
export function focusFirst(...candidates: Array<HTMLElement | null | undefined>): void {
  for (const candidate of [...candidates, document.getElementById('page-title'), document.getElementById('main-content')]) {
    if (canTakeFocus(candidate)) {
      candidate.focus({ preventScroll: true });
      if (document.activeElement === candidate) return;
    }
  }
}

/**
 * Like `focusFirst`, but after React has applied the current update (state
 * changes of an event handler are committed before a zero timeout runs).
 * `target` is resolved late, so it may refer to an element rendered by that update.
 */
export function focusSoon(target?: () => HTMLElement | null | undefined): void {
  window.setTimeout(() => focusFirst(target?.()), 0);
}

/** The element that currently has the focus (to give it back later). */
export function activeElement(): HTMLElement | null {
  const active = document.activeElement;
  return active instanceof HTMLElement && active !== document.body ? active : null;
}
