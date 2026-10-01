import { FlaskConical } from 'lucide-react';
import { useId } from 'react';

import { useI18n } from '../../i18n';
import { useAppState } from '../../state/AppState';

/** Visible marker for the browser demo backend with a scenario switch for visual QA. */
export function DemoBanner() {
  const { t } = useI18n();
  const { backend } = useAppState();
  const id = useId();
  if (backend.kind !== 'mock') return null;
  const change = (scenario: string) => {
    const url = new URL(window.location.href);
    url.searchParams.set('scenario', scenario);
    window.location.assign(url.toString());
  };
  return (
    <div className="demo-banner" role="note">
      <FlaskConical aria-hidden="true" className="demo-banner__icon" />
      <span>{t('demo.banner')}</span>
      <label htmlFor={id} className="demo-banner__label">
        {t('demo.scenario')}
      </label>
      <select id={id} className="select select--small" value={backend.scenario ?? 'default'} onChange={(event) => change(event.target.value)}>
        {backend.scenarios.map((scenario) => (
          <option key={scenario} value={scenario}>
            {scenario}
          </option>
        ))}
      </select>
    </div>
  );
}
