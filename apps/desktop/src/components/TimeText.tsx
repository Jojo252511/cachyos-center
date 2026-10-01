import { useI18n } from '../i18n';
import { useNow } from '../state/now';

/** Relative time for recent values, absolute date otherwise; the absolute value is in `title`. */
export function TimeText({ seconds, absolute = false }: { seconds: number; absolute?: boolean }) {
  const { fmt } = useI18n();
  const now = useNow();
  return (
    <time dateTime={fmt.iso(seconds)} title={fmt.dateTime(seconds)}>
      {absolute ? fmt.dateTime(seconds) : fmt.relative(seconds, now)}
    </time>
  );
}
