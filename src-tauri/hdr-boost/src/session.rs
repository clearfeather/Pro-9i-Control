use crate::{brightness, display, renderer, validate_gain, Signals, DEFAULT_STRENGTH};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::{atomic::Ordering, Arc, Condvar, Mutex},
    thread,
    time::Duration,
};
use windows::{
    core::w,
    Win32::{
        Foundation::*,
        System::{Registry::*, Threading::CreateMutexW},
    },
};

const PANEL: &str = "Built-in Mini-LED Display";
const BOOST_REFRESH: (u32, u32) = (60, 1);

/// Shares the prototype's session name: either executable can own the panel.
pub struct DisplayLease(HANDLE);
impl DisplayLease {
    fn acquire() -> Result<Self, String> {
        let handle = unsafe { CreateMutexW(None, false, w!("Local\\LumaLiftHDRSession")) }
            .map_err(|e| e.to_string())?;
        if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
            unsafe {
                let _ = CloseHandle(handle);
            }
            return Err("HDR Boost or LumaLift is using the display. Stop it before changing display settings.".into());
        }
        Ok(Self(handle))
    }
}
impl Drop for DisplayLease {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

pub fn recovery_path() -> Result<PathBuf, String> {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|p| p.join("HZHDR").join("hdr-boost-recovery.json"))
        .ok_or_else(|| "LOCALAPPDATA is unavailable; cannot save HDR Boost recovery".into())
}

pub fn change_lease() -> Result<DisplayLease, String> {
    let lease = DisplayLease::acquire()?;
    if recovery_path()?.exists() || prototype_recovery_exists() {
        return Err(
            "Restore the saved HDR Boost/LumaLift session before changing display settings.".into(),
        );
    }
    Ok(lease)
}

fn prototype_recovery_exists() -> bool {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .is_some_and(|p| p.join("LumaLift/restore-state.txt").exists())
}

fn bios_string(name: &str) -> Result<String, String> {
    let name: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    let mut buffer = [0_u16; 256];
    let mut bytes = std::mem::size_of_val(&buffer) as u32;
    unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            w!("HARDWARE\\DESCRIPTION\\System\\BIOS"),
            windows::core::PCWSTR(name.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
        .ok()
        .map_err(|e| e.to_string())?;
    }
    Ok(String::from_utf16_lossy(
        &buffer[..buffer.iter().position(|x| *x == 0).unwrap_or(buffer.len())],
    ))
}

fn target(device: Option<&str>) -> Result<display::DisplayInfo, String> {
    if !bios_string("SystemManufacturer")?.eq_ignore_ascii_case("LENOVO")
        || bios_string("SystemProductName")? != "83BV"
    {
        return Err("HDR Boost is configured only for this Lenovo 83BV laptop.".into());
    }
    let displays = display::active_displays()?;
    let candidate = displays
        .iter()
        .find(|d| {
            d.built_in
                && d.name == PANEL
                && d.hdr_supported
                && !d.advanced_color_force_disabled
                && device.is_none_or(|name| d.gdi_device_name == name)
        })
        .ok_or_else(|| "The Lenovo built-in mini-LED HDR panel is not active.".to_string())?;
    // A cloned desktop cannot be isolated to this panel by a monitor capture.
    if displays
        .iter()
        .filter(|d| d.gdi_device_name == candidate.gdi_device_name)
        .count()
        != 1
    {
        return Err("Use Extend displays before enabling HDR Boost.".into());
    }
    Ok(candidate.clone())
}

pub fn diagnose() -> Result<String, String> {
    let d = target(None)?;
    Ok(format!(
        "Lenovo 83BV: {} ({}) HDR={} SDR-white={:?} brightness={}%, refresh={}/{}",
        d.name,
        d.gdi_device_name,
        d.hdr_enabled,
        d.sdr_white_nits,
        brightness::current()?,
        d.refresh.0,
        d.refresh.1
    ))
}

pub fn verify_boost_display() -> Result<(), String> {
    check_boost_display(&target(None)?)
}

