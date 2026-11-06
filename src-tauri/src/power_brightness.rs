//! Optional brightness preservation and bounded diagnostics for Lenovo mode changes.
use serde::{Deserialize, Serialize};
use std::{path::Path, thread, time::Duration};

trait ModeBackend {
    type Output;
    fn is_current(&self, mode: &str) -> Result<bool, String>;
    fn brightness(&self) -> Result<u8, String>;
    fn change_mode(&self, mode: &str) -> Result<Self::Output, String>;
    fn settle(&self);
    fn restore_brightness(&self, level: u8) -> Result<(), String>;
}

struct NativeModeBackend;
impl ModeBackend for NativeModeBackend {
    type Output = crate::lenovo_power::PowerModeStatus;
    fn is_current(&self, mode: &str) -> Result<bool, String> {
        Ok(crate::lenovo_power::read_power_mode()?.mode == mode)
    }
    fn brightness(&self) -> Result<u8, String> {
        lenovo_hdr_boost::current_brightness()
    }
    fn change_mode(&self, mode: &str) -> Result<Self::Output, String> {
        crate::lenovo_power::apply_power_mode(mode)
    }
    fn settle(&self) {
        thread::sleep(Duration::from_millis(500));
    }
    fn restore_brightness(&self, level: u8) -> Result<(), String> {
        lenovo_hdr_boost::set_panel_brightness(level)
    }
}

pub struct BrightnessKeeper {
    pub enabled: bool,
    pub message: Option<String>,
}

impl BrightnessKeeper {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            message: None,
        }
    }

    pub fn apply(&mut self, mode: &str) -> Result<crate::lenovo_power::PowerModeStatus, String> {
        self.apply_with(&NativeModeBackend, mode)
    }

    fn apply_with<B: ModeBackend>(&mut self, backend: &B, mode: &str) -> Result<B::Output, String> {
        if !self.enabled || backend.is_current(mode)? {
            return backend.change_mode(mode);
        }
        self.message = None;
        let brightness = match backend.brightness() {
            Ok(level) => Some(level),
            Err(error) => {
                self.message = Some(format!(
                    "Power mode can change, but brightness could not be read: {error}"
                ));
                None
            }
        };
        let result = backend.change_mode(mode);
        // Restore even if a service error followed a partial mode change. This
        // is one bounded restore, not a monitor that fights brightness keys.
        if let Some(level) = brightness {
            backend.settle();
            if backend.brightness() != Ok(level) {
                if let Err(error) = backend.restore_brightness(level) {
                    self.message = Some(format!(
                        "Power mode changed, but brightness could not be kept: {error}"
                    ));
                }
            }
        }
        result
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub mode: String,
    pub brightness: u8,
    pub on_battery: Option<bool>,
    pub windows_energy_saver: Option<bool>,
}

pub fn snapshot() -> Result<Snapshot, String> {
    #[cfg(target_os = "windows")]
    let windows_energy_saver = {
        use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
        let mut status = SYSTEM_POWER_STATUS::default();
        unsafe { GetSystemPowerStatus(&mut status) }
            .ok()
            .map(|_| status.SystemStatusFlag != 0)
    };
    #[cfg(not(target_os = "windows"))]
    let windows_energy_saver = None;
    Ok(Snapshot {
        mode: crate::lenovo_power::read_power_mode()?.mode,
        brightness: lenovo_hdr_boost::current_brightness()?,
        on_battery: crate::power_source::read_power().on_battery(),
        windows_energy_saver,
    })
}

fn print_sample(phase: &str) -> Result<Snapshot, String> {
    let sample = snapshot()?;
    println!("{}", serde_json::json!({"phase": phase, "state": sample}));
    Ok(sample)
}

struct RestoreGuard<'a> {
    before: Snapshot,
    journal: &'a Path,
    restored: bool,
}

impl RestoreGuard<'_> {
    fn restore(&mut self) -> Result<(), String> {
        crate::lenovo_power::apply_power_mode(&self.before.mode)?;
        thread::sleep(Duration::from_secs(1));
        lenovo_hdr_boost::set_panel_brightness(self.before.brightness)?;
        let after = snapshot()?;
        if after.mode != self.before.mode || after.brightness != self.before.brightness {
            return Err("The original power mode or brightness has not been restored yet.".into());
        }
        self.restored = true;
        std::fs::remove_file(self.journal).map_err(|e| e.to_string())?;
        println!("{}", serde_json::json!({"phase":"restored", "state":after}));
        Ok(())
    }
}

impl Drop for RestoreGuard<'_> {
    fn drop(&mut self) {
        if !self.restored {
            if let Err(error) = self.restore() {
                eprintln!(
                    "Brightness test recovery is pending: {error}. Recovery record: {}",
                    self.journal.display()
                );
            }
        }
    }
}

