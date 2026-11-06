import { useCallback, useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";

export type HdrBoostStatus = {
  phase: string;
  device_name: string | null;
  available_device: string | null;
  strength: number;
  recovery_pending: boolean;
  message: string;
};

export function useHdrBoost(onSessionChange: () => Promise<void>) {
  const [status, setStatus] = useState<HdrBoostStatus | null>(null);
  const phase = useRef<string | null>(null);
  const inFlight = useRef(false);
  const refresh = useCallback(async () => {
    if (!isTauri() || inFlight.current) return;
    inFlight.current = true;
    try {
      const next = await invoke<HdrBoostStatus>("get_hdr_boost_status");
      setStatus(next);
      if (phase.current !== null && phase.current !== next.phase) void onSessionChange();
      phase.current = next.phase;
    } finally {
      inFlight.current = false;
    }
  }, [onSessionChange]);
  useEffect(() => {
    const update = () => { void refresh().catch(console.error); };
    update();
    const timer = window.setInterval(update, 750);
    window.addEventListener("focus", update);
    return () => { window.clearInterval(timer); window.removeEventListener("focus", update); };
  }, [refresh]);
  const locked = status?.recovery_pending || ["starting", "running", "stopping", "restoring"].includes(status?.phase ?? "");
  return { status, refresh, locked: Boolean(locked) };
}
