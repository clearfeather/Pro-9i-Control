import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { DisplayInfo } from "../lib/Resolutions";
import { useHdrBoost } from "../hooks/useHdrBoost";

type MonitorsContextValue = {
  boost: ReturnType<typeof useHdrBoost>;
  monitors: DisplayInfo[];
  loading: boolean;
  refreshing: boolean;
  error: string | null;
  setError: (msg: string | null) => void;
  reloadMonitors: () => Promise<void>;
};

const MonitorsContext = createContext<MonitorsContextValue | undefined>(
  undefined
);

const DEV_MONITORS: DisplayInfo[] = [
  {
    device_name: "DEV-DISPLAY-1",
    friendly_name: "LG UltraGear",
    display_name: "Display 1 (Primary)",
    is_primary: true,
    position_x: 0,
    position_y: 0,
    orientation: 0,
    current: {
      width: 2560,
      height: 1440,
      bits_per_pixel: 32,
      refresh_hz: 165,
    },
    modes: [60, 100, 120, 144, 165, 240].map((refresh_hz) => ({
      width: 2560,
      height: 1440,
      bits_per_pixel: 32,
      refresh_hz,
    })),
    max_native: {
      width: 2560,
      height: 1440,
      bits_per_pixel: 32,
      refresh_hz: 240,
    },
    model: "LG UltraGear 27GR95QE",
    serial: "",
    manufacturer: "LG",
    year_of_manufacture: 2024,
    week_of_manufacture: 1,
    connection: "DisplayPort",
    built_in: false,
    active: true,
    enabled: true,
    scale: 1,
    scales: [],
    hdr_status: "off",
    supports_input_switch: false,
  },
];

export function MonitorsProvider({ children }: { children: React.ReactNode }) {
  const [monitors, setMonitors] = useState<DisplayInfo[]>([]);
  const [initialLoading, setInitialLoading] = useState<boolean>(true);
  const [refreshing, setRefreshing] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);
  const reloadPromiseRef = useRef<Promise<void> | null>(null);
  const reloadQueuedRef = useRef(false);

  const reloadMonitors = useCallback(async (): Promise<void> => {
    if (reloadPromiseRef.current) {
      // A boost transition may arrive while an older HDR query is in flight.
      // Follow it with a fresh read so that old state cannot win after restore.
      reloadQueuedRef.current = true;
      return reloadPromiseRef.current;
    }

    const promise = (async () => {
      try {
        setRefreshing(true);
        if (!isTauri()) {
          setMonitors(DEV_MONITORS);
          return;
        }

        const result = await invoke<DisplayInfo[]>("get_all_monitors");
        const displayNameInfo = result?.map((m, idx) => ({
          ...m,
          display_name: `Monitor ${idx + 1}${m.is_primary ? " (Primary)" : ""}`,
        }));
        setMonitors(displayNameInfo ?? []);
      } catch (e) {
        setError((e as Error).message ?? String(e));
      } finally {
        setRefreshing(false);
        setInitialLoading(false);
        reloadPromiseRef.current = null;
        if (reloadQueuedRef.current) {
          reloadQueuedRef.current = false;
          void reloadMonitors();
        }
      }
    })();

    reloadPromiseRef.current = promise;
    return promise;
  }, []);

  useEffect(() => {
    reloadMonitors();
  }, []);

  // Listen for display change events from the backend (Windows system events)
  useEffect(() => {
    if (!isTauri()) return;
    const unlisten = listen("display-changed", () => {
      reloadMonitors();
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [reloadMonitors]);

  const boost = useHdrBoost(reloadMonitors);
  const value = useMemo(
    () => ({
      boost,
      monitors,
      loading: initialLoading,
      refreshing,
      error,
      setError,
      reloadMonitors,
    }),
    [boost, monitors, initialLoading, refreshing, error, reloadMonitors]
  );

  return (
    <MonitorsContext.Provider value={value}>
      {children}
    </MonitorsContext.Provider>
  );
}

export function useMonitorsContext(): MonitorsContextValue {
  const ctx = useContext(MonitorsContext);
  if (!ctx) {
    throw new Error("useMonitorsContext must be used within MonitorsProvider");
  }
  return ctx;
}
