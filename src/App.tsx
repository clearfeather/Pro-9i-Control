import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import "./App.css";
import hzhdrIcon from "./assets/hzhdr-icon-v2.svg";
import { MonitorControls } from "./components/MonitorControls";
import { ConservationModeToggle } from "./components/ConservationModeToggle";
import { PowerModeControl } from "./components/PowerModeControl";
import { AlwaysOnUsbToggle } from "./components/AlwaysOnUsbToggle";
import { SyncthingControl } from "./components/SyncthingControl";
import { Selector } from "./components/ui/Selector/Selector";
import { ErrorToast } from "./components/ui/ErrorToast/ErrorToast";
import { MonitorIcon } from "./components/ui/icons/MonitorIcon";
import { useMonitorsContext } from "./context/MonitorsContext";
import { ThemeSelector } from "./components/ThemeSelector";
import { useSettings } from "./hooks/useSettings";
import { useBatteryControls } from "./hooks/useBatteryControls";
import { Settings } from "./components/Settings/Settings";
import { CogIcon } from "./components/ui/icons/CogIcon";
import { CONTROL_GROUPS } from "./lib/interfaceControls";
import { HdrBoostControl } from "./components/HdrBoostControl";

function getSystemTheme(): "light" | "dark" {
  if (
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-color-scheme: dark)").matches
  ) {
    return "dark";
  }

  return "light";
}

