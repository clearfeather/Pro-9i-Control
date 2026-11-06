import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { BatteryCareIcon } from "./ui/icons/BatteryCareIcon";

type ConservationModeStatus = {
  supported: boolean;
  enabled: boolean;
  mode: "normal" | "conservation" | "rapid" | "unsupported";
  message?: string | null;
};

type ConservationModeToggleProps = {
  onError?: (message: string) => void;
};

export function ConservationModeToggle({
  onError,
}: ConservationModeToggleProps) {
  const [status, setStatus] = useState<ConservationModeStatus | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setStatus(await invoke<ConservationModeStatus>("get_conservation_mode"));
    } catch (error) {
      setStatus({
        supported: false,
        enabled: false,
        mode: "unsupported",
        message: (error as Error).message ?? String(error),
      });
    }
  }, []);

  useEffect(() => {
    void refresh();
    const onFocus = () => void refresh();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [refresh]);

  const setEnabled = async (enabled: boolean) => {
    if (!status?.supported || busy || enabled === status.enabled) return;

    try {
      setBusy(true);
      const result = await invoke<ConservationModeStatus>(
        "set_conservation_mode",
        { enabled }
      );
      setStatus(result);
    } catch (error) {
      onError?.((error as Error).message ?? String(error));
    } finally {
      setBusy(false);
    }
  };

  const loading = status === null;
  const unsupported = status?.supported === false;
  const enabled = status?.enabled === true;

  const description = loading
    ? "Checking Lenovo charge controls…"
    : unsupported
    ? status.message ?? "Lenovo charge limiting is unavailable."
    : enabled
    ? "Starts charging at 75% and stops at 80%."
    : status.mode === "rapid"
    ? "Rapid charging is active. Turn this on for a 75–80% limit."
    : "Off — charging is allowed past 80%.";

  return (
    <div
      className={`settings-row ${
        unsupported ? "settings-row-disabled" : ""
      }`}
      aria-labelledby="battery-care-heading"
    >
      <span className="control-icon battery-care-icon" aria-hidden="true">
        <BatteryCareIcon active={enabled} />
      </span>
      <span className="control-title-group">
        <strong id="battery-care-heading">Charge limit</strong>
        <span>{description}</span>
      </span>
      <button
        type="button"
        className={`switch-toggle ${enabled ? "active" : ""}`}
        role="switch"
        aria-label="75 to 80 percent charge limit"
        aria-checked={enabled}
        disabled={loading || unsupported || busy}
        onClick={() => void setEnabled(!enabled)}
      >
        <span className="visually-hidden">
          {busy ? "…" : enabled ? "On" : "Off"}
        </span>
        <span className="switch-toggle-thumb" aria-hidden="true" />
      </button>
    </div>
  );
}
