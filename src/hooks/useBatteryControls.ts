import { useCallback, useEffect, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type BatteryFeature = "refreshRate" | "powerMode" | "keepBrightness";
export type BatteryControlStatus = {
  enabled: boolean;
  active: boolean;
  overridden: boolean;
  message: string | null;
};
export type BatteryStatus = {
  supported: boolean;
  onBattery: boolean | null;
  refreshRate: BatteryControlStatus;
  powerMode: BatteryControlStatus;
  keepBrightness: boolean;
  brightnessMessage: string | null;
};
export type BatteryControls = {
  status: BatteryStatus | null;
  busy: BatteryFeature | null;
  setEnabled: (feature: BatteryFeature, enabled: boolean) => Promise<void>;
};

export function batteryControlMessage(control: BatteryControlStatus | undefined) {
  return control?.message ?? (control?.enabled && control.overridden ? "Using your manual choice until the power source changes." : undefined);
}

export function useBatteryControls(onError: (message: string) => void): BatteryControls {
  const [status, setStatus] = useState<BatteryStatus | null>(null);
  const [busy, setBusy] = useState<BatteryFeature | null>(null);
  const refresh = useCallback(async () => {
    if (isTauri()) setStatus(await invoke<BatteryStatus>("get_battery_controls"));
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const update = () => { void refresh().catch(console.error); };
    void listen<BatteryStatus>("battery-controls-changed", ({ payload }) => {
      if (!disposed) setStatus(payload);
    }).then((stop) => {
      if (disposed) stop();
      else { unlisten = stop; update(); }
    }).catch((error) => {
      console.error(error);
      if (!disposed) update();
    });
    window.addEventListener("focus", update);
    return () => { disposed = true; unlisten?.(); window.removeEventListener("focus", update); };
  }, [refresh]);

  const setEnabled = useCallback(async (feature: BatteryFeature, enabled: boolean) => {
    if (busy || !status?.supported) return;
    setBusy(feature);
    try {
      setStatus(await invoke<BatteryStatus>("set_battery_control", { feature, enabled }));
    } catch (error) {
      onError(error instanceof Error ? error.message : String(error));
      await refresh().catch(console.error);
    } finally { setBusy(null); }
  }, [busy, status?.supported, onError, refresh]);

  return { status, busy, setEnabled };
}
