//! Battery-aware folder pausing. All Syncthing IO stays off the UI/message thread.
mod api;
mod policy;

use crate::power_source::read_power;
use api::Api;
use policy::{Policy, PowerSource, SavedState};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    supported: bool,
    available: bool,
    paused: bool,
    auto_pause_on_battery: bool,
    on_battery: Option<bool>,
    battery_override: bool,
    managed_folders: usize,
    total_folders: usize,
    paused_folders: usize,
    message: String,
}

impl Status {
    fn initial(supported: bool, message: &str) -> Self {
        Self {
            supported,
            available: false,
            paused: false,
            auto_pause_on_battery: true,
            on_battery: None,
            battery_override: false,
            managed_folders: 0,
            total_folders: 0,
            paused_folders: 0,
            message: message.into(),
        }
    }
}

type Reply = mpsc::Sender<Result<Status, String>>;
enum Action {
    Refresh,
    Pause(bool, Reply),
    AutoPause(bool, Reply),
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
        let supported = cfg!(target_os = "windows") && !preview;
        let message = if preview {
            "Preview — Syncthing is unchanged."
        } else if !supported {
            "Syncthing power controls require Windows."
        } else {
            "Connecting to Syncthing…"
        };
        let status = Arc::new(Mutex::new(Status::initial(supported, message)));
        let controller = Self {
            sender,
            status: status.clone(),
        };
        if supported {
            std::thread::spawn(move || {
                let mut engine = match Engine::new(app.clone(), status.clone()) {
                    Ok(engine) => engine,
                    Err(error) => {
                        let mut failed = Status::initial(true, &error);
                        failed.auto_pause_on_battery = false;
                        publish(&app, &status, failed);
                        return;
                    }
                };
                engine.refresh();
                loop {
                    // Power broadcasts wake this immediately. The slow retry also
                    // handles Syncthing starting/restarting and newly added folders.
                    match receiver.recv_timeout(Duration::from_secs(30)) {
                        Ok(Action::Pause(paused, reply)) => {
                            let _ = reply.send(engine.change(|p| p.set_paused(paused)));
                        }
                        Ok(Action::AutoPause(enabled, reply)) => {
                            let _ = reply.send(engine.change(|p| p.set_auto_pause(enabled)));
                        }
                        Ok(Action::Stop(reply)) => {
                            if let Err(error) = engine.release() {
                                log::warn!("Syncthing pause recovery is pending: {error}");
                            }
                            let _ = reply.send(());
                            break;
                        }
                        Ok(Action::Refresh) | Err(mpsc::RecvTimeoutError::Timeout) => {
                            engine.refresh();
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
            });
        }
        controller
    }

    fn snapshot(&self) -> Status {
        self.status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    fn request(&self, action: impl FnOnce(Reply) -> Action) -> Result<Status, String> {
        let (reply, receiver) = mpsc::channel();
        self.sender.send(action(reply)).map_err(|_| {
            "Syncthing controls are unavailable. Restart Pro 9i Control.".to_string()
        })?;
        receiver
            .recv_timeout(Duration::from_secs(30))
            .map_err(|_| {
                "Syncthing is taking too long to respond. The controller will retry.".to_string()
            })?
    }

    pub fn refresh(&self) {
        let _ = self.sender.send(Action::Refresh);
    }

    pub fn shutdown(&self) {
        let (reply, receiver) = mpsc::channel();
        if self.sender.send(Action::Stop(reply)).is_ok()
            && receiver.recv_timeout(Duration::from_secs(10)).is_err()
        {
            log::warn!(
                "Syncthing cleanup timed out; saved pause ownership will recover at next startup."
            );
        }
    }
}

fn publish(app: &AppHandle, shared: &Mutex<Status>, next: Status) {
    let mut current = shared.lock().unwrap_or_else(|e| e.into_inner());
    if *current != next {
        *current = next.clone();
        drop(current);
        let _ = app.emit("syncthing-status-changed", next);
    }
}

struct Journal(PathBuf);

impl Journal {
    fn load(&self) -> Result<SavedState, String> {
        match std::fs::read(&self.0) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|_| "Saved Syncthing controls could not be read. Existing pauses have been left in place.".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(SavedState::default()),
            Err(_) => Err("Saved Syncthing controls are inaccessible.".into()),
        }
    }

    fn save(&self, state: &SavedState) -> Result<(), String> {
        crate::state_file::write_atomic(&self.0, state)
            .map_err(|_| "Could not save Syncthing pause preferences and recovery data.".into())
    }
}

struct Engine {
    app: AppHandle,
    shared: Arc<Mutex<Status>>,
    policy: Policy,
    journal: Journal,
    api: Option<Api>,
}

impl Engine {
    fn new(app: AppHandle, shared: Arc<Mutex<Status>>) -> Result<Self, String> {
        let path = app
            .path()
            .app_data_dir()
            .map_err(|_| "App data directory is unavailable.".to_string())?;
        let journal = Journal(path.join("syncthing.json"));
        let policy = Policy::new(journal.load()?);
        Ok(Self {
            app,
            shared,
            policy,
            journal,
            api: None,
        })
    }

    fn connect(&mut self) -> Result<(), String> {
        if self.api.is_none() {
            let api = Api::discover(self.policy.saved.config_path.as_deref())?;
            if self.policy.saved.config_path.as_ref() != Some(&api.config_path) {
                if !self.policy.saved.owned_folders.is_empty() {
                    return Err("Syncthing's configuration moved. Reconnect the original configuration to restore its paused folders.".into());
                }
                self.policy.saved.config_path = Some(api.config_path.clone());
                self.journal.save(&self.policy.saved)?;
            }
            self.api = Some(api);
        }
        Ok(())
    }

    fn reconcile(&mut self, release: bool) -> Result<Vec<policy::Folder>, String> {
        self.connect()?;
        self.policy.reconcile(
            self.api.as_ref().expect("connected API"),
            &|s| self.journal.save(s),
            release,
        )
    }

    fn refresh(&mut self) -> Status {
        self.policy.observe_power(read_power());
        let mut next = Status::initial(true, "");
        next.auto_pause_on_battery = self.policy.saved.auto_pause_on_battery;
        next.on_battery = match self.policy.power {
            PowerSource::Ac => Some(false),
            PowerSource::Battery => Some(true),
            PowerSource::Unknown => None,
        };
        next.battery_override = self.policy.battery_override;
        match self.reconcile(false) {
            Ok(folders) => {
                next.available = true;
                next.total_folders = folders.len();
                next.paused_folders = folders.iter().filter(|f| f.paused).count();
                next.message = if folders.is_empty() {
                    "No Syncthing folders configured.".into()
                } else if self.policy.desired_pause() == Some(true) {
                    if self.policy.saved.manual_paused {
                        "Paused manually.".into()
                    } else {
                        "Paused automatically on battery.".into()
                    }
                } else if self.policy.battery_override && self.policy.saved.auto_pause_on_battery {
                    "Running on battery until the power source changes.".into()
                } else if self.policy.power == PowerSource::Unknown
                    && self.policy.saved.auto_pause_on_battery
                {
                    "Power source unknown. Keeping existing folder states.".into()
                } else if next.paused_folders == folders.len() {
                    "Folders paused outside Pro 9i Control remain paused.".into()
                } else if next.paused_folders > 0 {
                    "Running. Previously paused folders stay paused.".into()
                } else {
                    "Running.".into()
                };
            }
            Err(error) => {
                next.message = error;
                // Reload connection details after a restart or API key rotation.
                self.api = None;
            }
        }
        next.managed_folders = self.policy.saved.owned_folders.len();
        next.paused = self
            .policy
            .desired_pause()
            .unwrap_or(next.managed_folders > 0);
        publish(&self.app, &self.shared, next.clone());
        next
    }

    fn change(&mut self, change: impl FnOnce(&mut Policy)) -> Result<Status, String> {
        self.policy.observe_power(read_power());
        let previous = self.policy.saved.clone();
        let previous_override = self.policy.battery_override;
        change(&mut self.policy);
        if let Err(error) = self.journal.save(&self.policy.saved) {
            self.policy.saved = previous;
            self.policy.battery_override = previous_override;
            return Err(error);
        }
        Ok(self.refresh())
    }

    fn release(&mut self) -> Result<(), String> {
        if !self.policy.saved.owned_folders.is_empty() {
            self.reconcile(true)?;
        }
        Ok(())
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
pub fn get_syncthing_status(controller: State<'_, Controller>) -> Status {
    controller.refresh();
    controller.snapshot()
}

#[tauri::command]
pub async fn set_syncthing_paused(
    controller: State<'_, Controller>,
    paused: bool,
) -> Result<Status, String> {
    let controller = controller.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        controller.request(|reply| Action::Pause(paused, reply))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn set_syncthing_auto_pause(
    controller: State<'_, Controller>,
    enabled: bool,
) -> Result<Status, String> {
    let controller = controller.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        controller.request(|reply| Action::AutoPause(enabled, reply))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    fn journal() -> Journal {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Journal(std::env::temp_dir().join(format!(
            "hzhdr-syncthing-{}-{stamp}.json",
            std::process::id()
        )))
    }

    #[test]
    fn journal_replaces_existing_file_and_preserves_pause_ownership() {
        let journal = journal();
        let mut state = journal.load().unwrap();
        assert!(state.auto_pause_on_battery);
        journal.save(&state).unwrap();
        state.auto_pause_on_battery = false;
        state.owned_folders.insert("recover-after-restart".into());
        journal.save(&state).unwrap();
        let restored = journal.load().unwrap();
        assert!(!restored.auto_pause_on_battery);
        assert!(restored.owned_folders.contains("recover-after-restart"));
        std::fs::remove_file(journal.0).unwrap();
    }

    #[test]
    fn damaged_recovery_data_is_not_silently_discarded() {
        let journal = journal();
        std::fs::write(&journal.0, b"incomplete json").unwrap();
        assert!(journal.load().is_err());
        assert_eq!(std::fs::read(&journal.0).unwrap(), b"incomplete json");
        std::fs::remove_file(journal.0).unwrap();
    }
}
