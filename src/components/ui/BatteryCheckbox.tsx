type Props = {
  label: string;
  checked: boolean;
  disabled?: boolean;
  message?: string;
  onChange: (checked: boolean) => void;
};

export function BatteryCheckbox({ label, checked, disabled, message, onChange }: Props) {
  return (
    <div className="battery-option">
      <label className="battery-auto-option">
        <input type="checkbox" checked={checked} disabled={disabled}
          onChange={(event) => onChange(event.target.checked)} />
        <span>{label}</span>
      </label>
      {message && <span className="battery-option-message" role="status">{message}</span>}
    </div>
  );
}
