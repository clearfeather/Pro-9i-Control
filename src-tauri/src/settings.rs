use serde_json::Value;
use tauri::{App, AppHandle};
use tauri_plugin_store::StoreExt;

const AUTOSTART_ENABLED_KEY: &str = "autostartEnabled";

pub fn should_register_brightness_hotkeys_app(app: &App) -> bool {
    if let Ok(store) = app.store("settings.json") {
        if let Some(Value::Object(map)) = store.get("settings") {
            if let Some(s) = map
                .get("keyboardBrightnessShortcut")
                .and_then(|v| v.as_str())
            {
                return matches!(s, "all_screens" | "screen_with_mouse");
            }
        }
    }
    false
}

pub fn should_show_startup_notification_app(app: &App) -> bool {
    if let Ok(store) = app.store("settings.json") {
        if let Some(Value::Object(map)) = store.get("settings") {
            if let Some(v) = map.get("showStartupNotification").and_then(|v| v.as_bool()) {
                return v;
            }
        }
    }
    true
}

pub fn should_hide_ui_on_focus_out_handle(handle: &AppHandle) -> bool {
    if let Ok(store) = handle.store("settings.json") {
        if let Some(Value::Object(map)) = store.get("settings") {
            if let Some(v) = map.get("shouldHideUIOnFocusOut").and_then(|v| v.as_bool()) {
                return v;
            }
        }
    }
    true
}

pub fn autostart_preference_app(app: &App) -> Option<bool> {
    app.store("settings.json")
        .ok()
        .and_then(|store| store.get(AUTOSTART_ENABLED_KEY))
        .and_then(|value| value.as_bool())
}

pub fn save_autostart_preference(handle: &AppHandle, enabled: bool) {
    match handle.store("settings.json") {
        Ok(store) => {
            store.set(AUTOSTART_ENABLED_KEY, Value::Bool(enabled));
            if let Err(error) = store.save() {
                log::warn!("Failed to save Start at login preference: {error}");
            }
        }
        Err(error) => {
            log::warn!("Failed to open settings for Start at login: {error}");
        }
    }
}
