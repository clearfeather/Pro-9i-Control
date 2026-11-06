//! Background battery preferences for the laptop panel and Lenovo power mode.
mod native;
mod policy;

use crate::power_source::{read_power, PowerSource};
use native::{PowerModeSetting, RefreshSetting, RefreshValue};
use policy::{Control, SavedControl};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    cell::RefCell,
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Feature {
    RefreshRate,
    PowerMode,
    KeepBrightness,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlStatus {
    enabled: bool,
    active: bool,
    overridden: bool,
    message: Option<String>,
}

impl Default for ControlStatus {
    fn default() -> Self {
        Self {
            enabled: true,
            active: false,
            overridden: false,
            message: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    supported: bool,
    on_battery: Option<bool>,
    refresh_rate: ControlStatus,
    power_mode: ControlStatus,
    keep_brightness: bool,
    brightness_message: Option<String>,
}

type Reply<T> = mpsc::Sender<Result<T, String>>;
enum Action {
    Refresh,
    Enable(Feature, bool, Reply<Status>),
    ManualRefresh(String, u32, u32, Option<u32>, Reply<()>),
    ManualMode(String, Reply<crate::lenovo_power::PowerModeStatus>),
    Stop(mpsc::Sender<()>),
}

#[derive(Clone)]
pub struct Controller {
    sender: mpsc::Sender<Action>,
    status: Arc<Mutex<Status>>,
}

impl Controller {
    pub fn start(app: AppHandle, preview: bool) -> Self {
        let (sender, receiver) = mpsc::channel();
        let supported = cfg!(target_os = "windows") && !cfg!(feature = "fake-displays") && !preview;
        let status = Arc::new(Mutex::new(Status {
            supported,
            on_battery: None,
            refresh_rate: ControlStatus::default(),
            power_mode: ControlStatus::default(),
            keep_brightness: true,
            brightness_message: None,
        }));
        let controller = Self {
            sender,
            status: status.clone(),
        };
        if supported {
            std::thread::spawn(move || {
                let mut engine = match Engine::new(app.clone(), status.clone()) {
                    Ok(engine) => engine,
                    Err(error) => {
                        let mut failed = status.lock().unwrap_or_else(|e| e.into_inner()).clone();
                        failed.supported = false;
                        failed.refresh_rate.message = Some(error.clone());
                        failed.power_mode.message = Some(error);
                        publish(&app, &status, failed);
                        return;
                    }
                };
                engine.refresh();
                loop {
                    match receiver.recv_timeout(Duration::from_secs(30)) {
                        Ok(Action::Enable(feature, enabled, reply)) => {
                            let _ = reply.send(engine.set_enabled(feature, enabled));
                        }
                        Ok(Action::ManualRefresh(device, width, height, hz, reply)) => {
                            engine.observe_power();
                            let result = native::manual_refresh(device, width, height, hz)
                                .and_then(|built_in| {
                                    if built_in {
                                        engine
                                            .refresh_control
                                            .manual_changed(&|s| engine.refresh_journal.save(s))?;
                                    }
                                    Ok(())
                                });
                            engine.refresh();
                            let _ = reply.send(result);
                        }
                        Ok(Action::ManualMode(mode, reply)) => {
                            engine.observe_power();
                            let result =
                                engine
                                    .brightness
                                    .borrow_mut()
                                    .apply(&mode)
                                    .and_then(|status| {
                                        engine
                                            .mode_control
                                            .manual_changed(&|s| engine.mode_journal.save(s))?;
                                        Ok(status)
                                    });
                            engine.refresh();
                            let _ = reply.send(result);
                        }
                        Ok(Action::Refresh) | Err(mpsc::RecvTimeoutError::Timeout) => {
                            engine.refresh();
                        }
                        Ok(Action::Stop(reply)) => {
                            engine.release();
                            let _ = reply.send(());
                            break;
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            });
        }
        controller
    }

    fn request<T>(&self, action: impl FnOnce(Reply<T>) -> Action) -> Result<T, String> {
        let (reply, receiver) = mpsc::channel();
        self.sender
            .send(action(reply))
            .map_err(|_| "Battery controls are unavailable. Restart Pro 9i Control.".to_string())?;
        receiver
            .recv_timeout(Duration::from_secs(30))
            .map_err(|_| "Battery controls are taking too long to respond.".to_string())?
    }

    pub fn manual_refresh(
        &self,
        device: String,
        width: u32,
        height: u32,
        hz: Option<u32>,
    ) -> Result<(), String> {
        self.request(|reply| Action::ManualRefresh(device, width, height, hz, reply))
    }

    pub fn manual_mode(
        &self,
        mode: String,
    ) -> Result<crate::lenovo_power::PowerModeStatus, String> {
        self.request(|reply| Action::ManualMode(mode, reply))
    }

    fn snapshot(&self) -> Status {
        self.status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub fn supported(&self) -> bool {
        self.snapshot().supported
    }
    pub fn refresh(&self) {
        let _ = self.sender.send(Action::Refresh);
    }

    fn shutdown(&self) {
        let (reply, receiver) = mpsc::channel();
        if self.sender.send(Action::Stop(reply)).is_ok()
            && receiver.recv_timeout(Duration::from_secs(10)).is_err()
        {
            log::warn!("Battery control cleanup timed out; recovery will retry on next startup.");
        }
    }
}

fn publish(app: &AppHandle, shared: &Mutex<Status>, next: Status) {
    let mut current = shared.lock().unwrap_or_else(|e| e.into_inner());
    if *current != next {
        *current = next.clone();
        drop(current);
        let _ = app.emit("battery-controls-changed", next);
    }
}

struct Journal(PathBuf);
impl Journal {
    fn load<T: DeserializeOwned>(&self) -> Result<SavedControl<T>, String> {
        match std::fs::read(&self.0) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|_| "Saved battery controls could not be read. Existing settings have been retained.".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(SavedControl::default()),
            Err(_) => Err("Saved battery controls are inaccessible.".into()),
        }
    }
    fn save<T: Serialize>(&self, saved: &SavedControl<T>) -> Result<(), String> {
        crate::state_file::write_atomic(&self.0, saved)
            .map_err(|_| "Could not save battery preferences and recovery data.".into())
    }
}

struct Engine {
    app: AppHandle,
    status: Arc<Mutex<Status>>,
    refresh_control: Control<RefreshValue>,
    mode_control: Control<String>,
    refresh_journal: Journal,
    mode_journal: Journal,
    brightness: RefCell<crate::power_brightness::BrightnessKeeper>,
    brightness_path: PathBuf,
}

#[derive(Deserialize, Serialize)]
struct BrightnessPreference {
    enabled: bool,
}

fn load_brightness_preference(path: &std::path::Path) -> Result<bool, String> {
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice::<BrightnessPreference>(&bytes)
            .map(|p| p.enabled)
            .map_err(|_| {
                "Saved brightness preference could not be read. Brightness keeping is disabled."
                    .into()
            }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(true),
        Err(_) => Err(
            "Saved brightness preference is inaccessible. Brightness keeping is disabled.".into(),
        ),
    }
}

impl Engine {
    fn new(app: AppHandle, status: Arc<Mutex<Status>>) -> Result<Self, String> {
        let directory = app
            .path()
            .app_data_dir()
            .map_err(|_| "App data directory is unavailable.".to_string())?;
        let refresh_journal = Journal(directory.join("battery-refresh.json"));
        let mode_journal = Journal(directory.join("battery-power-mode.json"));
        let brightness_path = directory.join("power-brightness.json");
        let (enabled, message) = match load_brightness_preference(&brightness_path) {
            Ok(enabled) => (enabled, None),
            Err(error) => (false, Some(error)),
        };
        let mut brightness = crate::power_brightness::BrightnessKeeper::new(enabled);
        brightness.message = message;
        Ok(Self {
            app,
            status,
            refresh_control: Control::new(refresh_journal.load()?),
            mode_control: Control::new(mode_journal.load()?),
            refresh_journal,
            mode_journal,
            brightness: RefCell::new(brightness),
            brightness_path,
        })
    }

    fn observe_power(&mut self) -> PowerSource {
        let power = read_power();
        self.refresh_control.observe_power(power);
        self.mode_control.observe_power(power);
        power
    }

    fn refresh(&mut self) -> Status {
        let power = self.observe_power();
        let refresh_result = self.refresh_control.reconcile(
            &RefreshSetting,
            &|s| self.refresh_journal.save(s),
            false,
        );
        let mode_result = self.mode_control.reconcile(
            &PowerModeSetting {
                brightness: &self.brightness,
            },
            &|s| self.mode_journal.save(s),
            false,
        );
        let next = Status {
            supported: true,
            on_battery: power.on_battery(),
            refresh_rate: control_status(&self.refresh_control, refresh_result),
            power_mode: control_status(&self.mode_control, mode_result),
            keep_brightness: self.brightness.borrow().enabled,
            brightness_message: self.brightness.borrow().message.clone(),
        };
        publish(&self.app, &self.status, next.clone());
        next
    }

    fn set_enabled(&mut self, feature: Feature, enabled: bool) -> Result<Status, String> {
        self.observe_power();
        match feature {
            Feature::RefreshRate => self
                .refresh_control
                .set_enabled(enabled, &|s| self.refresh_journal.save(s))?,
            Feature::PowerMode => self
                .mode_control
                .set_enabled(enabled, &|s| self.mode_journal.save(s))?,
            Feature::KeepBrightness => {
                crate::state_file::write_atomic(
                    &self.brightness_path,
                    &BrightnessPreference { enabled },
                )
                .map_err(|_| "Could not save the brightness preference.".to_string())?;
                let mut brightness = self.brightness.borrow_mut();
                brightness.enabled = enabled;
                brightness.message = None;
            }
        }
        Ok(self.refresh())
    }

    fn release(&mut self) {
        if let Err(error) =
            self.refresh_control
                .reconcile(&RefreshSetting, &|s| self.refresh_journal.save(s), true)
        {
            log::warn!("Refresh-rate restoration is pending: {error}");
        }
        if let Err(error) = self.mode_control.reconcile(
            &PowerModeSetting {
                brightness: &self.brightness,
            },
            &|s| self.mode_journal.save(s),
            true,
        ) {
            log::warn!("Power-mode restoration is pending: {error}");
        }
    }
}

fn control_status<T>(control: &Control<T>, result: Result<bool, String>) -> ControlStatus {
    ControlStatus {
        enabled: control.saved.enabled,
        active: result.as_ref().copied().unwrap_or(false),
        overridden: control.overridden,
        message: result.err(),
    }
}

pub fn power_changed(app: &AppHandle) {
    if let Some(controller) = app.try_state::<Controller>() {
        controller.refresh();
    }
}

pub fn shutdown(app: &AppHandle) {
    if let Some(controller) = app.try_state::<Controller>() {
        controller.shutdown();
    }
}

#[tauri::command]
pub fn get_battery_controls(controller: State<'_, Controller>) -> Status {
    controller.refresh();
    controller.snapshot()
}

#[tauri::command]
pub async fn set_battery_control(
    controller: State<'_, Controller>,
    feature: Feature,
    enabled: bool,
) -> Result<Status, String> {
    let controller = controller.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        controller.request(|reply| Action::Enable(feature, enabled, reply))
    })
    .await
    .map_err(|error| error.to_string())?
}
