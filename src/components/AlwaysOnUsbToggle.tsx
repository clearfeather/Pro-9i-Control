import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { UsbChargeIcon } from "./ui/icons/UsbChargeIcon";

type AlwaysOnUsbStatus = {
  supported: boolean;
  enabled: boolean;
  message?: string | null;
};

type AlwaysOnUsbToggleProps = {
  onError?: (message: string) => void;
};

function errorMessage(error: unknown) {
  return error instanceof Error ? error.message : String(error);
}

export function AlwaysOnUsbToggle({ onError }: AlwaysOnUsbToggleProps) {
  const [status, setStatus] = useState<AlwaysOnUsbStatus | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setStatus(await invoke<AlwaysOnUsbStatus>("get_always_on_usb"));
    } catch (error) {
      setStatus({
        supported: false,
        enabled: false,
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

  const setEnabled = async (enabled: boolean) => {
    if (!status?.supported || busy || enabled === status.enabled) return;

    try {
      setBusy(true);
      setStatus(
        await invoke<AlwaysOnUsbStatus>("set_always_on_usb", { enabled })
      );
    } catch (error) {
      onError?.(errorMessage(error));
      await refresh();
    } finally {
      setBusy(false);
    }
  };

  const loading = status === null;
  const unsupported = status?.supported === false;
  const enabled = status?.enabled === true;
  const description = loading
    ? "Checking Lenovo USB controls…"
    : unsupported
    ? status.message ?? "Always on USB is unavailable."
    : enabled
    ? "USB devices can charge while this PC sleeps or is off."
    : "USB charging stops when this PC sleeps or turns off.";

  return (
    <div
      className={`settings-row ${
        unsupported ? "settings-row-disabled" : ""
      }`}
      aria-labelledby="always-on-usb-heading"
    >
      <span className="control-icon usb-charge-icon" aria-hidden="true">
        <UsbChargeIcon active={enabled} />
      </span>
      <span className="control-title-group">
        <strong id="always-on-usb-heading">Always on USB</strong>
        <span>{description}</span>
      </span>
      <button
        type="button"
        className={`switch-toggle ${enabled ? "active" : ""}`}
        role="switch"
        aria-label="Always on USB"
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