pub fn live_test(journal: &Path, recover: bool) -> Result<(), String> {
    let _lease = lenovo_hdr_boost::change_lease()?;
    if recover {
        let before = serde_json::from_slice(&std::fs::read(journal).map_err(|e| e.to_string())?)
            .map_err(|e| format!("Cannot read the test recovery record: {e}"))?;
        return RestoreGuard {
            before,
            journal,
            restored: false,
        }
        .restore();
    }
    if journal.exists() {
        return Err(
            "A brightness test recovery record already exists. Run the test with --recover first."
                .into(),
        );
    }
    let before = print_sample("before")?;
    if !matches!(before.mode.as_str(), "auto" | "cool" | "performance") {
        return Err("The current Lenovo mode is unknown.".into());
    }
    crate::state_file::write_atomic(journal, &before).map_err(|e| e.to_string())?;
    let mut guard = RestoreGuard {
        before: before.clone(),
        journal,
        restored: false,
    };
    // Always enter Saver from Auto so the before/after brightness is comparable.
    crate::lenovo_power::apply_power_mode("auto")?;
    thread::sleep(Duration::from_secs(1));
    lenovo_hdr_boost::set_panel_brightness(before.brightness)?;
    print_sample("auto_baseline")?;
    crate::lenovo_power::apply_power_mode("cool")?;
    for _ in 0..4 {
        thread::sleep(Duration::from_millis(500));
        print_sample("saver_without_preservation")?;
    }
    lenovo_hdr_boost::set_panel_brightness(before.brightness)?;
    let mut maintained = true;
    for _ in 0..4 {
        thread::sleep(Duration::from_millis(500));
        let sample = print_sample("saver_after_brightness_restore")?;
        maintained &= sample.brightness == before.brightness && sample.mode == "cool";
    }
    let mut keeper = BrightnessKeeper::new(true);
    for mode in ["auto", "cool"] {
        keeper.apply(mode)?;
        if let Some(error) = &keeper.message {
            return Err(error.clone());
        }
        for _ in 0..4 {
            thread::sleep(Duration::from_millis(250));
            let sample = print_sample("production_brightness_keeper")?;
            maintained &= sample.brightness == before.brightness && sample.mode == mode;
        }
    }
    guard.restore()?;
    if !maintained {
        return Err(
            "Brightness did not remain at the starting level while Saver was active.".into(),
        );
    }
    println!(
        "PASS: original brightness maintained in Saver; starting mode and brightness restored."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    struct Backend {
        mode: RefCell<String>,
        level: Cell<u8>,
        writes: RefCell<Vec<u8>>,
        reads: Cell<usize>,
        read_fails: Cell<bool>,
        write_fails: Cell<bool>,
        mode_fails: Cell<bool>,
    }
    impl Backend {
        fn new() -> Self {
            Self {
                mode: RefCell::new("auto".into()),
                level: Cell::new(54),
                writes: RefCell::new(vec![]),
                reads: Cell::new(0),
                read_fails: Cell::new(false),
                write_fails: Cell::new(false),
                mode_fails: Cell::new(false),
            }
        }
    }
    impl ModeBackend for Backend {
        type Output = String;
        fn is_current(&self, mode: &str) -> Result<bool, String> {
            Ok(*self.mode.borrow() == mode)
        }
        fn brightness(&self) -> Result<u8, String> {
            self.reads.set(self.reads.get() + 1);
            if self.read_fails.get() {
                Err("Panel unavailable".into())
            } else {
                Ok(self.level.get())
            }
        }
        fn change_mode(&self, mode: &str) -> Result<String, String> {
            if *self.mode.borrow() != mode {
                *self.mode.borrow_mut() = mode.into();
                self.level.set(22);
            }
            if self.mode_fails.get() {
                Err("Service did not confirm".into())
            } else {
                Ok(mode.into())
            }
        }
        fn settle(&self) {}
        fn restore_brightness(&self, level: u8) -> Result<(), String> {
            self.writes.borrow_mut().push(level);
            if self.write_fails.get() {
                Err("Panel rejected brightness".into())
            } else {
                self.level.set(level);
                Ok(())
            }
        }
    }

    #[test]
    fn preserves_brightness_once_and_later_manual_brightness_is_left_alone() {
        let backend = Backend::new();
        let mut keeper = BrightnessKeeper::new(true);
        assert_eq!(keeper.apply_with(&backend, "cool").unwrap(), "cool");
        assert_eq!(backend.level.get(), 54);
        assert_eq!(*backend.writes.borrow(), vec![54]);
        backend.level.set(40);
        keeper.apply_with(&backend, "cool").unwrap();
        assert_eq!(backend.level.get(), 40);
        assert_eq!(backend.writes.borrow().len(), 1);
    }

    #[test]
    fn disabled_option_preserves_lenovos_normal_dimming_behavior() {
        let backend = Backend::new();
        BrightnessKeeper::new(false)
            .apply_with(&backend, "cool")
            .unwrap();
        assert_eq!(backend.level.get(), 22);
        assert!(backend.writes.borrow().is_empty());
        assert_eq!(backend.reads.get(), 0);
    }

    #[test]
    fn brightness_read_or_write_failure_does_not_prevent_saver_mode() {
        for read_fails in [true, false] {
            let backend = Backend::new();
            backend.read_fails.set(read_fails);
            backend.write_fails.set(!read_fails);
            let mut keeper = BrightnessKeeper::new(true);
            assert_eq!(keeper.apply_with(&backend, "cool").unwrap(), "cool");
            assert_eq!(backend.mode.borrow().as_str(), "cool");
            assert!(keeper.message.is_some());
        }
    }

    #[test]
    fn partial_mode_failure_still_restores_brightness_and_returns_mode_error() {
        let backend = Backend::new();
        backend.mode_fails.set(true);
        assert!(BrightnessKeeper::new(true)
            .apply_with(&backend, "cool")
            .is_err());
        assert_eq!(backend.level.get(), 54);
    }
}
