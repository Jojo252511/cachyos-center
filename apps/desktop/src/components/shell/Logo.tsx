/** Neutral app mark (inline SVG, no external resources). */
export function Logo() {
  return (
    <svg className="logo" viewBox="0 0 32 32" aria-hidden="true" focusable="false">
      <rect x="1" y="1" width="30" height="30" rx="8" className="logo__bg" />
      <path d="M21.5 10.5a8 8 0 1 0 0 11" className="logo__arc" fill="none" strokeWidth="3" strokeLinecap="round" />
      <circle cx="22.5" cy="16" r="2.25" className="logo__dot" />
    </svg>
  );
}
