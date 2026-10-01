import { createContext, useContext, useEffect, useState, type ReactNode } from 'react';

const NowContext = createContext<number>(Date.now());

/** Shared clock for relative times ("vor 5 Minuten"), updated every 30 s. */
export function NowProvider({ children, intervalMs = 30_000 }: { children: ReactNode; intervalMs?: number }) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), intervalMs);
    return () => window.clearInterval(id);
  }, [intervalMs]);
  return <NowContext.Provider value={now}>{children}</NowContext.Provider>;
}

/** Current time in milliseconds. */
export function useNow(): number {
  return useContext(NowContext);
}
