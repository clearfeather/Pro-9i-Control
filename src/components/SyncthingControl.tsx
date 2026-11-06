import { useCallback, useEffect, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { SyncIcon } from "./ui/icons/SyncIcon";
import { BatteryCheckbox } from "./ui/BatteryCheckbox";

type SyncthingStatus = {
  supported: boolean;
  available: boolean;
  paused: boolean;
  autoPauseOnBattery: boolean;
  onBattery: boolean | null;
  batteryOverride: boolean;
  managedFolders: number;
  totalFolders: number;
  pausedFolders: number;
  message: string;
};

type Props = { onError?: (message: string) => void };

export function SyncthingControl({ onError }: Props) {
  const [status, setStatus] = useState<SyncthingStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const refresh = useCallback(async () => {
    if (!isTauri()) return;
    setStatus(await invoke<SyncthingStatus>("get_syncthing_status"));
  }, []);

  useEffect(() => {
    if (!isTauri()) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    const update = () => { void refresh().catch(console.error); };
    void listen<SyncthingStatus>("syncthing-status-changed", ({ payload }) => {
      if (!disposed) setStatus(payload);
    }).then((stop) => {
      if (disposed) stop();
      else { unlisten = stop; update(); }
    }).catch((error) => {
      console.error(error);
      if (!disposed) update();
    });
    window.addEventListener("focus", update);
    return () => {
      disposed = true;
      unlisten?.();
      window.removeEventListener("focus", update);
    };
  }, [refresh]);

  const command = async (name: string, args: Record<string, boolean>) => {
    if (busy || !status?.supported) return;
    setBusy(true);
    try {
      setStatus(await invoke<SyncthingStatus>(name, args));
    } catch (error) {
      onError?.(error instanceof Error ? error.message : String(error));
      await refresh().catch(console.error);
    } finally {
      setBusy(false);
    }
  };

  const paused = status?.paused ?? false;
  const disabled = !isTauri() || !status?.supported || busy;
  const message = !isTauri()
    ? "Syncthing controls are available in the desktop app."
    : !status
    ? "Checking Syncthing…"
    : !status.available && paused
    ? `Pause pending. ${status.message}`
    : status.message;

  return (
    <section className="settings-row settings-row-stacked syncthing-control"
      aria-labelledby="syncthing-heading" aria-busy={busy}>
      <div className="settings-row-heading">
        <span className="control-icon" aria-hidden="true"><SyncIcon paused={paused} /></span>
        <span className="control-title-group">
          <strong id="syncthing-heading">Pause Syncthing</strong>
          <span role="status" title={message}>{message}</span>
        </span>
        <button type="button" className={`switch-toggle ${paused ? "active" : ""}`}
          role="switch" aria-label="Pause Syncthing" aria-checked={paused}
          title="Pause folder scanning and syncing" disabled={disabled}
          onClick={() => void command("set_syncthing_paused", { paused: !paused })}>
          <span className="visually-hidden">{paused ? "Paused" : "Running"}</span>
          <span className="switch-toggle-thumb" aria-hidden="true" />
        </button>
      </div>
      <BatteryCheckbox label="Pause on battery · resume when plugged in"
        checked={status?.autoPauseOnBattery ?? true} disabled={disabled}
        onChange={(enabled) => void command("set_syncthing_auto_pause", { enabled })} />
    </section>
  );
}
