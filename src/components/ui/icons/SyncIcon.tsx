export function SyncIcon({ paused = false }: { paused?: boolean }) {
  return (
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.7"
      strokeLinecap="round" strokeLinejoin="round" aria-hidden="true">
      <path d="M20 8a8 8 0 0 0-13-3L4 8m0-5v5h5M4 16a8 8 0 0 0 13 3l3-3m0 5v-5h-5" />
      {paused && <path d="M10 10v4m4-4v4" />}
    </svg>
  );
}
