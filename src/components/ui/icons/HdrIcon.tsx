type HdrIconProps = {
  on?: boolean;
};

export function HdrIcon(_props: HdrIconProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
    >
      <rect
        x="2.5"
        y="4.5"
        width="19"
        height="15"
        rx="3"
        stroke="currentColor"
        strokeWidth="1.6"
      />
      <text
        x="12"
        y="14.3"
        textAnchor="middle"
        fontFamily="Segoe UI, Arial, sans-serif"
        fontSize="6.3"
        fontWeight="700"
        fill="currentColor"
      >
        HDR
      </text>
    </svg>
  );
}
