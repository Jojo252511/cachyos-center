import { afterEach, describe, expect, it } from 'vitest';

import { focusSoon, focusWithSuccessor, successorsOf, watchFocusLoss } from './focus';

/** Lets the zero timeouts of the check and of `focusSoon` run. */
const settle = () => new Promise((resolve) => window.setTimeout(resolve, 10));

const byId = (id: string) => {
  const element = document.getElementById(id);
  if (!element) throw new Error(`no #${id}`);
  return element;
};

/**
 * WebKitGTK and Chromium blur a focused control shortly after it is disabled
 * (measured with WebKitGTK 2.52); jsdom keeps the focus on it. This replays
 * that blur: `blur()` has no effect on a disabled control in jsdom.
 */
function browserBlur(button: HTMLButtonElement) {
  button.disabled = false;
  button.blur();
  button.disabled = true;
}

describe('watchFocusLoss', () => {
  let stop: () => void = () => undefined;
  const events: string[] = [];
  const record = (event: FocusEvent) => events.push(`${event.type}:${(event.target as HTMLElement).id}`);

  const mount = (html: string) => {
    document.body.innerHTML = `<div id="app"><h1 id="page-title" tabindex="-1">Updates</h1>${html}</div><div id="portal"><button id="in-dialog">Im Dialog</button></div>`;
    stop = watchFocusLoss(byId('app'));
    events.length = 0;
    document.addEventListener('focusin', record);
    document.addEventListener('focusout', record);
  };

  afterEach(() => {
    stop();
    document.removeEventListener('focusin', record);
    document.removeEventListener('focusout', record);
    document.body.innerHTML = '';
  });

  it('moves the focus of a removed element to its declared fallback, although no blur event fires', async () => {
    mount('<button id="header-check">Jetzt prüfen</button><div id="empty"><button id="check" data-focus-fallback="header-check">Jetzt prüfen</button></div>');
    byId('check').focus();
    byId('empty').remove();
    // Like WebKitGTK: the focus is on <body> and no focusout was fired.
    expect(document.activeElement).toBe(document.body);
    expect(events).toEqual(['focusin:check']);
    await settle();
    expect(byId('header-check')).toHaveFocus();
  });

  it('applies the fallback of an ancestor that was removed with the element', async () => {
    mount('<button id="header-check">Jetzt prüfen</button><div id="panel" data-focus-fallback="header-check"><button id="retry">Erneut prüfen</button></div>');
    byId('retry').focus();
    byId('panel').remove();
    await settle();
    expect(byId('header-check')).toHaveFocus();
  });

  it('applies the fallback of an ancestor that stays in the document', async () => {
    mount(
      '<section data-focus-fallback="panel-title"><h2 id="panel-title" tabindex="-1">Systemupgrade</h2><div id="cancel-row"><button id="cancel">Abbrechen</button></div></section>',
    );
    byId('cancel').focus();
    // Only the row is removed: its ancestors are no longer reachable from the removed button.
    byId('cancel-row').remove();
    await settle();
    expect(byId('panel-title')).toHaveFocus();
  });

  it('hands the focus to the next focusable element of a focus group', async () => {
    mount('<div id="actions" data-focus-group><button id="check">Jetzt prüfen</button></div>');
    byId('check').focus();
    const link = document.createElement('a');
    link.id = 'view';
    link.href = '#/updates';
    byId('actions').replaceChildren(link);
    await settle();
    expect(link).toHaveFocus();
  });

  it('skips disabled elements of a group and falls back to the page title', async () => {
    mount('<nav id="pages" data-focus-group><button id="prev" disabled>Zurück</button><button id="next">Weiter</button></nav><div id="panel"><button id="retry">Erneut prüfen</button></div>');
    const next = byId('next') as HTMLButtonElement;
    next.focus();
    next.disabled = true;
    browserBlur(next);
    await settle();
    // Both buttons of the group are disabled now.
    expect(byId('page-title')).toHaveFocus();
    byId('retry').focus();
    byId('panel').remove();
    await settle();
    expect(byId('page-title')).toHaveFocus();
  });

  it('follows a disabled element to its fallback once the browser blurs it', async () => {
    mount('<span id="applied" tabindex="-1">Richtlinie übernommen.</span><button id="apply" data-focus-fallback="applied">Übernehmen</button>');
    const apply = byId('apply') as HTMLButtonElement;
    apply.focus();
    apply.disabled = true;
    await settle();
    // Still focused (jsdom, and WebKitGTK until its late blur): nothing to do yet.
    expect(apply).toHaveFocus();
    browserBlur(apply);
    await settle();
    expect(byId('applied')).toHaveFocus();
  });

  it('leaves the focus alone when it was moved away on purpose', async () => {
    mount('<div id="panel"><button id="retry">Erneut prüfen</button></div>');
    const retry = byId('retry');
    retry.focus();
    // A click on an empty area: the element is still there and usable.
    retry.blur();
    await settle();
    expect(document.activeElement).toBe(document.body);
    // A later re-render does not pull the focus back into the page.
    byId('panel').remove();
    await settle();
    expect(document.activeElement).toBe(document.body);
  });

  it('lets a focus move of an event handler win without moving the focus twice', async () => {
    mount('<p id="news-status" tabindex="-1">Keine ungelesenen Meldungen</p><div id="row"><button id="read">Als gelesen markieren</button></div>');
    byId('read').focus();
    focusSoon(() => byId('news-status'));
    byId('row').remove();
    await settle();
    expect(byId('news-status')).toHaveFocus();
    expect(events.filter((event) => event.startsWith('focusin'))).toEqual(['focusin:read', 'focusin:news-status']);
  });

  it('uses its own fallback before the page title and does nothing once its root is gone', async () => {
    stop();
    document.body.innerHTML =
      '<h1 id="page-title" tabindex="-1">Updates</h1><div id="dialog" role="dialog" tabindex="-1"><button id="start">Upgrade starten</button><button id="other">Abbrechen</button></div>';
    const dialog = byId('dialog');
    stop = watchFocusLoss(dialog, dialog);
    const start = byId('start') as HTMLButtonElement;
    start.focus();
    start.disabled = true;
    browserBlur(start);
    await settle();
    expect(dialog).toHaveFocus();
    // The dialog closes with the focus inside: restoring it is up to the dialog.
    byId('other').focus();
    dialog.remove();
    await settle();
    expect(document.activeElement).toBe(document.body);
  });

  it('ignores elements outside its root, such as dialogs', async () => {
    mount('');
    byId('in-dialog').focus();
    byId('portal').remove();
    await settle();
    expect(document.activeElement).toBe(document.body);
  });

  it('stops watching when stopped', async () => {
    mount('<div id="panel"><button id="retry">Erneut prüfen</button></div>');
    stop();
    byId('retry').focus();
    byId('panel').remove();
    await settle();
    expect(document.activeElement).toBe(document.body);
  });
});

