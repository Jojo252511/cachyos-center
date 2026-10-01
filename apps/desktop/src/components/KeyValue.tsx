import type { ReactNode } from 'react';

export function KeyValueList({ children, className }: { children: ReactNode; className?: string }) {
  return <dl className={`kv ${className ?? ''}`}>{children}</dl>;
}

export function KeyValue({ label, children }: { label: ReactNode; children: ReactNode }) {
  return (
    <div className="kv__row">
      <dt className="kv__key">{label}</dt>
      <dd className="kv__value">{children}</dd>
    </div>
  );
}
