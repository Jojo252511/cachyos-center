import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { api } from '../../api/client';
import { mockBackend, renderApp } from '../../test/utils';
import { DiagnosticDialog } from './DiagnosticDialog';

describe('DiagnosticDialog', () => {
  it('copies the sanitized report only after the preview and an explicit click', async () => {
    const user = userEvent.setup();
    mockBackend();
    const writeClipboard = vi.spyOn(api, 'writeClipboard').mockResolvedValue(undefined);
    renderApp(<DiagnosticDialog onClose={vi.fn()} />);

    const preview = await screen.findByLabelText('Vorschau des Diagnoseberichts');
    expect(preview).toHaveTextContent('cachyos-center diagnostic report (sanitized)');
    expect(screen.getByText(/Er ist auf Englisch und enthält keine Benutzernamen/)).toBeInTheDocument();
    expect(writeClipboard).not.toHaveBeenCalled();

    await user.click(screen.getByRole('button', { name: 'Kopieren' }));
    expect(writeClipboard).toHaveBeenCalledTimes(1);
    expect(writeClipboard).toHaveBeenCalledWith(preview.textContent);
    expect(await screen.findByText('Bericht in die Zwischenablage kopiert.')).toBeInTheDocument();
  });
});
