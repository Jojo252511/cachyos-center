import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';

import { Button } from './Button';

describe('Button', () => {
  it('ignores clicks and Enter while busy but stays focusable', async () => {
    const user = userEvent.setup();
    const onClick = vi.fn();
    render(
      <Button busy onClick={onClick}>
        Übernehmen
      </Button>,
    );
    const button = screen.getByRole('button', { name: 'Übernehmen' });
    expect(button).toHaveAttribute('aria-busy', 'true');
    expect(button).toHaveAttribute('aria-disabled', 'true');
    expect(button).not.toBeDisabled();
    await user.dblClick(button);
    button.focus();
    await user.keyboard('{Enter}{Enter} ');
    expect(onClick).not.toHaveBeenCalled();
    expect(button).toHaveFocus();
  });

  it('runs the action when not busy', async () => {
    const user = userEvent.setup();
    const onClick = vi.fn();
    const { rerender } = render(
      <Button busy onClick={onClick}>
        Übernehmen
      </Button>,
    );
    rerender(<Button onClick={onClick}>Übernehmen</Button>);
    const button = screen.getByRole('button', { name: 'Übernehmen' });
    expect(button).not.toHaveAttribute('aria-disabled');
    expect(button).not.toHaveAttribute('aria-busy');
    await user.click(button);
    expect(onClick).toHaveBeenCalledTimes(1);
  });

  it('is really disabled when disabled, also while busy', () => {
    render(
      <Button busy disabled>
        Installieren
      </Button>,
    );
    const button = screen.getByRole('button', { name: 'Installieren' });
    expect(button).toBeDisabled();
    expect(button).not.toHaveAttribute('aria-disabled');
  });
});
