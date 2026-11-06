import { useEffect, useRef, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { relaunch } from "@tauri-apps/plugin-process";
import { useSettings, type Settings as AppSettings } from "../../hooks/useSettings";
import { CONTROL_GROUPS, DEFAULT_VISIBLE_CONTROLS, allControlsVisible } from "../../lib/interfaceControls";
import "./Settings.css";

type SettingsProps = {
  onClose: () => void;
  onError: (message: string) => void;
};

type PreferenceProps = {
  label: string;
  description: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
};

function Preference({ label, description, checked, onChange }: PreferenceProps) {
  return (
    <label className="customize-option">
      <span className="customize-option-copy">
        <span>{label}</span>
        <small>{description}</small>
      </span>
      <input type="checkbox" checked={checked} onChange={(event) => onChange(event.target.checked)} />
    </label>
  );
}

export function Settings({ onClose, onError }: SettingsProps) {
  const { settings, updateSettings, loading } = useSettings();
  const heading = useRef<HTMLHeadingElement>(null);
  const [saving, setSaving] = useState(0);
  const [saveFailed, setSaveFailed] = useState(false);

  useEffect(() => {
    heading.current?.focus();
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  const save = async (partial: Partial<AppSettings>) => {
    setSaving((count) => count + 1);
    setSaveFailed(false);
    try {
      await updateSettings(partial);
    } catch {
      setSaveFailed(true);
      onError("Could not save your preferences. Please try again.");
    } finally {
      setSaving((count) => count - 1);
    }
  };

  const shown = Object.values(settings.visibleControls).filter(Boolean).length;
  const total = Object.keys(DEFAULT_VISIBLE_CONTROLS).length;

  return (
    <section className="customize-panel" aria-labelledby="customize-heading">
      <div className="customize-heading">
        <div>
          <h1 id="customize-heading" ref={heading} tabIndex={-1}>Customize your panel</h1>
          <p>Choose which controls appear. Changes save automatically.</p>
        </div>
        <button type="button" className="button" onClick={onClose}>Done</button>
      </div>

      <div className="customize-presets" aria-label="Interface presets">
        <button type="button" className="button" disabled={loading} onClick={() => void save({ visibleControls: allControlsVisible(true) })}>Show all</button>
        <button type="button" className="button" disabled={loading} onClick={() => void save({ visibleControls: allControlsVisible(false) })}>Hide all</button>
        <button type="button" className="button" disabled={loading} onClick={() => void save({ visibleControls: { ...DEFAULT_VISIBLE_CONTROLS } })}>Restore defaults</button>
      </div>
      <p className="customize-save-status" role="status">
        {loading ? "Loading preferences…" : saving ? "Saving…" : saveFailed ? "Preferences could not be saved." : `${shown} of ${total} controls shown`}
      </p>

      {CONTROL_GROUPS.map((group) => (
        <fieldset className="customize-group" key={group.title} disabled={loading}>
          <legend>{group.title} controls</legend>
          {group.controls.map((control) => (
            <Preference key={control.id} label={control.label} description={control.description}
              checked={settings.visibleControls[control.id]}
              onChange={(checked) => void save({ visibleControls: { ...settings.visibleControls, [control.id]: checked } })} />
          ))}
        </fieldset>
      ))}
      <p className="customize-note">Some controls need compatible hardware. Hiding a control keeps its current settings, including battery and sync automation. Active HDR Boost stays accessible until stopped.</p>

      <fieldset className="customize-group" disabled={loading}>
        <legend>Window behavior</legend>
        <Preference label="Hide when focus is lost" description="Close the tray panel when you click elsewhere."
          checked={settings.shouldHideUIOnFocusOut}
          onChange={(checked) => void save({ shouldHideUIOnFocusOut: checked })} />
        <Preference label="Startup notification" description="Show a notification when the app starts."
          checked={settings.showStartupNotification}
          onChange={(checked) => void save({ showStartupNotification: checked })} />
      </fieldset>

      <details className="customize-shortcuts">
        <summary>Brightness shortcuts</summary>
        <label htmlFor="brightness-shortcuts">Brightness keys affect</label>
        <select id="brightness-shortcuts" value={settings.keyboardBrightnessShortcut} disabled={loading}
          onChange={(event) => void save({ keyboardBrightnessShortcut: event.target.value as AppSettings["keyboardBrightnessShortcut"] })}>
          <option value="system">Use Windows behavior</option>
          <option value="all_screens">All displays (F14 / F15)</option>
          {settings.keyboardBrightnessShortcut === "screen_with_mouse" && <option value="screen_with_mouse">Display under the pointer (saved preference)</option>}
        </select>
        <p className="customize-note">All displays uses F14 and F15. Bind those keys in your keyboard software. Shortcut changes take effect after restarting the app.</p>
        <button type="button" className="button" disabled={saving > 0 || !isTauri()}
          onClick={() => void relaunch().catch(() => onError("Could not restart the app. Exit it from the tray and reopen it."))}>Restart app</button>
      </details>
    </section>
  );
}
