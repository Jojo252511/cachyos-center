import { screen, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';

import { mockBackend, renderApp } from '../test/utils';
import { ActivityPage } from './ActivityPage';

describe('ActivityPage', () => {
  it('shows how many updates a check found, and no result for other entries', async () => {
    mockBackend();
    renderApp(<ActivityPage />);
    const result = await screen.findByText('12 Updates gefunden');
    const check = result.closest('li');
    expect(check).not.toBeNull();
    expect(within(check as HTMLElement).getByText('Updateprüfung')).toBeInTheDocument();
    expect(within(check as HTMLElement).getByText('Timer')).toBeInTheDocument();
    expect(screen.getAllByText('Ergebnis')).toHaveLength(1);
  });
});
