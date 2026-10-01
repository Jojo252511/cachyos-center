import { AppShell } from './components/shell/AppShell';
import { AppStateProvider } from './state/AppState';
import { NowProvider } from './state/now';
import { OperationsProvider } from './state/Operations';
import { StatusProvider } from './state/Status';

/** Provider order: settings/language → clock → system status → package operation. */
export function App() {
  return (
    <AppStateProvider>
      <NowProvider>
        <StatusProvider>
          <OperationsProvider>
            <AppShell />
          </OperationsProvider>
        </StatusProvider>
      </NowProvider>
    </AppStateProvider>
  );
}
