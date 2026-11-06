type BatteryCareIconProps = {
  active?: boolean;
};

export function BatteryCareIcon({ active = false }: BatteryCareIconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
    >
      <rect
        x="3"
        y="6.5"
        width="16"
        height="11"
        rx="2.5"
        stroke="currentColor"
        strokeWidth="1.8"
      />
      <path
        d="M19 10h1.2c.99 0 1.8.81 1.8 1.8v.4c0 .99-.81 1.8-1.8 1.8H19"
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
      />
      <path
        d="M8.4 14.9c2.45-.08 4.13-1.42 4.7-4.1-2.7-.18-4.5 1.14-4.7 4.1Z"
        fill={active ? "currentColor" : "none"}
        stroke="currentColor"
        strokeWidth="1.45"
        strokeLinejoin="round"
      />
      <path
        d="M7.4 16.2c.55-1.42 1.5-2.45 2.86-3.1"
        stroke="currentColor"
        strokeWidth="1.45"
        strokeLinecap="round"
      />
    </svg>
  );
}