fn check_boost_display(current: &display::DisplayInfo) -> Result<(), String> {
    if current.hdr_enabled && display::nominal_refresh(current.refresh) == Some(60) {
        Ok(())
    } else {
        Err("HDR Boost requires HDR at 60 Hz; Windows did not apply the requested settings".into())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RestorePoint {
    version: u32,
    device_path: String,
    hdr_enabled: bool,
    sdr_white_nits: u32,
    // Version 3 adds the exact refresh fraction. Older records never owned refresh.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    refresh: Option<(u32, u32)>,
    // Only version 1 owned the backlight. Never restore an old brightness for
    // newer sessions over the user's current choice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    brightness: Option<u8>,
}

trait DisplaySettings {
    fn read(&self) -> Result<display::DisplayInfo, String>;
    fn hdr(&self, d: &display::DisplayInfo, enabled: bool) -> Result<(), String>;
    fn white(&self, d: &display::DisplayInfo, nits: u32) -> Result<(), String>;
    fn refresh(&self, d: &display::DisplayInfo, refresh: (u32, u32)) -> Result<(), String>;
}

struct SystemDisplay;
impl DisplaySettings for SystemDisplay {
    fn read(&self) -> Result<display::DisplayInfo, String> {
        target(None)
    }
    fn hdr(&self, d: &display::DisplayInfo, enabled: bool) -> Result<(), String> {
        display::set_hdr(d, enabled)?;
        thread::sleep(Duration::from_millis(1000));
        Ok(())
    }
    fn white(&self, d: &display::DisplayInfo, nits: u32) -> Result<(), String> {
        display::set_sdr_white(d, nits)
    }
    fn refresh(&self, d: &display::DisplayInfo, refresh: (u32, u32)) -> Result<(), String> {
        display::set_refresh(d, refresh)?;
        thread::sleep(Duration::from_millis(1000));
        Ok(())
    }
}

impl RestorePoint {
    fn capture(original: &display::DisplayInfo) -> Result<Self, String> {
        if original.refresh.0 == 0 || original.refresh.1 == 0 {
            return Err("Cannot read the original refresh rate for recovery".into());
        }
        Ok(Self {
            version: 3,
            device_path: original.device_path.clone(),
            hdr_enabled: original.hdr_enabled,
            sdr_white_nits: original
                .sdr_white_nits
                .ok_or("Cannot read SDR white for recovery")?,
            refresh: Some(original.refresh),
            brightness: None,
        })
    }

    fn save(&self, path: &Path) -> Result<(), String> {
        fs::create_dir_all(path.parent().ok_or("Invalid recovery path")?)
            .map_err(|e| e.to_string())?;
        // Never overwrite a recovery record, including a damaged one. Flush to disk
        // before the first display write; a partial record also blocks new sessions.
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| e.to_string())?;
        file.write_all(&serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        file.sync_all().map_err(|e| e.to_string())
    }
    fn load(path: &Path) -> Result<Self, String> {
        let point: Self = serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| {
                format!(
                    "Recovery record is invalid; retained at {}: {e}",
                    path.display()
                )
            })?;
        if !matches!(point.version, 1..=3)
            || point.device_path.is_empty()
            || point.brightness.is_some_and(|level| level > 100)
            || (point.version == 1 && point.brightness.is_none())
            || (point.version == 3 && point.refresh.is_none())
            || (point.version < 3 && point.refresh.is_some())
            || point.refresh.is_some_and(|(n, d)| n == 0 || d == 0)
        {
            return Err("Unsupported or invalid recovery record; it has been retained.".into());
        }
        Ok(point)
    }
    fn apply(&self) -> Result<(), String> {
        self.apply_with(&SystemDisplay)
    }

    fn matches(&self, actual: &display::DisplayInfo) -> bool {
        actual.device_path.eq_ignore_ascii_case(&self.device_path)
            && actual.hdr_enabled == self.hdr_enabled
            && actual.sdr_white_nits == Some(self.sdr_white_nits)
            && self
                .refresh
                .is_none_or(|rate| display::refresh_matches(actual.refresh, rate))
    }

    fn apply_with(&self, settings: &impl DisplaySettings) -> Result<(), String> {
        let mut d = settings.read()?;
        if !d.device_path.eq_ignore_ascii_case(&self.device_path) {
            return Err("The saved built-in panel is disconnected; recovery retained.".into());
        }
        let mut errors = Vec::new();
        // Attempt every independent restoration step, even when an earlier one fails.
        if d.sdr_white_nits != Some(self.sdr_white_nits) {
            if !d.hdr_enabled {
                collect(&mut errors, settings.hdr(&d, true));
            }
            collect(&mut errors, settings.white(&d, self.sdr_white_nits));
        }
        if let Ok(current) = settings.read() {
            d = current;
        }
        if d.hdr_enabled != self.hdr_enabled {
            collect(&mut errors, settings.hdr(&d, self.hdr_enabled));
        }
        // Restore timing after HDR: switching dynamic range can change the mode.
        // Read fresh adapter/target IDs instead of replaying an old topology.
        if let Some(refresh) = self.refresh {
            match settings.read() {
                Ok(current) if current.device_path.eq_ignore_ascii_case(&self.device_path) => {
                    if !display::refresh_matches(current.refresh, refresh) {
                        collect(&mut errors, settings.refresh(&current, refresh));
                    }
                }
                Ok(_) => errors.push("The saved panel changed during recovery".into()),
                Err(error) => errors.push(error),
            }
        }
        collect(
            &mut errors,
            self.restore_legacy_brightness(brightness::set, brightness::current),
        );
        match settings.read() {
            Ok(actual) if self.matches(&actual) => {}
            Ok(actual) => errors.push(format!(
                "Restore read-back differs: HDR={}, SDR white={:?}, refresh={:?}; expected HDR={}, white={}, refresh={:?}",
                actual.hdr_enabled, actual.sdr_white_nits, actual.refresh,
                self.hdr_enabled, self.sdr_white_nits, self.refresh
            )),
            Err(error) => errors.push(format!("Cannot verify the restored display: {error}")),
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("; "))
        }
    }

    fn restore_legacy_brightness(
        &self,
        write: impl FnOnce(u8) -> Result<(), String>,
        read: impl FnOnce() -> Result<u8, String>,
    ) -> Result<(), String> {
        if self.version == 1 {
            let original = self
                .brightness
                .ok_or("Legacy recovery is missing brightness")?;
            write(original)?;
            if read()? != original {
                return Err(format!("Legacy brightness did not restore to {original}%"));
            }
        }
        Ok(())
    }
}
fn collect(errors: &mut Vec<String>, result: Result<(), String>) {
    if let Err(e) = result {
        errors.push(e);
    }
}

