import { useSyncExternalStore, useCallback } from "react";
import { Store } from "@tauri-apps/plugin-store";
import { error as logError } from "@tauri-apps/plugin-log";
import { isTauri } from "@tauri-apps/api/core";
import { DEFAULT_VISIBLE_CONTROLS, normalizeVisibleControls, type ControlVisibility } from "../lib/interfaceControls";

export type KeyboardBrightnessShortcut =
  | "all_screens"
  | "screen_with_mouse"
  | "system";

export type ThemeMode = "light" | "dark" | "system";

export type Settings = {
  showUIOnMonitorChange: boolean;
  keyboardBrightnessShortcut: KeyboardBrightnessShortcut;
  showStartupNotification: boolean;
  shouldHideUIOnFocusOut: boolean;
  theme: ThemeMode;
  visibleControls: ControlVisibility;
};

const DEFAULT_SETTINGS: Settings = {
  showUIOnMonitorChange: false,
  keyboardBrightnessShortcut: "system",
  showStartupNotification: true,
  shouldHideUIOnFocusOut: true,
  theme: "system",
  visibleControls: DEFAULT_VISIBLE_CONTROLS,
};

const STORE_KEY = "settings";
const BROWSER_STORE_KEY = "pro-9i-control-settings";

class SettingsStore {
  private store: Store | null = null;
  private settings: Settings = DEFAULT_SETTINGS;
  private loading: boolean = true;
  private listeners: Set<() => void> = new Set();
  private saveQueue: Promise<void> = Promise.resolve();

  async init() {
    try {
      let stored: Partial<Settings> | null | undefined;
      if (isTauri()) {
        this.store = await Store.load("settings.json");
        stored = await this.store.get<Partial<Settings>>(STORE_KEY);
      } else {
        stored = JSON.parse(localStorage.getItem(BROWSER_STORE_KEY) ?? "null");
      }
      this.settings = stored
        ? { ...DEFAULT_SETTINGS, ...stored, visibleControls: normalizeVisibleControls(stored.visibleControls) }
        : DEFAULT_SETTINGS;
    } catch (err) {
      this.reportError(`Failed to load settings: ${err}`);
    } finally {
      this.loading = false;
      this.notify();
    }
  }

  getSettings(): Settings {
    return this.settings;
  }

  isLoading(): boolean {
    return this.loading;
  }

  async updateSettings(partial: Partial<Settings>): Promise<void> {
    const previous = this.settings;
    const next = { ...previous, ...partial };
    this.settings = next;
    this.notify();

    const save = this.saveQueue.catch(() => {}).then(async () => {
      if (isTauri()) {
        if (!this.store) this.store = await Store.load("settings.json");
        await this.store.set(STORE_KEY, next);
        await this.store.save();
      } else {
        localStorage.setItem(BROWSER_STORE_KEY, JSON.stringify(next));
      }
    });
    this.saveQueue = save;
    try {
      await save;
    } catch (err) {
      if (this.settings === next) {
        this.settings = previous;
        this.notify();
      }
      this.reportError(`Failed to save settings: ${err}`);
      throw err;
    }
  }

  private reportError(message: string) {
    if (isTauri()) void logError(message).catch(console.error);
    else console.error(message);
  }

  subscribe(listener: () => void): () => void {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  private notify() {
    this.listeners.forEach((listener) => listener());
  }
}

const settingsStore = new SettingsStore();
settingsStore.init();

export function useSettings() {
  const settings = useSyncExternalStore(
    (listener) => settingsStore.subscribe(listener),
    () => settingsStore.getSettings(),
    () => DEFAULT_SETTINGS
  );

  const loading = useSyncExternalStore(
    (listener) => settingsStore.subscribe(listener),
    () => settingsStore.isLoading(),
    () => true
  );

  const updateSettings = useCallback(async (partial: Partial<Settings>) => {
    await settingsStore.updateSettings(partial);
  }, []);

  return {
    settings,
    updateSettings,
    loading,
  };
}
