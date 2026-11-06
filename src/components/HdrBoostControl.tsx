import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { HdrBoostStatus } from "../hooks/useHdrBoost";
import { HdrIcon } from "./ui/icons/HdrIcon";

type Props = {
  deviceName: string;
  status: HdrBoostStatus;
  refresh: () => Promise<void>;
  disabled?: boolean;
  onError?: (message: string) => void;
};

export function HdrBoostControl({ deviceName, status, refresh, disabled, onError }: Props) {
  const [busy, setBusy] = useState(false);
  const [strength, setStrength] = useState(status.strength);
  const active = ["starting", "running", "stopping"].includes(status.phase);
  const restoring = status.phase === "restoring";
  useEffect(() => { setStrength(status.strength); }, [status.strength]);
  const command = async (name: string, args?: Record<string, unknown>) => {
    if (busy) return;
    setBusy(true);
    try { await invoke(name, args); await refresh(); }
    catch (error) { onError?.(String(error)); }
    finally { setBusy(false); }
  };

  return (
    <section className="settings-row settings-row-stacked hdr-boost-control" aria-labelledby="hdr-boost-heading">
      <div className="settings-row-heading">
        <span className="control-icon hdr-control-icon" aria-hidden="true"><HdrIcon on={active} /></span>
        <span className="control-title-group">
          <strong id="hdr-boost-heading">HDR Boost</strong>
          <span role="status" title={status.message}>{status.message}</span>
        </span>
        <button type="button" className={`switch-toggle ${active ? "active" : ""}`}
          role="switch" aria-label="HDR Boost" aria-checked={active}
          disabled={busy || restoring || (!active && (disabled || status.recovery_pending))}
          onClick={() => void command(active ? "stop_hdr_boost" : "start_hdr_boost",
            active ? undefined : { deviceName, strength })}>
          <span className="switch-toggle-thumb" aria-hidden="true" />
        </button>
      </div>
      <details className="boost-strength-disclosure">
        <summary>
          <span>Adjust strength</span>
          <output>{strength.toFixed(2)}×</output>
        </summary>
        <div className="boost-strength">
          <label className="visually-hidden" htmlFor="hdr-boost-strength">HDR Boost strength</label>
          <input id="hdr-boost-strength" type="range" min={1} max={2.5} step={0.1} value={strength}
            disabled={busy || restoring || status.recovery_pending || (disabled && !active)}
            aria-valuetext={`${strength.toFixed(2)} times linear light`}
            onChange={(event) => setStrength(Number(event.target.value))}
            onPointerUp={(event) => void command("set_hdr_boost_strength", { strength: Number(event.currentTarget.value) })}
            onKeyUp={(event) => void command("set_hdr_boost_strength", { strength: Number(event.currentTarget.value) })}
            onBlur={() => { if (strength !== status.strength) void command("set_hdr_boost_strength", { strength }); }} />
          <div className="boost-strength-limits" aria-hidden="true"><span>1×</span><span>2.5×</span></div>
          <span>Boost uses HDR at 60 Hz. Turning it off restores your previous HDR and refresh rate. Ctrl+Alt+B also stops and restores.</span>
        </div>
      </details>
      {status.recovery_pending && !restoring && (
        <button className="button" type="button" disabled={busy}
          onClick={() => void command("restore_hdr_boost")}>Restore saved display settings</button>
      )}
    </section>
  );
}
