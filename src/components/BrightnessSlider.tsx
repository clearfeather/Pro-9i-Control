import { useEffect, useState, useCallback, useRef } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Slider } from "./ui/Slider/Slider";
import { BrightnessIcon } from "./ui/icons/BrightnessIcon";

type Props = {
  deviceName: string | null;
  disabled?: boolean;
  disabledReason?: string;
  onError?: (msg: string) => void;
};

export function BrightnessSlider({
  deviceName,
  disabled,
  disabledReason,
  onError,
}: Props) {
  const [pct, setPct] = useState<number | null>(null);
  const [unavailable, setUnavailable] = useState(false);
  const mounted = useRef(true);
  const writing = useRef(false);
  const pending = useRef<number | null>(null);
  const inFlight = useRef<number | null>(null);
  const writeVersion = useRef(0);
  const reading = useRef(false);
  const refreshQueued = useRef(false);

  useEffect(() => {
    mounted.current = true;
    return () => { mounted.current = false; pending.current = null; };
  }, [deviceName]);

  const fetchBrightness = useCallback(async (): Promise<void> => {
    if (writing.current || reading.current) {
      refreshQueued.current = true;
      return;
    }
    if (!deviceName || disabled) {
      setPct(null);
      return;
    }
    const version = writeVersion.current;
    reading.current = true;
    refreshQueued.current = false;
    try {
      const info = await invoke<{
        min: number;
        current: number;
        max: number;
      }>("get_monitor_brightness", { deviceName });
      if (!mounted.current || writing.current || version !== writeVersion.current) return;
      if (info) {
        const span = Math.max(1, info.max - info.min);
        const val = Math.round(((info.current - info.min) / span) * 100);
        setPct(Math.max(0, Math.min(100, val)));
        setUnavailable(false);
      }
    } catch {
      if (!mounted.current || writing.current || version !== writeVersion.current) return;
      setPct(null);
      setUnavailable(true);
    } finally {
      reading.current = false;
      if (refreshQueued.current && mounted.current && !writing.current) {
        refreshQueued.current = false;
        void fetchBrightness();
      }
    }
  }, [deviceName, disabled]);

  useEffect(() => {
    if (!isTauri()) return;
    let cancelled = false;
    (async () => {
      if (cancelled) return;
      await fetchBrightness();
    })().catch(console.error);
    const onFocus = () => { void fetchBrightness(); };
    window.addEventListener("focus", onFocus);
    return () => {
      cancelled = true;
      window.removeEventListener("focus", onFocus);
    };
  }, [fetchBrightness]);

  useEffect(() => {
    if (!isTauri()) return;
    let unlisten: (() => void) | null = null;
    let disposed = false;
    (async () => {
      unlisten = await listen("brightness-changed", async () => {
        await fetchBrightness();
      });
      if (disposed) unlisten();
    })().catch(console.error);
    return () => {
      disposed = true;
      if (unlisten) unlisten();
    };
  }, [fetchBrightness]);

  const apply = useCallback(
    async (next: number) => {
      if (!deviceName || disabled || !mounted.current) return;
      writeVersion.current += 1;
      // Only one native write may run at a time. During a drag, replace queued
      // values with the latest choice so an older write cannot win on release.
      pending.current = writing.current && inFlight.current === next ? null : next;
      if (writing.current) return;
      writing.current = true;
      try {
        while (mounted.current && pending.current !== null) {
          const percent = pending.current;
          pending.current = null;
          inFlight.current = percent;
          await invoke("set_monitor_brightness", { deviceName, percent });
        }
      } catch (e) {
        pending.current = null;
        if (mounted.current) onError?.((e as Error).message ?? String(e));
      } finally {
        writing.current = false;
        inFlight.current = null;
        // Read the actual value after success too: the panel may round a request
        // and Windows brightness keys can change it independently of this slider.
        if (mounted.current) await fetchBrightness();
      }
    },
    [deviceName, disabled, onError, fetchBrightness]
  );

  const title = disabled && disabledReason ? disabledReason : undefined;

  return (
    <div className="field" title={title}>
      <label className="label" htmlFor="brightness-range">
        Brightness
      </label>
      <Slider
        id="brightness-range"
        min={0}
        max={100}
        step={1}
        disabled={disabled || pct === null}
        title={title}
        value={pct ?? 0}
        onChange={(next) => {
          setPct(next);
          void apply(next);
        }}
        onValueSubmit={(next) => {
          setPct(next);
          void apply(next);
        }}
        icon={<BrightnessIcon />}
        label={pct === null ? "Unavailable" : `${pct}%`}
      />
      {(title || unavailable) && <p className="optional-control-note">{title ?? "Brightness is unavailable for this display."}</p>}
    </div>
  );
}
