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

  it('stays focusable while busy even when also disabled, and is disabled again afterwards', async () => {
    const user = userEvent.setup();
    const onClick = vi.fn();
    const { rerender } = render(
      <Button busy disabled onClick={onClick}>
        Upgrade starten
      </Button>,
    );
    const button = screen.getByRole('button', { name: 'Upgrade starten' });
    // E.g. the start is blocked as soon as the operation runs, while the start request is still pending.
    expect(button).not.toBeDisabled();
    expect(button).toHaveAttribute('aria-disabled', 'true');
    button.focus();
    await user.keyboard('{Enter}');
    expect(onClick).not.toHaveBeenCalled();
    expect(button).toHaveFocus();
    rerender(
      <Button disabled onClick={onClick}>
        Upgrade starten
      </Button>,
    );
    expect(button).toBeDisabled();
    expect(button).not.toHaveAttribute('aria-disabled');
  });
});