describe('focusWithSuccessor', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('focuses the element itself when it can take the focus', () => {
    document.body.innerHTML = '<h1 id="page-title" tabindex="-1">Software</h1><button id="opener">Installieren</button>';
    focusWithSuccessor(byId('opener'));
    expect(byId('opener')).toHaveFocus();
  });

  it('uses successors looked up before the element was removed', () => {
    document.body.innerHTML =
      '<h1 id="page-title" tabindex="-1">Updates</h1><section data-focus-fallback="panel-title"><h2 id="panel-title" tabindex="-1">Systemupgrade</h2><div id="notice"><button id="opener">Plan prüfen</button></div></section>';
    const opener = byId('opener');
    const successors = successorsOf(opener);
    byId('notice').remove();
    focusWithSuccessor(opener, successors);
    expect(byId('panel-title')).toHaveFocus();
  });

  it('focuses the declared fallback of a disabled element', () => {
    document.body.innerHTML =
      '<h1 id="page-title" tabindex="-1">Software</h1><h2 id="details-title" tabindex="-1">firefox</h2><div data-focus-fallback="details-title"><button id="opener" disabled>Installieren</button></div>';
    focusWithSuccessor(byId('opener'));
    expect(byId('details-title')).toHaveFocus();
  });

  it('falls back to the page title and then to the main content', () => {
    document.body.innerHTML = '<main id="main-content" tabindex="-1"><button id="opener" disabled>Installieren</button></main>';
    focusWithSuccessor(byId('opener'));
    expect(byId('main-content')).toHaveFocus();
    focusWithSuccessor(null);
    expect(byId('main-content')).toHaveFocus();
  });
});
