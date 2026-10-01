import { render, type RenderResult } from '@testing-library/react';
import type { ReactElement, ReactNode } from 'react';

import { overrideTransport } from '../api/client';
import { createMockTransport, type MockTransport, type ScenarioId } from '../api/mock';
import { I18nProvider } from '../i18n';
import { AppStateProvider } from '../state/AppState';
import { NowProvider } from '../state/now';
import { OperationsProvider } from '../state/Operations';
import { StatusProvider } from '../state/Status';

/**
 * Uses the browser mock (without artificial latency) as backend of the API
 * client. Individual commands are overridden with `vi.spyOn(api, …)`.
 */
export function mockBackend(scenario: ScenarioId = 'default'): MockTransport {
  const transport = createMockTransport(`?scenario=${scenario}`, { latency: false });
  overrideTransport(transport);
  return transport;
}

/** Restores the runtime transport detection after a test. */
export function resetBackend(): void {
  overrideTransport(null);
}

export function AllProviders({ children }: { children: ReactNode }) {
  return (
    <AppStateProvider>
      <NowProvider>
        <StatusProvider>
          <OperationsProvider>{children}</OperationsProvider>
        </StatusProvider>
      </NowProvider>
    </AppStateProvider>
  );
}

/** Renders with all application providers (settings, status, operations). */
export function renderApp(ui: ReactElement): RenderResult {
  return render(<AllProviders>{ui}</AllProviders>);
}

/** Renders a presentational component with the German dictionary only. */
export function renderI18n(ui: ReactElement, lang: 'de' | 'en' = 'de'): RenderResult {
  return render(
    <I18nProvider lang={lang}>
      <NowProvider>{ui}</NowProvider>
    </I18nProvider>,
  );
}
