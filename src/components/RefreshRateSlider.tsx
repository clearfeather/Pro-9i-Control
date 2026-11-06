import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Resolution } from "../lib/Resolutions";
import { useMonitorsMutation } from "../hooks/useMonitorsMutation";
import { RefreshRateIcon } from "./ui/icons/RefreshRateIcon";
import { BatteryCheckbox } from "./ui/BatteryCheckbox";
import { BatteryControls, batteryControlMessage } from "../hooks/useBatteryControls";

type Props = {
  modes: Resolution[];
  current: Resolution | null;
  disabled?: boolean;
  deviceName: string | null;
  onError?: (msg: string) => void;
  builtIn?: boolean;
  batteryControls?: BatteryControls;
};

export function RefreshRateSlider({
  modes,
  current,
  disabled = false,
  deviceName,
  onError,
  builtIn = false,
  batteryControls,
}: Props) {
  const { mutation } = useMonitorsMutation();
  const [selectedHz, setSelectedHz] = useState(current?.refresh_hz ?? 0);
  const [busy, setBusy] = useState(false);

  const availableHz = useMemo(() => {
    if (!current) return [] as number[];

    return Array.from(
      new Set(
        modes
          .filter(
            (mode) =>
              mode.width === current.width && mode.height === current.height
          )
          .map((mode) => mode.refresh_hz)
      )
    ).sort((a, b) => a - b);
  }, [modes, current]);

  useEffect(() => {
    setSelectedHz(current?.refresh_hz ?? 0);
  }, [current?.refresh_hz, deviceName]);

  const applyRefreshRate = async (nextHz: number) => {
    if (
      busy ||
      disabled ||
      !deviceName ||
      !current ||
      nextHz === selectedHz
    ) {
      return;
    }

    const previousHz = selectedHz;
    setSelectedHz(nextHz);
    setBusy(true);

    try {
      await mutation(() =>
        invoke("set_monitor_resolution", {
          deviceName,
          width: current.width,
          height: current.height,
          refreshHz: nextHz,
        })
      );
    } catch (error) {
      setSelectedHz(previousHz);
      onError?.((error as Error).message ?? String(error));
    } finally {
      setBusy(false);
    }
  };

  return (
    <section className="settings-row settings-row-stacked" aria-labelledby="refresh-rate-heading">
      <div className="settings-row-heading">
        <span className="control-icon" aria-hidden="true">
          <RefreshRateIcon />
        </span>
        <span className="control-title-group">
          <strong id="refresh-rate-heading">Refresh rate</strong>
          <span>Choose how smoothly this display updates.</span>
        </span>
        <span className="current-value" aria-label={`${selectedHz} hertz`}>
          <strong>{selectedHz || "—"}</strong>
          <span>Hz</span>
        </span>
      </div>

      <div className="rate-options" aria-label="Available refresh rates">
        {availableHz.map((hz) => {
          const active = hz === selectedHz;
          return (
            <button
              key={hz}
              type="button"
              className={`rate-option ${active ? "active" : ""}`}
              aria-pressed={active}
              disabled={disabled || busy}
              onClick={() => void applyRefreshRate(hz)}
            >
              {hz}
              <span>Hz</span>
            </button>
          );
        })}

        {availableHz.length === 0 && (
          <span className="control-unavailable">No refresh rates available</span>
        )}
      </div>
      {builtIn && batteryControls && (
        <BatteryCheckbox label="60 Hz on battery · restore when plugged in"
          checked={batteryControls.status?.refreshRate.enabled ?? true}
          disabled={!batteryControls.status?.supported || batteryControls.busy !== null || busy}
          message={batteryControlMessage(batteryControls.status?.refreshRate)}
          onChange={(enabled) => void batteryControls.setEnabled("refreshRate", enabled)} />
      )}
    </section>
  );
}
