export function UsbChargeIcon({ active }: { active: boolean }) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
    >
      <path
        d="M8.2 4.4v5.1m7.6-5.1v5.1M7 4.4h2.4m5.4 0h2.4M6.7 9.5h10.6v3.1A5.3 5.3 0 0 1 12 17.9a5.3 5.3 0 0 1-5.3-5.3V9.5Z"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path
        d="M12 17.9v2.2"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinecap="round"
      />
      <path
        d="m12.8 10.9-2.3 3h1.7l-.7 2.1 2.3-3.1h-1.6l.6-2Z"
        fill={active ? "currentColor" : "none"}
        stroke="currentColor"
        strokeWidth="0.8"
        strokeLinejoin="round"
      />
    </svg>
  );
}
