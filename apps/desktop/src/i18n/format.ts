/**
 * Locale aware formatting helpers based on `Intl`. Timestamps of the contract
 * are Unix seconds, sizes are bytes.
 */

export interface Formatters {
  number(value: number): string;
  /** Binary units (KiB, MiB, GiB) as used by pacman. */
  bytes(value: number): string;
  /** Size change with explicit sign, e.g. "+12 MiB" / "−3 MiB". */
  signedBytes(value: number): string;
  dateTime(seconds: number): string;
  date(seconds: number): string;
  /** Relative time ("vor 5 Minuten"), absolute date for older values. */
  relative(seconds: number, nowMs: number): string;
  /** Duration such as uptime ("3 Tage, 4 Stunden"). */
  duration(seconds: number): string;
  list(items: readonly string[]): string;
  /** ISO string for `<time dateTime>`. */
  iso(seconds: number): string;
}

const BYTE_UNITS = ['B', 'KiB', 'MiB', 'GiB', 'TiB'] as const;
const MINUTE = 60;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

export function createFormatters(locale: string): Formatters {
  const numberFormat = new Intl.NumberFormat(locale);
  const sizeFormat = new Intl.NumberFormat(locale, { maximumFractionDigits: 1 });
  const dateTimeFormat = new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'short' });
  const dateFormat = new Intl.DateTimeFormat(locale, { dateStyle: 'medium' });
  const relativeFormat = new Intl.RelativeTimeFormat(locale, { numeric: 'auto' });
  const listFormat = new Intl.ListFormat(locale, { style: 'long', type: 'conjunction' });
  const unitFormat = (unit: 'day' | 'hour' | 'minute') =>
    new Intl.NumberFormat(locale, { style: 'unit', unit, unitDisplay: 'long' });
  const dayFormat = unitFormat('day');
  const hourFormat = unitFormat('hour');
  const minuteFormat = unitFormat('minute');

  const bytes = (value: number): string => {
    if (!Number.isFinite(value)) return '–';
    let size = Math.abs(value);
    let unit = 0;
    while (size >= 1024 && unit < BYTE_UNITS.length - 1) {
      size /= 1024;
      unit += 1;
    }
    const formatted = unit === 0 ? numberFormat.format(size) : sizeFormat.format(size);
    return `${value < 0 ? '−' : ''}${formatted} ${BYTE_UNITS[unit]}`;
  };

  return {
    number: (value) => numberFormat.format(value),
    bytes,
    signedBytes: (value) => (value > 0 ? `+${bytes(value)}` : bytes(value)),
    dateTime: (seconds) => dateTimeFormat.format(new Date(seconds * 1000)),
    date: (seconds) => dateFormat.format(new Date(seconds * 1000)),
    relative: (seconds, nowMs) => {
      const diff = seconds - Math.round(nowMs / 1000);
      const abs = Math.abs(diff);
      if (abs < MINUTE) return relativeFormat.format(0, 'second');
      if (abs < HOUR) return relativeFormat.format(Math.round(diff / MINUTE), 'minute');
      if (abs < DAY) return relativeFormat.format(Math.round(diff / HOUR), 'hour');
      if (abs < 7 * DAY) return relativeFormat.format(Math.round(diff / DAY), 'day');
      return dateTimeFormat.format(new Date(seconds * 1000));
    },
    duration: (seconds) => {
      const days = Math.floor(seconds / DAY);
      const hours = Math.floor((seconds % DAY) / HOUR);
      const minutes = Math.floor((seconds % HOUR) / MINUTE);
      const parts: string[] = [];
      if (days > 0) parts.push(dayFormat.format(days));
      if (hours > 0) parts.push(hourFormat.format(hours));
      if (days === 0 && (minutes > 0 || hours === 0)) parts.push(minuteFormat.format(minutes));
      return parts.join(', ');
    },
    list: (items) => listFormat.format(items),
    iso: (seconds) => new Date(seconds * 1000).toISOString(),
  };
}
