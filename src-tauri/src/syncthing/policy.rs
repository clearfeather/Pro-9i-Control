use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub use crate::power_source::PowerSource;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SavedState {
    pub auto_pause_on_battery: bool,
    pub manual_paused: bool,
    pub owned_folders: BTreeSet<String>,
    pub config_path: Option<String>,
}

impl Default for SavedState {
    fn default() -> Self {
        Self {
            auto_pause_on_battery: true,
            manual_paused: false,
            owned_folders: BTreeSet::new(),
            config_path: None,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct Folder {
    pub id: String,
    #[serde(default)]
    pub paused: bool,
}

pub trait FolderApi {
    fn folders(&self) -> Result<Vec<Folder>, String>;
    fn set_paused(&self, id: &str, paused: bool) -> Result<(), String>;
}

pub struct Policy {
    pub saved: SavedState,
    pub power: PowerSource,
    pub battery_override: bool,
    last_known_power: Option<PowerSource>,
}

impl Policy {
    pub fn new(saved: SavedState) -> Self {
        Self {
            saved,
            power: PowerSource::Unknown,
            battery_override: false,
            last_known_power: None,
        }
    }

    pub fn observe_power(&mut self, power: PowerSource) {
        self.power = power;
        if power != PowerSource::Unknown && self.last_known_power != Some(power) {
            self.battery_override = false;
            self.last_known_power = Some(power);
        }
    }

    pub fn set_paused(&mut self, paused: bool) {
        self.saved.manual_paused = paused;
        self.battery_override = !paused && self.power == PowerSource::Battery;
    }

    pub fn set_auto_pause(&mut self, enabled: bool) {
        self.saved.auto_pause_on_battery = enabled;
        self.battery_override = false;
    }

    pub fn desired_pause(&self) -> Option<bool> {
        if self.saved.manual_paused {
            Some(true)
        } else if !self.saved.auto_pause_on_battery {
            Some(false)
        } else {
            match self.power {
                PowerSource::Ac => Some(false),
                PowerSource::Battery => Some(!self.battery_override),
                PowerSource::Unknown => None,
            }
        }
    }

    pub fn reconcile(
        &mut self,
        api: &impl FolderApi,
        save: &impl Fn(&SavedState) -> Result<(), String>,
        release: bool,
    ) -> Result<Vec<Folder>, String> {
        let mut folders = api.folders()?;
        let existing: BTreeSet<_> = folders.iter().map(|f| f.id.clone()).collect();
        let retained: BTreeSet<_> = self
            .saved
            .owned_folders
            .intersection(&existing)
            .cloned()
            .collect();
        if retained != self.saved.owned_folders {
            let previous = self.saved.owned_folders.clone();
            self.saved.owned_folders = retained;
            if let Err(error) = save(&self.saved) {
                self.saved.owned_folders = previous;
                return Err(error);
            }
        }
        let desired = if release {
            Some(false)
        } else {
            self.desired_pause()
        };
        for folder in &mut folders {
            match desired {
                Some(true) if !folder.paused => {
                    if !self.saved.owned_folders.contains(&folder.id) {
                        // Write ownership before the API call so a crash or lost reply
                        // cannot leave a successfully paused folder without recovery data.
                        self.saved.owned_folders.insert(folder.id.clone());
                        if let Err(error) = save(&self.saved) {
                            self.saved.owned_folders.remove(&folder.id);
                            return Err(error);
                        }
                    }
                    api.set_paused(&folder.id, true)?;
                    folder.paused = true;
                }
                Some(false) if self.saved.owned_folders.contains(&folder.id) => {
                    if folder.paused {
                        api.set_paused(&folder.id, false)?;
                        folder.paused = false;
                    }
                    self.saved.owned_folders.remove(&folder.id);
                    if let Err(error) = save(&self.saved) {
                        self.saved.owned_folders.insert(folder.id.clone());
                        return Err(error);
                    }
                }
                _ => {}
            }
        }
        Ok(folders)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    struct MockApi {
        folders: RefCell<Vec<Folder>>,
        fail_next_patch: Cell<bool>,
        patches: RefCell<Vec<(String, bool)>>,
    }

    impl MockApi {
        fn new() -> Self {
            Self {
                folders: RefCell::new(vec![
                    Folder {
                        id: "active".into(),
                        paused: false,
                    },
                    Folder {
                        id: "already-paused".into(),
                        paused: true,
                    },
                ]),
                fail_next_patch: Cell::new(false),
                patches: RefCell::new(vec![]),
            }
        }
    }

    impl FolderApi for MockApi {
        fn folders(&self) -> Result<Vec<Folder>, String> {
            Ok(self.folders.borrow().clone())
        }
        fn set_paused(&self, id: &str, paused: bool) -> Result<(), String> {
            if self.fail_next_patch.replace(false) {
                return Err("API unavailable".into());
            }
            self.patches.borrow_mut().push((id.into(), paused));
            self.folders
                .borrow_mut()
                .iter_mut()
                .find(|f| f.id == id)
                .unwrap()
                .paused = paused;
            Ok(())
        }
    }

    fn save(_: &SavedState) -> Result<(), String> {
        Ok(())
    }

    #[test]
    fn battery_pauses_and_ac_resumes_only_folders_we_owned() {
        let api = MockApi::new();
        let mut policy = Policy::new(SavedState::default());
        policy.observe_power(PowerSource::Battery);
        policy.reconcile(&api, &save, false).unwrap();
        assert_eq!(
            policy.saved.owned_folders,
            BTreeSet::from(["active".into()])
        );
        policy.observe_power(PowerSource::Ac);
        let folders = policy.reconcile(&api, &save, false).unwrap();
        assert!(!folders[0].paused);
        assert!(folders[1].paused);
        assert!(policy.saved.owned_folders.is_empty());
    }

    #[test]
    fn battery_resume_override_survives_duplicate_events_until_power_changes() {
        let api = MockApi::new();
        let mut policy = Policy::new(SavedState::default());
        policy.observe_power(PowerSource::Battery);
        policy.reconcile(&api, &save, false).unwrap();
        policy.set_paused(false);
        policy.observe_power(PowerSource::Battery);
        assert!(!policy.reconcile(&api, &save, false).unwrap()[0].paused);
        policy.observe_power(PowerSource::Ac);
        policy.observe_power(PowerSource::Battery);
        assert!(policy.reconcile(&api, &save, false).unwrap()[0].paused);
    }

    #[test]
    fn manual_pause_stays_paused_on_ac_and_auto_can_be_disabled() {
        let mut policy = Policy::new(SavedState::default());
        policy.observe_power(PowerSource::Ac);
        policy.set_paused(true);
        assert_eq!(policy.desired_pause(), Some(true));
        policy.set_auto_pause(false);
        assert_eq!(policy.desired_pause(), Some(true));
        policy.set_paused(false);
        policy.observe_power(PowerSource::Battery);
        assert_eq!(policy.desired_pause(), Some(false));
    }

    #[test]
    fn unknown_power_does_not_resume_automatically_paused_folders() {
        let api = MockApi::new();
        let mut policy = Policy::new(SavedState::default());
        policy.observe_power(PowerSource::Battery);
        policy.reconcile(&api, &save, false).unwrap();
        policy.observe_power(PowerSource::Unknown);
        assert_eq!(policy.desired_pause(), None);
        assert!(policy.reconcile(&api, &save, false).unwrap()[0].paused);
    }

    #[test]
    fn saved_ownership_recovers_after_restart_on_ac() {
        let api = MockApi::new();
        let mut policy = Policy::new(SavedState::default());
        policy.observe_power(PowerSource::Battery);
        policy.reconcile(&api, &save, false).unwrap();
        let saved: SavedState =
            serde_json::from_str(&serde_json::to_string(&policy.saved).unwrap()).unwrap();
        let mut restarted = Policy::new(saved);
        restarted.observe_power(PowerSource::Ac);
        let folders = restarted.reconcile(&api, &save, false).unwrap();
        assert!(!folders[0].paused);
        assert!(folders[1].paused);
    }

    #[test]
    fn lost_patch_reply_keeps_recovery_ownership_and_retries() {
        let api = MockApi::new();
        api.fail_next_patch.set(true);
        let mut policy = Policy::new(SavedState::default());
        policy.observe_power(PowerSource::Battery);
        assert!(policy.reconcile(&api, &save, false).is_err());
        assert!(policy.saved.owned_folders.contains("active"));
        policy.reconcile(&api, &save, false).unwrap();
        assert!(api.folders.borrow()[0].paused);
    }

    #[test]
    fn ownership_must_be_saved_before_pausing_and_survives_failed_resume() {
        let api = MockApi::new();
        let mut policy = Policy::new(SavedState::default());
        policy.observe_power(PowerSource::Battery);
        assert!(policy
            .reconcile(&api, &|_| Err("Disk full".into()), false)
            .is_err());
        assert!(api.patches.borrow().is_empty());
        assert!(policy.saved.owned_folders.is_empty());
        policy.reconcile(&api, &save, false).unwrap();
        api.fail_next_patch.set(true);
        policy.observe_power(PowerSource::Ac);
        assert!(policy.reconcile(&api, &save, false).is_err());
        assert!(policy.saved.owned_folders.contains("active"));
        policy.reconcile(&api, &save, false).unwrap();
        assert!(policy.saved.owned_folders.is_empty());
    }

    #[test]
    fn newly_added_folders_are_paused_and_exit_releases_only_our_pauses() {
        let api = MockApi::new();
        let mut policy = Policy::new(SavedState::default());
        policy.observe_power(PowerSource::Battery);
        policy.reconcile(&api, &save, false).unwrap();
        api.folders.borrow_mut().push(Folder {
            id: "new".into(),
            paused: false,
        });
        assert!(policy.reconcile(&api, &save, false).unwrap()[2].paused);
        let folders = policy.reconcile(&api, &save, true).unwrap();
        assert!(!folders[0].paused);
        assert!(folders[1].paused);
        assert!(!folders[2].paused);
    }

    #[test]
    fn disabling_automatic_pause_on_battery_restores_owned_folders() {
        let api = MockApi::new();
        let mut policy = Policy::new(SavedState::default());
        policy.observe_power(PowerSource::Battery);
        policy.reconcile(&api, &save, false).unwrap();
        policy.set_auto_pause(false);
        let folders = policy.reconcile(&api, &save, false).unwrap();
        assert!(!folders[0].paused);
        assert!(folders[1].paused);
        assert!(policy.saved.owned_folders.is_empty());
    }

    #[test]
    fn removing_a_paused_folder_prunes_its_saved_ownership() {
        let api = MockApi::new();
        let mut policy = Policy::new(SavedState::default());
        policy.observe_power(PowerSource::Battery);
        policy.reconcile(&api, &save, false).unwrap();
        api.folders
            .borrow_mut()
            .retain(|folder| folder.id != "active");
        policy.reconcile(&api, &save, false).unwrap();
        assert!(policy.saved.owned_folders.is_empty());
    }
}
