/**
 * Keyboard focus after an element disappears (closing a panel, dismissing a
 * result, a button that is no longer rendered or is disabled). Without this
 * the focus falls back to <body> and keyboard and screen reader users lose
 * their place.
 *
 * Successors are declared in the markup:
 * - `data-focus-fallback="id"`: the element with this id takes the focus when
 *   the marked element, or a focused element inside it, disappears;
 * - `data-focus-group`: the first focusable element left in the group takes it.
 * Without either the page title takes it, then the main content.
 */

export const FOCUS_FALLBACK = 'data-focus-fallback';
export const FOCUS_GROUP = 'data-focus-group';

const FOCUSABLE = 'a[href], button, input, select, textarea, [tabindex]:not([tabindex="-1"])';

function canTakeFocus(element: Element | null | undefined): element is HTMLElement {
  return element instanceof HTMLElement && element.isConnected && element !== document.body && !element.matches(':disabled');
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

function firstFocusable(group: Element | null | undefined): HTMLElement | null {
  if (!group?.isConnected) return null;
  for (const element of group.querySelectorAll(FOCUSABLE)) {
    if (canTakeFocus(element)) return element;
  }
  return null;
}

/** The closest elements that declare a successor for the focus of an element. */
export interface Successors {
  fallback: Element | null;
  group: Element | null;
}

/**
 * The declared successors of `element`. Look them up while the element is in
 * the document: once removed, its ancestors outside the removed part are unknown.
 */
export function successorsOf(element: Element | null): Successors {
  return {
    fallback: element?.closest(`[${FOCUS_FALLBACK}]`) ?? null,
    group: element?.closest(`[${FOCUS_GROUP}]`) ?? null,
  };
}

/**
 * Focuses `element`, or its declared successor when it is gone or disabled;
 * `last` is tried before the page title.
 */
export function focusWithSuccessor(element: HTMLElement | null, successors: Successors = successorsOf(element), last: HTMLElement | null = null): void {
  const fallback = successors.fallback?.getAttribute(FOCUS_FALLBACK);
  focusFirst(element, fallback ? document.getElementById(fallback) : null, firstFocusable(successors.group), last);
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

/**
 * Keeps the keyboard focus when a re-render removes or disables the focused
 * element inside `root`: a button replaced by a link, an empty state or error
 * panel that disappears, an apply button that is disabled after saving.
 * Browsers move the focus to <body> then. WebKitGTK, the engine of the app on
 * Linux, fires no blur event for a removed element, so removals are observed
 * with a MutationObserver (like the focus trap of the Radix dialogs); for a
 * disabled element it fires a late blur, handled through `focusout`.
 *
 * The check runs after a zero timeout, so focus moves that an event handler
 * scheduled with `focusSoon` come first. Without a declared successor
 * `fallback` takes the focus, then the page title; a dialog watches its own
 * content and passes itself. Once `root` has left the document (a dialog
 * closed) nothing is moved. Returns the function that stops watching.
 */
export function watchFocusLoss(root: HTMLElement, fallback: HTMLElement | null = null): () => void {
  let focused: HTMLElement | null = null;
  let successors = successorsOf(null);
  let timer: number | undefined;

  const lost = (element: HTMLElement) => !element.isConnected || element.matches(':disabled');

  const check = () => {
    timer = undefined;
    const element = focused;
    const active = document.activeElement;
    if (!element || !root.isConnected || (active instanceof HTMLElement && active !== document.body && active.isConnected)) return;
    focused = null;
    // Still there and usable: the focus was left on purpose, e.g. by a click on an empty area.
    if (lost(element)) focusWithSuccessor(element, successors, fallback);
  };
  const schedule = () => {
    if (timer === undefined) timer = window.setTimeout(check, 0);
  };
  const track = (event: FocusEvent) => {
    focused = event.target instanceof HTMLElement && root.contains(event.target) ? event.target : null;
    successors = successorsOf(focused);
  };
  const observer = new MutationObserver(() => {
    if (focused && lost(focused)) schedule();
  });

  observer.observe(root, { subtree: true, childList: true, attributes: true, attributeFilter: ['disabled'] });
  document.addEventListener('focusin', track);
  document.addEventListener('focusout', schedule);
  return () => {
    observer.disconnect();
    document.removeEventListener('focusin', track);
    document.removeEventListener('focusout', schedule);
    window.clearTimeout(timer);
  };
}