fn clear_after_restore(path: &Path, result: Result<(), String>) -> Result<(), String> {
    result?;
    fs::remove_file(path).map_err(|e| e.to_string())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scenario {
    #[default]
    Live,
    Idle,
    Battery,
    BatterySaver,
    PowerTransition,
    DisplayChange,
    Suspend,
    Emergency,
    Failure,
    Panic,
    PresentationStall,
    CaptureStall,
}

#[derive(Clone, Debug, Serialize)]
pub struct Status {
    pub phase: String,
    pub device_name: Option<String>,
    pub available_device: Option<String>,
    pub strength: f32,
    pub recovery_pending: bool,
    pub message: String,
}
impl Status {
    pub fn busy(&self) -> bool {
        matches!(
            self.phase.as_str(),
            "starting" | "running" | "stopping" | "restoring"
        )
    }
}

struct Shared {
    state: Mutex<Status>,
    done: Condvar,
    signals: Mutex<Arc<Signals>>,
    path: PathBuf,
}

#[derive(Clone)]
pub struct Controller(Arc<Shared>);
impl Controller {
    pub fn new() -> Result<Self, String> {
        let path = recovery_path()?;
        let pending = path.exists();
        Ok(Self(Arc::new(Shared {
            state: Mutex::new(Status {
                phase: "off".into(),
                device_name: None,
                available_device: None,
                strength: DEFAULT_STRENGTH,
                recovery_pending: pending,
                message: if pending {
                    "Saved display settings need recovery.".into()
                } else {
                    "Off. Uses your current Windows brightness.".into()
                },
            }),
            done: Condvar::new(),
            signals: Mutex::new(Arc::new(Signals::new(DEFAULT_STRENGTH))),
            path,
        })))
    }
    pub fn status(&self) -> Status {
        self.0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub fn refresh_availability(&self) -> Status {
        let candidate = target(None);
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        state.available_device = candidate.ok().map(|d| d.gdi_device_name);
        if !state.busy() && state.recovery_pending && !self.0.path.exists() {
            state.recovery_pending = false;
            state.phase = "off".into();
            state.message = "Off. Original display settings restored.".into();
        }
        state.clone()
    }
    fn phase(&self, phase: &str, message: &str) {
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        state.phase = phase.into();
        state.message = message.into();
    }
    pub fn set_strength(&self, strength: f32) -> Result<Status, String> {
        validate_gain(strength)?;
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        state.strength = strength;
        self.0
            .signals
            .lock()
            .unwrap()
            .gain
            .store(strength.to_bits(), Ordering::SeqCst);
        Ok(state.clone())
    }
    pub fn stop(&self) -> Status {
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        self.0.signals.lock().unwrap().stop();
        if state.busy() {
            state.phase = "stopping".into();
            state.message = "Stopping and restoring display settings…".into();
        }
        state.clone()
    }
    /// Used only on a blocking task, never the UI thread.
    pub fn wait(&self) -> Status {
        let state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        self.0
            .done
            .wait_while(state, |s| s.busy())
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub fn start(
        &self,
        device: String,
        strength: f32,
        self_test: bool,
        scenario: Scenario,
    ) -> Result<Status, String> {
        validate_gain(strength)?;
        self.launch(Some(device), strength, self_test, scenario)
    }
    pub fn recover(&self) -> Result<Status, String> {
        self.launch(None, DEFAULT_STRENGTH, false, Scenario::Live)
    }
    fn launch(
        &self,
        device: Option<String>,
        strength: f32,
        self_test: bool,
        scenario: Scenario,
    ) -> Result<Status, String> {
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.busy() {
            return Err("HDR Boost is already starting, running, or restoring.".into());
        }
        if device.is_some() && self.0.path.exists() {
            return Err("Restore the saved session first.".into());
        }
        *self.0.signals.lock().unwrap() = Arc::new(Signals::new(strength));
        state.phase = if device.is_some() {
            "starting"
        } else {
            "restoring"
        }
        .into();
        state.device_name = device.clone();
        state.strength = strength;
        state.message = "Preparing display session…".into();
        let this = self.clone();
        if let Err(e) = thread::Builder::new()
            .name("lenovo-hdr-boost".into())
            .spawn(move || {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    this.execute(device, self_test, scenario)
                }))
                .unwrap_or_else(|_| {
                    Err("HDR Boost worker panicked; recovery record retained.".into())
                });
                let mut state = this.0.state.lock().unwrap_or_else(|e| e.into_inner());
                state.recovery_pending = this.0.path.exists();
                state.phase = if state.recovery_pending {
                    "recovery"
                } else {
                    "off"
                }
                .into();
                state.device_name = None;
                state.message = result
                    .err()
                    .unwrap_or_else(|| "Off. Original display settings restored.".into());
                this.0.done.notify_all();
            })
        {
            state.phase = "off".into();
            return Err(e.to_string());
        }
        Ok(state.clone())
    }
    fn execute(
        &self,
        device: Option<String>,
        self_test: bool,
        scenario: Scenario,
    ) -> Result<(), String> {
        let _lease = DisplayLease::acquire()?;
        let Some(device) = device else {
            if self.0.path.exists() {
                clear_after_restore(&self.0.path, RestorePoint::load(&self.0.path)?.apply())?;
            }
            return Ok(());
        };
        if prototype_recovery_exists() {
            return Err(
                "LumaLift has saved recovery data. Run its --restore command first.".into(),
            );
        }
        crate::desktop::ensure_active()?;
        let original = target(Some(&device))?;
        let restore = RestorePoint::capture(&original)?;
        restore.save(&self.0.path)?;
        let signals = self.0.signals.lock().unwrap().clone();
        let rendering =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<(), String> {
                if !original.hdr_enabled {
                    display::set_hdr(&original, true)?;
                    for _ in 0..12 {
                        thread::sleep(Duration::from_millis(100));
                        if signals.stopped() {
                            return Ok(());
                        }
                    }
                }
                if signals.stopped() {
                    return Ok(());
                }
                let mut current = target(Some(&device))?;
                if display::nominal_refresh(current.refresh) != Some(60) {
                    display::set_refresh(&current, BOOST_REFRESH)?;
                    for _ in 0..12 {
                        thread::sleep(Duration::from_millis(100));
                        if signals.stopped() {
                            return Ok(());
                        }
                    }
                    current = target(Some(&device))?;
                }
                check_boost_display(&current)?;
                renderer::run(
                    &current,
                    self_test,
                    scenario,
                    &signals,
                    || {
                        self.phase(
                            "running",
                            "Boost active · 60 Hz. Ctrl+Alt+B stops and restores.",
                        )
                    },
                    |message| {
                        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
                        if state.phase == "running" {
                            state.message = message.into();
                        }
                    },
                )
            }))
            .unwrap_or_else(|_| Err("HDR renderer panicked".into()));
        self.phase("restoring", "Restoring original HDR and refresh rate…");
        let restoration = clear_after_restore(&self.0.path, restore.apply());
        match (rendering, restoration) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(e), Ok(())) => Err(format!("{e}. Original settings restored.")),
            (rendering, Err(e)) => Err(format!(
                "{}Restore needs attention: {e}. Recovery retained at {}",
                rendering
                    .err()
                    .map(|e| format!("{e}; "))
                    .unwrap_or_default(),
                self.0.path.display()
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_nonfinite_or_out_of_bounds_gain() {
        for gain in [f32::NAN, f32::INFINITY, 0.99, 2.5001, 3.0, 8.0] {
            assert!(validate_gain(gain).is_err());
        }
        for gain in [1.0, 1.5, 2.4999, 2.5] {
            assert!(validate_gain(gain).is_ok());
        }
    }
    #[test]
    fn recovery_never_overwrites_existing_or_corrupt_record() {
        let folder =
            std::env::temp_dir().join(format!("hzhdr-recovery-test-{}", std::process::id()));
        fs::create_dir_all(&folder).unwrap();
        let file = folder.join("recovery.json");
        fs::write(&file, b"broken record").unwrap();
        let point = RestorePoint {
            version: 1,
            device_path: "panel".into(),
            hdr_enabled: false,
            sdr_white_nits: 80,
            refresh: None,
            brightness: Some(40),
        };
        assert!(RestorePoint::load(&file).is_err());
        assert!(point.save(&file).is_err());
        assert_eq!(fs::read(&file).unwrap(), b"broken record");
        fs::remove_file(&file).unwrap();
        point.save(&file).unwrap();
        assert_eq!(RestorePoint::load(&file).unwrap().brightness, Some(40));
        fs::remove_file(&file).unwrap();
        fs::remove_dir(folder).unwrap();
    }
    #[test]
    fn independent_restore_errors_are_accumulated() {
        let mut errors = vec![];
        collect(&mut errors, Err("HDR".into()));
        collect(&mut errors, Ok(()));
        collect(&mut errors, Err("brightness".into()));
        assert_eq!(errors, ["HDR", "brightness"]);
    }

    #[test]
    fn failed_restore_retains_record_until_success() {
        let file = std::env::temp_dir().join(format!(
            "hzhdr-restore-retention-{}.json",
            std::process::id()
        ));
        fs::write(&file, b"original restore point").unwrap();
        assert!(clear_after_restore(&file, Err("panel disconnected".into())).is_err());
        assert_eq!(fs::read(&file).unwrap(), b"original restore point");
        clear_after_restore(&file, Ok(())).unwrap();
        assert!(!file.exists());
    }

    #[test]
    fn unsupported_recovery_data_is_rejected() {
        let file = std::env::temp_dir().join(format!(
            "hzhdr-invalid-recovery-{}.json",
            std::process::id()
        ));
        for data in [
            r#"{"version":4,"device_path":"panel","hdr_enabled":false,"sdr_white_nits":80,"brightness":40}"#,
            r#"{"version":1,"device_path":"panel","hdr_enabled":false,"sdr_white_nits":80,"brightness":101}"#,
            r#"{"version":1,"device_path":"panel","hdr_enabled":false,"sdr_white_nits":80}"#,
            r#"{"version":3,"device_path":"panel","hdr_enabled":false,"sdr_white_nits":80}"#,
            r#"{"version":3,"device_path":"panel","hdr_enabled":false,"sdr_white_nits":80,"refresh":[0,1]}"#,
            r#"{"version":3,"device_path":"panel","hdr_enabled":false,"sdr_white_nits":80,"refresh":[165,0]}"#,
            r#"{"version":2,"device_path":"panel","hdr_enabled":false,"sdr_white_nits":80,"refresh":[165,1]}"#,
        ] {
            fs::write(&file, data).unwrap();
            assert!(RestorePoint::load(&file).is_err());
            assert!(file.exists());
        }
        fs::remove_file(file).unwrap();
    }

    #[test]
    fn content_only_recovery_never_reads_or_writes_backlight() {
        for (version, brightness) in [(2, None), (2, Some(40)), (3, None)] {
            let point = RestorePoint {
                version,
                device_path: "panel".into(),
                hdr_enabled: false,
                sdr_white_nits: 80,
                refresh: (version == 3).then_some((165, 1)),
                brightness,
            };
            point
                .restore_legacy_brightness(
                    |_| panic!("New sessions must not overwrite a user's brightness change"),
                    || panic!("New session recovery must not depend on brightness access"),
                )
                .unwrap();
        }
    }

    #[test]
    fn legacy_recovery_restores_saved_brightness() {
        let point: RestorePoint = serde_json::from_str(
            r#"{"version":1,"device_path":"panel","hdr_enabled":false,"sdr_white_nits":80,"brightness":40}"#,
        ).unwrap();
        let level = std::cell::Cell::new(100);
        point
            .restore_legacy_brightness(
                |original| {
                    level.set(original);
                    Ok(())
                },
                || Ok(level.get()),
            )
            .unwrap();
        assert_eq!(level.get(), 40);
    }

    #[test]
    fn content_only_recovery_record_has_no_brightness_to_restore() {
        let point = RestorePoint {
            version: 2,
            device_path: "panel".into(),
            hdr_enabled: false,
            sdr_white_nits: 80,
            refresh: None,
            brightness: None,
        };
        let json = serde_json::to_value(&point).unwrap();
        assert!(json.get("brightness").is_none());
        let loaded: RestorePoint = serde_json::from_value(json).unwrap();
        assert_eq!(loaded.version, 2);
        assert_eq!(loaded.brightness, None);
    }

    fn panel(hdr_enabled: bool, refresh: (u32, u32)) -> display::DisplayInfo {
        display::DisplayInfo {
            adapter_id: LUID::default(),
            target_id: 0,
            name: PANEL.into(),
            device_path: "panel".into(),
            built_in: true,
            refresh,
            gdi_device_name: "display".into(),
            hdr_supported: true,
            hdr_enabled,
            advanced_color_force_disabled: false,
            sdr_white_nits: Some(80),
        }
    }

    struct TestDisplay {
        state: std::cell::RefCell<display::DisplayInfo>,
        writes: std::cell::RefCell<Vec<String>>,
        fail_hdr: std::cell::Cell<bool>,
        fail_refresh: std::cell::Cell<bool>,
        ignore_refresh: std::cell::Cell<bool>,
    }
    impl TestDisplay {
        fn boosted() -> Self {
            Self {
                state: std::cell::RefCell::new(panel(true, BOOST_REFRESH)),
                writes: Default::default(),
                fail_hdr: Default::default(),
                fail_refresh: Default::default(),
                ignore_refresh: Default::default(),
            }
        }
    }
    impl DisplaySettings for TestDisplay {
        fn read(&self) -> Result<display::DisplayInfo, String> {
            Ok(self.state.borrow().clone())
        }
        fn hdr(&self, _: &display::DisplayInfo, enabled: bool) -> Result<(), String> {
            self.writes.borrow_mut().push(format!("hdr:{enabled}"));
            if self.fail_hdr.get() {
                return Err("injected HDR failure".into());
            }
            let mut state = self.state.borrow_mut();
            state.hdr_enabled = enabled;
            // Model a driver choosing a different refresh rate during an HDR switch.
            state.refresh = BOOST_REFRESH;
            Ok(())
        }
        fn white(&self, _: &display::DisplayInfo, nits: u32) -> Result<(), String> {
            self.writes.borrow_mut().push(format!("white:{nits}"));
            self.state.borrow_mut().sdr_white_nits = Some(nits);
            Ok(())
        }
        fn refresh(&self, _: &display::DisplayInfo, refresh: (u32, u32)) -> Result<(), String> {
            self.writes
                .borrow_mut()
                .push(format!("refresh:{refresh:?}"));
            if self.fail_refresh.get() {
                return Err("injected refresh failure".into());
            }
            if !self.ignore_refresh.get() {
                self.state.borrow_mut().refresh = refresh;
            }
            Ok(())
        }
    }

    #[test]
    fn new_recovery_saves_exact_refresh_and_original_hdr_before_changes() {
        for hdr in [false, true] {
            let original = panel(hdr, (120_000, 1001));
            let saved = RestorePoint::capture(&original).unwrap();
            let json = serde_json::to_value(&saved).unwrap();
            assert_eq!(json["refresh"], serde_json::json!([120_000, 1001]));
            assert!(json.get("brightness").is_none());
            let saved: RestorePoint = serde_json::from_value(json).unwrap();
            assert_eq!(saved.version, 3);
            assert!(saved.matches(&original));
        }
        for refresh in [(0, 1), (165, 0)] {
            assert!(RestorePoint::capture(&panel(false, refresh)).is_err());
        }
    }

    #[test]
    fn restore_returns_to_original_hdr_and_exact_refresh_after_hdr_switch() {
        for (hdr, refresh) in [(false, (165, 1)), (true, (120_000, 1001)), (false, (60, 1))] {
            let saved = RestorePoint::capture(&panel(hdr, refresh)).unwrap();
            let display = TestDisplay::boosted();
            display.state.borrow_mut().sdr_white_nits = Some(120);
            saved.apply_with(&display).unwrap();
            assert!(saved.matches(&display.read().unwrap()));
            let writes = display.writes.borrow();
            assert_eq!(writes[0], "white:80");
            if !hdr && refresh != BOOST_REFRESH {
                assert_eq!(writes[1], "hdr:false");
                assert_eq!(writes[2], format!("refresh:{refresh:?}"));
            }
            if refresh == BOOST_REFRESH {
                assert!(!writes.iter().any(|w| w.starts_with("refresh:")));
            }
        }
    }

    #[test]
    fn refresh_restore_failure_or_incorrect_readback_retains_recovery_for_retry() {
        let saved = RestorePoint::capture(&panel(false, (165, 1))).unwrap();
        let file =
            std::env::temp_dir().join(format!("hzhdr-refresh-retry-{}.json", std::process::id()));
        saved.save(&file).unwrap();
        let display = TestDisplay::boosted();
        display.fail_refresh.set(true);
        let error = clear_after_restore(&file, saved.apply_with(&display)).unwrap_err();
        assert!(error.contains("injected refresh failure"));
        assert!(!display.read().unwrap().hdr_enabled);
        assert_eq!(RestorePoint::load(&file).unwrap().refresh, Some((165, 1)));
        display.fail_refresh.set(false);
        display.ignore_refresh.set(true);
        let error = clear_after_restore(&file, saved.apply_with(&display)).unwrap_err();
        assert!(error.contains("Restore read-back differs"));
        assert!(file.exists());
        display.ignore_refresh.set(false);
        clear_after_restore(&file, saved.apply_with(&display)).unwrap();
        assert!(!file.exists());
        assert!(saved.matches(&display.read().unwrap()));
    }

    #[test]
    fn hdr_failure_still_attempts_refresh_restore() {
        let saved = RestorePoint::capture(&panel(false, (165, 1))).unwrap();
        let display = TestDisplay::boosted();
        display.fail_hdr.set(true);
        assert!(saved
            .apply_with(&display)
            .unwrap_err()
            .contains("injected HDR failure"));
        assert_eq!(display.read().unwrap().refresh, (165, 1));
    }

    #[test]
    fn legacy_recovery_leaves_refresh_rate_alone() {
        let saved: RestorePoint = serde_json::from_str(
            r#"{"version":2,"device_path":"panel","hdr_enabled":true,"sdr_white_nits":80}"#,
        )
        .unwrap();
        let display = TestDisplay::boosted();
        saved.apply_with(&display).unwrap();
        assert!(display.writes.borrow().is_empty());
        assert_eq!(display.read().unwrap().refresh, BOOST_REFRESH);
    }

    #[test]
    fn boost_requires_both_hdr_and_60_hz() {
        assert!(check_boost_display(&panel(true, (60_000, 1000))).is_ok());
        assert!(check_boost_display(&panel(true, (60_000, 1001))).is_ok());
        assert!(check_boost_display(&panel(false, (60, 1))).is_err());
        assert!(check_boost_display(&panel(true, (165, 1))).is_err());
    }
}
