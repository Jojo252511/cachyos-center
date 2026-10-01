import { ArrowDownToLine } from 'lucide-react';
import { useEffect, useId, useLayoutEffect, useRef, useState } from 'react';

import { useI18n } from '../../i18n';
import { Button } from '../Button';

/**
 * Scrollable monospace log. It follows new lines unless the user scrolled up;
 * „Zum Ende springen“ resumes following.
 */
export function OperationLog({ lines, focusRequest = 0 }: { lines: readonly string[]; focusRequest?: number }) {
  const { t } = useI18n();
  const ref = useRef<HTMLPreElement>(null);
  const logId = useId();
  const [follow, setFollow] = useState(true);

  useLayoutEffect(() => {
    const element = ref.current;
    if (follow && element) element.scrollTop = element.scrollHeight;
  }, [lines, follow]);

  useEffect(() => {
    if (focusRequest > 0 && ref.current) {
      ref.current.scrollIntoView?.({ block: 'nearest' });
      ref.current.focus();
    }
  }, [focusRequest]);

  const onScroll = () => {
    const element = ref.current;
    if (!element) return;
    const atBottom = element.scrollHeight - element.scrollTop - element.clientHeight < 24;
    if (atBottom !== follow) setFollow(atBottom);
  };

  return (
    <div className="operation-log">
      <div className="operation-log__header">
        <h3 className="operation-log__title">{t('operation.log')}</h3>
        {!follow ? (
          <>
            <span className="muted">{t('operation.logPaused')}</span>
            {/* The button disappears once the log follows again: the log takes the focus. */}
            <Button size="sm" variant="ghost" icon={<ArrowDownToLine />} data-focus-fallback={logId} onClick={() => setFollow(true)}>
              {t('operation.logJumpToEnd')}
            </Button>
          </>
        ) : null}
      </div>
      <pre ref={ref} id={logId} className="operation-log__body" tabIndex={0} role="log" aria-live="off" aria-label={t('operation.log')} onScroll={onScroll}>
        {lines.length > 0 ? lines.join('\n') : t('operation.logEmpty')}
      </pre>
    </div>
  );
}
