import { act, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { describe, expect, it } from 'vitest';

import { renderI18n } from '../test/utils';
import { AppDialog } from './Dialog';

/**
 * `disableOpener`: like a start button that stays disabled once an operation runs.
 * `fallback`: the opener declares a successor for the focus (`data-focus-fallback`).
 */
function Opener({ disableOpener = false, fallback = false }: { disableOpener?: boolean; fallback?: boolean }) {
  const [open, setOpen] = useState(false);
  const [used, setUsed] = useState(false);
  return (
    <>
      <main id="main-content" tabIndex={-1}>
        <h2 id="details-title" tabIndex={-1}>
          firefox
        </h2>
        <div data-focus-fallback={fallback ? 'details-title' : undefined}>
          <button
            type="button"
            disabled={disableOpener && used}
            onClick={() => {
              setUsed(true);
              setOpen(true);
            }}
          >
            Öffnen
          </button>
        </div>
      </main>
      <AppDialog open={open} onOpenChange={setOpen} title="Bestätigen">
        <button type="button">Im Dialog</button>
      </AppDialog>
    </>
  );
}

describe('AppDialog', () => {
  it('returns the focus to the element that opened it', async () => {
    const user = userEvent.setup();
    renderI18n(<Opener />);
    const opener = screen.getByRole('button', { name: 'Öffnen' });
    await user.click(opener);
    expect(await screen.findByRole('dialog', { name: 'Bestätigen' })).toBeInTheDocument();
    expect(opener).not.toHaveFocus();
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(opener).toHaveFocus();
  });

  it('falls back to the main content when the opener cannot take the focus', async () => {
    const user = userEvent.setup();
    renderI18n(<Opener disableOpener />);
    await user.click(screen.getByRole('button', { name: 'Öffnen' }));
    await user.click(await screen.findByRole('button', { name: 'Schließen' }));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(screen.getByRole('main')).toHaveFocus();
  });

  it('keeps the focus in the dialog when a focused control is disabled', async () => {
    const user = userEvent.setup();
    function Disabling() {
      const [locked, setLocked] = useState(false);
      return (
        <AppDialog open onOpenChange={() => undefined} title="Installieren bestätigen">
          <button type="button" disabled={locked} onClick={() => setLocked(true)}>
            Sperren
          </button>
        </AppDialog>
      );
    }
    renderI18n(<Disabling />);
    const dialog = await screen.findByRole('dialog', { name: 'Installieren bestätigen' });
    const button = screen.getByRole('button', { name: 'Sperren' });
    button.focus();
    await user.keyboard('{Enter}');
    expect(button).toBeDisabled();
    // jsdom keeps the focus on a disabled control; WebKitGTK and Chromium blur it shortly after.
    act(() => {
      button.removeAttribute('disabled');
      button.blur();
      button.setAttribute('disabled', '');
    });
    await waitFor(() => expect(dialog).toHaveFocus());
  });

  it('gives the focus to the declared successor of a disabled opener', async () => {
    const user = userEvent.setup();
    renderI18n(<Opener disableOpener fallback />);
    await user.click(screen.getByRole('button', { name: 'Öffnen' }));
    await user.click(await screen.findByRole('button', { name: 'Schließen' }));
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(screen.getByRole('heading', { name: 'firefox' })).toHaveFocus();
  });
});