function App() {
  const appRootRef = useRef<HTMLElement>(null);
  const settingsButtonRef = useRef<HTMLButtonElement>(null);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const { monitors, loading, refreshing, error, setError, reloadMonitors, boost } =
    useMonitorsContext();
  const batteryControls = useBatteryControls(setError);

  useEffect(() => {
    if (batteryControls.status) void reloadMonitors();
  }, [batteryControls.status?.refreshRate.active, batteryControls.status?.refreshRate.enabled, reloadMonitors]);
  const {
    settings,
    updateSettings,
    loading: settingsLoading,
  } = useSettings();
  const visible = settings.visibleControls;
  const showDisplay = CONTROL_GROUPS[0].controls.some((control) => visible[control.id]);
  const showLaptop = CONTROL_GROUPS[1].controls.some((control) => visible[control.id]);
  const closeSettings = useCallback(() => {
    setSettingsOpen(false);
    window.requestAnimationFrame(() => {
      window.scrollTo(0, 0);
      settingsButtonRef.current?.focus();
    });
  }, []);
  const [selectedDeviceName, setSelectedDeviceName] = useState<string | null>(
    null
  );

  useEffect(() => {
    setSelectedDeviceName((current) => {
      if (monitors.length === 0) return null;

      const stillExists = monitors.some((m) => m.device_name === current);
      return stillExists ? current : monitors[0].device_name;
    });
  }, [monitors]);

  const selectedMonitor = useMemo(
    () => monitors.find((m) => m.device_name === selectedDeviceName) ?? null,
    [monitors, selectedDeviceName]
  );

  const changeTheme = async (theme: "light" | "dark") => {
    try {
      if (isTauri()) {
        await invoke("set_system_theme", { theme });
      }
      await updateSettings({ theme });
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    }
  };

  useEffect(() => {
    if (settingsLoading) return;

    let cancelled = false;
    const synchronizeWithWindows = async () => {
      let theme: "light" | "dark" = getSystemTheme();

      if (isTauri()) {
        try {
          theme = await invoke<"light" | "dark">("get_system_theme");
        } catch {
          // matchMedia is a safe fallback if the registry value is unavailable.
        }
      }

      if (!cancelled) {
        document.documentElement.dataset.theme = theme;
        await updateSettings({ theme }).catch((cause) => {
          setError(cause instanceof Error ? cause.message : String(cause));
        });
      }
    };

    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const handleWindowsThemeChange = () => void synchronizeWithWindows();
    void synchronizeWithWindows();
    media.addEventListener("change", handleWindowsThemeChange);

    return () => {
      cancelled = true;
      media.removeEventListener("change", handleWindowsThemeChange);
    };
  }, [settingsLoading, updateSettings]);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: dark)");
    const applyTheme = () => {
      document.documentElement.dataset.theme =
        settings.theme === "system" ? getSystemTheme() : settings.theme;
    };

    applyTheme();
    media.addEventListener("change", applyTheme);
    return () => media.removeEventListener("change", applyTheme);
  }, [settings.theme]);

  useLayoutEffect(() => {
    const appRoot = appRootRef.current;
    if (!appRoot || !isTauri()) return;

    let animationFrame = 0;
    let lastHeight = 0;

    const fitWindowToContent = () => {
      window.cancelAnimationFrame(animationFrame);
      animationFrame = window.requestAnimationFrame(() => {
        const height = Math.ceil(appRoot.getBoundingClientRect().height);
        if (height <= 0 || height === lastHeight) return;

        lastHeight = height;
        void getCurrentWindow().setSize(new LogicalSize(360, height));
      });
    };

    const observer = new ResizeObserver(fitWindowToContent);
    observer.observe(appRoot);
    fitWindowToContent();

    return () => {
      observer.disconnect();
      window.cancelAnimationFrame(animationFrame);
    };
  }, []);

  return (
    <main className="app-root" ref={appRootRef}>
      {error && <ErrorToast message={error} onClose={() => setError(null)} />}

      <header className="app-toolbar">
        <img className="brand-mark" src={hzhdrIcon} alt="" aria-hidden="true" />
        <div className="brand-copy">
          <span className="brand-name">Pro 9i Control</span>
          <span className="brand-tagline">Display, power & battery</span>
        </div>
        {(loading || refreshing) && (
          <span className="sync-indicator" aria-label="Refreshing displays" />
        )}
        <ThemeSelector
          value={settings.theme}
          onChange={(theme) => void changeTheme(theme)}
        />
        <button ref={settingsButtonRef} type="button" className={`toolbar-settings ${settingsOpen ? "active" : ""}`}
          aria-label={settingsOpen ? "Close settings" : "Customize interface"} title="Customize interface"
          aria-expanded={settingsOpen} aria-controls="interface-settings" disabled={settingsLoading}
          onClick={() => {
            if (settingsOpen) closeSettings();
            else { setSettingsOpen(true); window.scrollTo(0, 0); }
          }}>
          <CogIcon size={17} />
        </button>
      </header>

      {settingsOpen && (
        <div id="interface-settings"><Settings onClose={closeSettings} onError={setError} /></div>
      )}
      {settingsLoading && <p className="panel-empty-note" role="status">Loading preferences…</p>}
      <div className="main-controls" hidden={settingsOpen || settingsLoading}>
      {boost.locked && boost.status && (!visible.hdrBoost || !showDisplay || selectedMonitor?.device_name !== boost.status.available_device) && (
        <HdrBoostControl deviceName={boost.status.device_name ?? boost.status.available_device ?? ""}
          status={boost.status} refresh={boost.refresh} onError={setError} />
      )}
      {showDisplay && monitors.length > 1 && (
        <div className="monitor-selector-container">
          <Selector
            ariaLabel="Select display"
            items={monitors}
            selectedItem={selectedMonitor}
            onChange={(monitor) =>
              setSelectedDeviceName(monitor.device_name)
            }
            getKey={(monitor) => monitor.device_name}
            getLabel={(monitor) => `Display ${monitors.indexOf(monitor) + 1}`}
            disabled={loading}
          />
        </div>
      )}

      {showDisplay && <section className="settings-section" aria-labelledby="display-section-heading">
        <h2 className="section-label" id="display-section-heading">Display</h2>
        <div className="settings-group">
          {!loading && monitors.length === 0 && (
            <div className="empty-state">
              <MonitorIcon type="external" manufacturer="" />
              <strong>No displays found</strong>
              <span>Connect a display and reopen Pro 9i Control.</span>
            </div>
          )}

          {selectedMonitor && (
            <>
              {visible.displaySummary && <div className="display-summary" aria-label="Selected display">
                <span className="display-icon">
                  <MonitorIcon
                    type={selectedMonitor.built_in ? "laptop" : "external"}
                    manufacturer={selectedMonitor.manufacturer}
                  />
                </span>
                <span className="display-copy">
                  <strong>
                    {selectedMonitor.model || selectedMonitor.display_name}
                  </strong>
                  <span>
                    {selectedMonitor.current.width} × {selectedMonitor.current.height}
                    {selectedMonitor.connection
                      ? ` · ${selectedMonitor.connection}`
                      : ""}
                  </span>
                </span>
                {selectedMonitor.is_primary && (
                  <span className="primary-badge">Primary</span>
                )}
              </div>}

              <MonitorControls
                key={selectedMonitor.device_name}
                monitor={selectedMonitor}
                disabled={loading || refreshing}
                onError={setError}
                batteryControls={batteryControls}
                visibleControls={visible}
              />
            </>
          )}
        </div>
      </section>}

      {showLaptop && <section className="settings-section" aria-labelledby="laptop-section-heading">
        <h2 className="section-label" id="laptop-section-heading">Laptop</h2>
        <div className="settings-group">
          {visible.powerMode && <PowerModeControl onError={setError} batteryControls={batteryControls} />}
          {visible.chargeLimit && <ConservationModeToggle onError={setError} />}
          {visible.alwaysOnUsb && <AlwaysOnUsbToggle onError={setError} />}
          {visible.syncthing && <SyncthingControl onError={setError} />}
        </div>
      </section>}
      {!showDisplay && !showLaptop && (
        <div className="panel-empty-note">
          <strong>Your panel is clear</strong>
          <p>Choose the controls you want to see in settings.</p>
          <button type="button" className="button" onClick={() => setSettingsOpen(true)}>Choose controls</button>
        </div>
      )}
      </div>
    </main>
  );
}

export default App;
