import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { DisplayInfo } from "../lib/Resolutions";
import { HdrIcon } from "./ui/icons/HdrIcon";
import { useMonitorsMutation } from "../hooks/useMonitorsMutation";

type HdrToggleProps = {
  monitor: DisplayInfo;
  disabled?: boolean;
  onError?: (msg: string) => void;
};

export function HdrToggle({ monitor, disabled, onError }: HdrToggleProps) {
  const [busy, setBusy] = useState(false);
  const { mutation } = useMonitorsMutation();
  const unsupported = monitor.hdr_status === "unsupported";
  const hdrOn = monitor.hdr_status === "on";

  const setHdr = async (enable: boolean) => {
    if (busy || disabled || unsupported || enable === hdrOn) return;

    try {
      setBusy(true);
      await mutation(() =>
        invoke("enable_hdr", {
          deviceName: monitor.device_name,
          enable,
        })
      );
    } catch (error) {
      onError?.((error as Error).message ?? String(error));
    } finally {
      setBusy(false);
    }
  };

  const description = unsupported
    ? "HDR is not available on this display."
    : hdrOn
    ? "High dynamic range is active."
    : "Standard dynamic range is active.";

  return (
    <section
      className={`settings-row settings-row-stacked ${unsupported ? "settings-row-disabled" : ""}`}
      aria-labelledby="dynamic-range-heading"
    >
      <div className="settings-row-heading">
        <span className="control-icon hdr-control-icon" aria-hidden="true">
          <HdrIcon on={hdrOn} />
        </span>
        <span className="control-title-group">
          <strong id="dynamic-range-heading">Dynamic range</strong>
          <span>{description}</span>
        </span>
      </div>

      <div className="range-toggle" role="group" aria-label="Dynamic range">
        <button
          type="button"
          className={!hdrOn && !unsupported ? "active" : ""}
          aria-pressed={!hdrOn && !unsupported}
          disabled={disabled || busy || unsupported}
          onClick={() => void setHdr(false)}
        >
          SDR
        </button>
        <button
          type="button"
          className={hdrOn ? "active hdr-active" : ""}
          aria-pressed={hdrOn}
          disabled={disabled || busy || unsupported}
          onClick={() => void setHdr(true)}
        >
          HDR
        </button>
      </div>
    </section>
  );
}
