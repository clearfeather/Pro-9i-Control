import type { ThemeMode } from "../hooks/useSettings";

type ThemeSelectorProps = {
  value: ThemeMode;
  onChange: (value: "light" | "dark") => void;
};

const options: Array<{ value: "light" | "dark"; label: string }> = [
  { value: "light", label: "Set Windows and Pro 9i Control to light" },
  { value: "dark", label: "Set Windows and Pro 9i Control to dark" },
];

function ThemeIcon({ mode }: { mode: "light" | "dark" }) {
  if (mode === "light") {
    return (
      <svg viewBox="0 0 20 20" aria-hidden="true">
        <circle cx="10" cy="10" r="3.25" />
        <path d="M10 2.1v1.6M10 16.3v1.6M2.1 10h1.6M16.3 10h1.6M4.4 4.4l1.15 1.15M14.45 14.45l1.15 1.15M15.6 4.4l-1.15 1.15M5.55 14.45 4.4 15.6" />
      </svg>
    );
  }

  return (
    <svg viewBox="0 0 20 20" aria-hidden="true">
      <path d="M16.2 12.4A6.5 6.5 0 0 1 7.6 3.8a6.5 6.5 0 1 0 8.6 8.6Z" />
    </svg>
  );
}

export function ThemeSelector({ value, onChange }: ThemeSelectorProps) {
  return (
    <div className="theme-selector" role="radiogroup" aria-label="Color theme">
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          className={`theme-option ${value === option.value ? "active" : ""}`}
          role="radio"
          aria-checked={value === option.value}
          aria-label={option.label}
          title={option.label}
          onClick={() => onChange(option.value)}
        >
          <ThemeIcon mode={option.value} />
        </button>
      ))}
    </div>
  );
}
