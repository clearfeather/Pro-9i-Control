import { memo } from "react";
import { DisplayInfo } from "../lib/Resolutions";
import { HdrToggle } from "./HdrToggle";
import { RefreshRateSlider } from "./RefreshRateSlider";
import { HdrBoostControl } from "./HdrBoostControl";
import { useMonitorsContext } from "../context/MonitorsContext";
import { BatteryControls } from "../hooks/useBatteryControls";
import { ControlVisibility } from "../lib/interfaceControls";
import { BrightnessSlider } from "./BrightnessSlider";
import { ResolutionSlider } from "./ResolutionSlider";
import { ScaleSlider } from "./ScaleSlider";
import { OrientationSelector } from "./OrientationTextToggle";
import { InputSourceSelector } from "./InputSourceSelector";
import { PowerToggle } from "./PowerToggle";
import { IdentifyMonitorsButton } from "./IdentifyMonitorsButton";

type MonitorControlsProps = {
  monitor: DisplayInfo;
  disabled?: boolean;
  onError?: (msg: string) => void;
  batteryControls?: BatteryControls;
  visibleControls: ControlVisibility;
};

function MonitorControlsContent({
  monitor,
  disabled = false,
  onError,
  batteryControls,
  visibleControls: visible,
}: MonitorControlsProps) {
  const { boost } = useMonitorsContext();
  return (
    <>
      {visible.refreshRate && <RefreshRateSlider
        modes={monitor.modes}
        current={monitor.current}
        disabled={disabled || boost.locked}
        deviceName={monitor.device_name}
        onError={onError}
        builtIn={monitor.built_in}
        batteryControls={batteryControls}
      />}

      {visible.hdr && <HdrToggle monitor={monitor} disabled={disabled || boost.locked} onError={onError} />}
      {visible.hdrBoost && boost.status?.available_device === monitor.device_name && monitor.built_in && (
        <HdrBoostControl deviceName={monitor.device_name} status={boost.status}
          refresh={boost.refresh} disabled={disabled} onError={onError} />
      )}

      {visible.brightness && (
        <div className="optional-display-control">
          <BrightnessSlider deviceName={monitor.device_name}
            disabled={disabled || monitor.hdr_status === "on"}
            disabledReason={monitor.hdr_status === "on" ? "Brightness adjustment is unavailable while HDR is on." : undefined}
            onError={onError} />
        </div>
      )}
      {visible.resolution && (
        <div className="optional-display-control">
          <ResolutionSlider modes={monitor.modes} current={monitor.current} deviceName={monitor.device_name}
            orientationDegrees={monitor.orientation} maxNative={monitor.max_native} disabled={disabled || boost.locked} />
        </div>
      )}
      {visible.scale && (
        <div className="optional-display-control">
          <ScaleSlider deviceName={monitor.device_name} currentScale={monitor.scale} scales={monitor.scales}
            disabled={disabled || boost.locked} onError={onError} />
        </div>
      )}
      {visible.orientation && (
        <div className="optional-display-control">
          <OrientationSelector deviceName={monitor.device_name} orientation={monitor.orientation}
            disabled={disabled || boost.locked} onError={onError} />
        </div>
      )}
      {visible.inputSource && (
        <div className="optional-display-control">
          <InputSourceSelector deviceName={monitor.device_name}
            disabled={disabled || boost.locked || monitor.built_in || !monitor.supports_input_switch} onError={onError} />
          {(monitor.built_in || !monitor.supports_input_switch) && <p className="optional-control-note">Requires an external display with input switching.</p>}
        </div>
      )}
      {visible.displayPower && (
        <div className="optional-display-control">
          <PowerToggle monitor={monitor} disabled={disabled || boost.locked || monitor.built_in} onError={onError} />
          {monitor.built_in && <p className="optional-control-note">Display power is available for external displays.</p>}
        </div>
      )}
      {visible.identifyDisplays && (
        <div className="optional-display-control">
          <IdentifyMonitorsButton disabled={disabled} onError={onError} />
        </div>
      )}
    </>
  );
}

export const MonitorControls = memo(MonitorControlsContent);
