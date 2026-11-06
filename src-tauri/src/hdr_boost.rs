//! Thin Tauri adapter. The engine and its recovery format live in hdr-boost/.
use lenovo_hdr_boost::{Controller, Scenario, Status};
use std::sync::OnceLock;

pub fn controller() -> Result<&'static Controller, String> {
    static ENGINE: OnceLock<Result<Controller, String>> = OnceLock::new();
    ENGINE
        .get_or_init(Controller::new)
        .as_ref()
        .map_err(Clone::clone)
}

#[tauri::command]
pub async fn get_hdr_boost_status() -> Result<Status, String> {
    tauri::async_runtime::spawn_blocking(|| Ok(controller()?.refresh_availability()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn start_hdr_boost(device_name: String, strength: f32) -> Result<Status, String> {
    if cfg!(feature = "fake-displays") {
        return Err("HDR Boost is disabled with fake displays.".into());
    }
    controller()?.start(device_name, strength, false, Scenario::Live)
}

#[tauri::command]
pub fn stop_hdr_boost() -> Result<Status, String> {
    Ok(controller()?.stop())
}

#[tauri::command]
pub fn set_hdr_boost_strength(strength: f32) -> Result<Status, String> {
    controller()?.set_strength(strength)
}

#[tauri::command]
pub fn restore_hdr_boost() -> Result<Status, String> {
    controller()?.recover()
}

pub fn stop_and_wait() {
    if let Ok(engine) = controller() {
        engine.stop();
        let mut status = engine.wait();
        if status.recovery_pending && engine.recover().is_ok() {
            status = engine.wait();
        }
        if status.recovery_pending {
            log::error!("HDR Boost: {}", status.message);
        }
    }
}
