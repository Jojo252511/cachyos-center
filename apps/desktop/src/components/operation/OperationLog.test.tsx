import { fireEvent, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it } from 'vitest';

import { watchFocusLoss } from '../../lib/focus';
import { renderI18n } from '../../test/utils';
import { OperationLog } from './OperationLog';

describe('OperationLog', () => {
  let stop: () => void = () => undefined;
  afterEach(() => stop());

  it('gives the focus to the log when „Zum Ende springen“ disappears', async () => {
    const user = userEvent.setup();
    const { container } = renderI18n(<OperationLog lines={['(1/3) upgrading mesa', '(2/3) upgrading linux-cachyos']} />);
    stop = watchFocusLoss(container);
    const log = screen.getByRole('log');
    // jsdom has no layout: a log scrolled away from its end.
    Object.defineProperty(log, 'scrollHeight', { configurable: true, value: 1000 });
    Object.defineProperty(log, 'clientHeight', { configurable: true, value: 100 });
    log.scrollTop = 0;
    fireEvent.scroll(log);
    const jump = await screen.findByRole('button', { name: 'Zum Ende springen' });
    jump.focus();
    await user.keyboard('{Enter}');
    expect(screen.queryByRole('button', { name: 'Zum Ende springen' })).toBeNull();
    await new Promise((resolve) => window.setTimeout(resolve, 10));
    expect(log).toHaveFocus();
  });
});
