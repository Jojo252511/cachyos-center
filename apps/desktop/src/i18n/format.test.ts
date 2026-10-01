import { describe, expect, it } from 'vitest';

import { createFormatters } from './format';

describe('formatters', () => {
  const de = createFormatters('de-DE');
  const en = createFormatters('en-US');

  it('formats binary sizes per locale', () => {
    expect(de.bytes(0)).toBe('0 B');
    expect(de.bytes(1536)).toBe('1,5 KiB');
    expect(en.bytes(1536)).toBe('1.5 KiB');
    expect(de.bytes(5 * 1024 ** 3)).toBe('5 GiB');
    expect(de.signedBytes(-2 * 1024 ** 2)).toBe('−2 MiB');
    expect(de.signedBytes(2 * 1024 ** 2)).toBe('+2 MiB');
  });

  it('formats relative times and durations', () => {
    const now = Date.UTC(2026, 9, 1, 12, 0, 0);
    const seconds = now / 1000;
    expect(de.relative(seconds - 5 * 60, now)).toBe('vor 5 Minuten');
    expect(en.relative(seconds - 3 * 3600, now)).toBe('3 hours ago');
    expect(de.relative(seconds + 2 * 3600, now)).toBe('in 2 Stunden');
    expect(de.duration(2 * 86400 + 5 * 3600 + 17 * 60)).toBe('2 Tage, 5 Stunden');
    expect(en.duration(17 * 60)).toBe('17 minutes');
  });
});
