import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { PowerModeIcon } from "./ui/icons/PowerModeIcon";
import { BatteryCheckbox } from "./ui/BatteryCheckbox";
import { BatteryControls, batteryControlMessage } from "../hooks/useBatteryControls";

type PowerMode = "auto" | "cool" | "performance";

type PowerModeStatus = {
  supported: boolean;
  mode: PowerMode | "unknown" | "unsupported";
  availableModes: PowerMode[];
  message?: string | null;
};

type PowerModeControlProps = {
  onError?: (message: string) => void;
  batteryControls?: BatteryControls;
};

const options: { mode: PowerMode; label: string; ariaLabel: string }[] = [
  { mode: "cool", label: "Power Save", ariaLabel: "Power Save power mode" },
  { mode: "auto", label: "Balanced", ariaLabel: "Balanced power mode" },
  { mode: "performance", label: "Perf.", ariaLabel: "Performance power mode" },
];

const descriptions: Record<PowerMode, string> = {
  auto: "Balances performance, battery life, and fan speed.",
  cool: "Prioritizes battery life with cooler, quieter operation.",
  performance: "Prioritizes speed and may use higher fan speeds.",
};

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

export function PowerModeControl({ onError, batteryControls }: PowerModeControlProps) {
  const [status, setStatus] = useState<PowerModeStatus | null>(null);
  const [busy, setBusy] = useState<PowerMode | null>(null);

  const refresh = useCallback(async () => {
    try {
      setStatus(await invoke<PowerModeStatus>("get_lenovo_power_mode"));
    } catch (error) {
      setStatus({
        supported: false,
        mode: "unsupported",
        availableModes: [],
        message: errorMessage(error),
      });
    }
  }, []);

  useEffect(() => {
    void refresh();

    const onFocus = () => void refresh();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [refresh]);

  useEffect(() => {
    if (batteryControls?.status) void refresh();
  }, [batteryControls?.status?.powerMode, refresh]);

  const selectMode = async (mode: PowerMode) => {
    if (!status?.supported || busy || mode === status.mode) return;

    try {
      setBusy(mode);
      setStatus(
        await invoke<PowerModeStatus>("set_lenovo_power_mode", { mode })
      );
    } catch (error) {
      onError?.(errorMessage(error));
      await refresh();
    } finally {
      setBusy(null);
    }
  };

  const loading = status === null;
  const unsupported = status?.supported === false;
  const description = loading
    ? "Checking Lenovo power controls…"
    : unsupported
    ? status.message ?? "Lenovo power controls are unavailable."
    : status.mode in descriptions
    ? descriptions[status.mode as PowerMode]
    : "Choose how this laptop balances performance and battery life.";

  return (
    <section
      className={`settings-row settings-row-stacked power-mode-row ${
        unsupported ? "settings-row-disabled" : ""
      }`}
      aria-labelledby="power-mode-heading"
    >
      <div className="settings-row-heading">
        <span className="control-icon power-mode-icon" aria-hidden="true">
          <PowerModeIcon />
        </span>
        <span className="control-title-group">
          <strong id="power-mode-heading">Power mode</strong>
          <span>{description}</span>
        </span>
      </div>

      <div className="power-mode-options" role="radiogroup" aria-label="Power mode">
        {options.map((option) => {
          const active = status?.mode === option.mode;
          const available = status?.availableModes.includes(option.mode) ?? false;
          return (
            <button
              key={option.mode}
              type="button"
              className={`power-mode-option ${active ? "active" : ""}`}
              role="radio"
              aria-label={option.ariaLabel}
              aria-checked={active}
              disabled={loading || unsupported || busy !== null || !available}
              onClick={() => void selectMode(option.mode)}
            >
              {busy === option.mode ? "…" : option.label}
            </button>
          );
        })}
      </div>
      {batteryControls && (
        <>
        <BatteryCheckbox label="Power Save on battery · restore when plugged in"
          checked={batteryControls.status?.powerMode.enabled ?? true}
          disabled={!batteryControls.status?.supported || batteryControls.busy !== null || busy !== null}
          message={batteryControlMessage(batteryControls.status?.powerMode)}
          onChange={(enabled) => void batteryControls.setEnabled("powerMode", enabled)} />
        <BatteryCheckbox label="Keep brightness when switching modes"
          checked={batteryControls.status?.keepBrightness ?? true}
          disabled={!batteryControls.status?.supported || batteryControls.busy !== null || busy !== null}
          message={batteryControls.status?.brightnessMessage ?? undefined}
          onChange={(enabled) => void batteryControls.setEnabled("keepBrightness", enabled)} />
        </>
      )}
    </section>
  );
}
