use crate::power_source::PowerSource;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Restore<T> {
    pub before: T,
    pub target: T,
    #[serde(default)]
    pub confirmed: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, bound(deserialize = "T: Deserialize<'de>"))]
pub struct SavedControl<T> {
    pub enabled: bool,
    pub restore: Option<Restore<T>>,
}

impl<T> Default for SavedControl<T> {
    fn default() -> Self {
        Self {
            enabled: true,
            restore: None,
        }
    }
}

pub trait Setting<T> {
    fn read(&self) -> Result<T, String>;
    fn target(&self, current: &T) -> T;
    fn apply(&self, value: &T) -> Result<(), String>;
    fn compatible(&self, _current: &T, _saved: &T) -> bool {
        true
    }
}

pub struct Control<T> {
    pub saved: SavedControl<T>,
    pub overridden: bool,
    pub power: PowerSource,
    last_power: Option<PowerSource>,
    expected: Option<T>,
}

impl<T: Clone + PartialEq> Control<T> {
    pub fn new(saved: SavedControl<T>) -> Self {
        Self {
            saved,
            overridden: false,
            power: PowerSource::Unknown,
            last_power: None,
            expected: None,
        }
    }

    pub fn observe_power(&mut self, power: PowerSource) {
        self.power = power;
        if power != PowerSource::Unknown && self.last_power != Some(power) {
            self.last_power = Some(power);
            self.overridden = false;
            self.expected = None;
        }
    }

    fn commit(
        &mut self,
        next: SavedControl<T>,
        save: &impl Fn(&SavedControl<T>) -> Result<(), String>,
    ) -> Result<(), String> {
        save(&next)?;
        self.saved = next;
        Ok(())
    }

    pub fn set_enabled(
        &mut self,
        enabled: bool,
        save: &impl Fn(&SavedControl<T>) -> Result<(), String>,
    ) -> Result<(), String> {
        let mut next = self.saved.clone();
        next.enabled = enabled;
        self.commit(next, save)?;
        self.overridden = false;
        self.expected = None;
        Ok(())
    }

    pub fn manual_changed(
        &mut self,
        save: &impl Fn(&SavedControl<T>) -> Result<(), String>,
    ) -> Result<(), String> {
        self.overridden = self.saved.enabled && self.power == PowerSource::Battery;
        self.expected = None;
        let mut next = self.saved.clone();
        next.restore = None;
        self.commit(next, save)
    }

    /// Returns whether the battery target is active. A write-ahead record
    /// distinguishes a failed automatic change from a later manual choice.
    pub fn reconcile(
        &mut self,
        hardware: &impl Setting<T>,
        save: &impl Fn(&SavedControl<T>) -> Result<(), String>,
        release: bool,
    ) -> Result<bool, String> {
        if !release && self.power == PowerSource::Unknown && self.saved.enabled {
            return Ok(false);
        }
        let apply_battery = !release
            && self.saved.enabled
            && !self.overridden
            && self.power == PowerSource::Battery;
        if !apply_battery {
            if let Some(record) = self.saved.restore.clone() {
                let current = hardware.read()?;
                if !hardware.compatible(&current, &record.before) {
                    return Err("Waiting for the original laptop display to reconnect.".into());
                }
                if current == record.target && current != record.before {
                    hardware.apply(&record.before)?;
                    if hardware.read()? != record.before {
                        return Err("The previous setting has not been restored yet.".into());
                    }
                }
                // A setting changed outside the automation belongs to the user.
                let mut next = self.saved.clone();
                next.restore = None;
                self.commit(next, save)?;
            }
            self.expected = None;
            return Ok(false);
        }

        let current = hardware.read()?;
        if let Some(record) = self.saved.restore.clone() {
            if !hardware.compatible(&current, &record.before) {
                return Err("Waiting for the original laptop display to reconnect.".into());
            }
            if (record.confirmed && current != record.target)
                || (!record.confirmed && current != record.before && current != record.target)
            {
                self.manual_changed(save)?;
                return Ok(false);
            }
        } else {
            if self
                .expected
                .as_ref()
                .is_some_and(|expected| current != *expected)
            {
                self.manual_changed(save)?;
                return Ok(false);
            }
            let target = hardware.target(&current);
            if current == target {
                self.expected = Some(target);
                return Ok(true);
            }
            let mut next = self.saved.clone();
            next.restore = Some(Restore {
                before: current.clone(),
                target,
                confirmed: false,
            });
            self.commit(next, save)?;
        }
        let record = self
            .saved
            .restore
            .as_ref()
            .expect("saved recovery record")
            .clone();
        if current != record.target {
            hardware.apply(&record.target)?;
        }
        if hardware.read()? != record.target {
            return Err("The battery setting has not been confirmed yet.".into());
        }
        if !record.confirmed {
            let mut next = self.saved.clone();
            next.restore.as_mut().unwrap().confirmed = true;
            self.commit(next, save)?;
        }
        self.expected = Some(record.target);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};

    struct Hardware {
        value: Cell<u32>,
        writes: RefCell<Vec<u32>>,
        fail: Cell<bool>,
        unavailable: Cell<bool>,
    }
    impl Hardware {
        fn new(value: u32) -> Self {
            Self {
                value: Cell::new(value),
                writes: RefCell::new(vec![]),
                fail: Cell::new(false),
                unavailable: Cell::new(false),
            }
        }
    }
    impl Setting<u32> for Hardware {
        fn read(&self) -> Result<u32, String> {
            if self.unavailable.get() {
                Err("HDR Boost owns the display".into())
            } else {
                Ok(self.value.get())
            }
        }
        fn target(&self, _: &u32) -> u32 {
            60
        }
        fn apply(&self, value: &u32) -> Result<(), String> {
            if self.fail.replace(false) {
                return Err("Driver rejected the change".into());
            }
            self.writes.borrow_mut().push(*value);
            self.value.set(*value);
            Ok(())
        }
    }
    fn save(_: &SavedControl<u32>) -> Result<(), String> {
        Ok(())
    }

    #[test]
    fn battery_switches_to_target_and_ac_restores_original_once() {
        let hardware = Hardware::new(165);
        let mut control = Control::new(SavedControl::default());
        control.observe_power(PowerSource::Battery);
        assert!(control.reconcile(&hardware, &save, false).unwrap());
        assert_eq!(hardware.value.get(), 60);
        control.reconcile(&hardware, &save, false).unwrap();
        control.observe_power(PowerSource::Ac);
        control.reconcile(&hardware, &save, false).unwrap();
        assert_eq!(hardware.value.get(), 165);
        assert_eq!(*hardware.writes.borrow(), vec![60, 165]);
        assert!(control.saved.restore.is_none());
    }

    #[test]
    fn already_saving_power_is_not_owned_or_changed_on_ac() {
        let hardware = Hardware::new(60);
        let mut control = Control::new(SavedControl::default());
        control.observe_power(PowerSource::Battery);
        control.reconcile(&hardware, &save, false).unwrap();
        assert!(control.saved.restore.is_none());
        control.observe_power(PowerSource::Ac);
        control.reconcile(&hardware, &save, false).unwrap();
        assert!(hardware.writes.borrow().is_empty());
    }

    #[test]
    fn disabling_checkbox_restores_and_does_not_reapply_on_battery() {
        let hardware = Hardware::new(165);
        let mut control = Control::new(SavedControl::default());
        control.observe_power(PowerSource::Battery);
        control.reconcile(&hardware, &save, false).unwrap();
        control.set_enabled(false, &save).unwrap();
        control.reconcile(&hardware, &save, false).unwrap();
        control.reconcile(&hardware, &save, false).unwrap();
        assert_eq!(hardware.value.get(), 165);
        assert_eq!(hardware.writes.borrow().len(), 2);
    }

    #[test]
    fn manual_override_survives_duplicate_events_and_preserves_manual_setting_on_ac() {
        let hardware = Hardware::new(165);
        let mut control = Control::new(SavedControl::default());
        control.observe_power(PowerSource::Battery);
        control.reconcile(&hardware, &save, false).unwrap();
        hardware.value.set(120);
        control.manual_changed(&save).unwrap();
        control.observe_power(PowerSource::Battery);
        control.reconcile(&hardware, &save, false).unwrap();
        assert_eq!(hardware.value.get(), 120);
        control.observe_power(PowerSource::Ac);
        control.reconcile(&hardware, &save, false).unwrap();
        assert_eq!(hardware.value.get(), 120);
        control.observe_power(PowerSource::Battery);
        control.reconcile(&hardware, &save, false).unwrap();
        assert_eq!(hardware.value.get(), 60);
    }

    #[test]
    fn external_manual_change_is_detected_even_if_original_was_already_target() {
        for original in [60, 165] {
            let hardware = Hardware::new(original);
            let mut control = Control::new(SavedControl::default());
            control.observe_power(PowerSource::Battery);
            control.reconcile(&hardware, &save, false).unwrap();
            hardware.value.set(120);
            control.reconcile(&hardware, &save, false).unwrap();
            assert!(control.overridden);
            control.reconcile(&hardware, &save, false).unwrap();
            assert_eq!(hardware.value.get(), 120);
        }
    }

    #[test]
    fn failed_change_keeps_original_and_retries_instead_of_assuming_manual_override() {
        let hardware = Hardware::new(165);
        let mut control = Control::new(SavedControl::default());
        control.observe_power(PowerSource::Battery);
        hardware.fail.set(true);
        assert!(control.reconcile(&hardware, &save, false).is_err());
        assert_eq!(control.saved.restore.as_ref().unwrap().before, 165);
        assert!(!control.saved.restore.as_ref().unwrap().confirmed);
        control.reconcile(&hardware, &save, false).unwrap();
        assert_eq!(hardware.value.get(), 60);
        assert!(!control.overridden);
    }

    #[test]
    fn save_failure_prevents_hardware_change_and_failed_restore_is_retried() {
        let hardware = Hardware::new(165);
        let mut control = Control::new(SavedControl::default());
        control.observe_power(PowerSource::Battery);
        assert!(control
            .reconcile(&hardware, &|_| Err("Disk full".into()), false)
            .is_err());
        assert!(hardware.writes.borrow().is_empty());
        control.reconcile(&hardware, &save, false).unwrap();
        control.observe_power(PowerSource::Ac);
        hardware.fail.set(true);
        assert!(control.reconcile(&hardware, &save, false).is_err());
        assert!(control.saved.restore.is_some());
        control.reconcile(&hardware, &save, false).unwrap();
        assert_eq!(hardware.value.get(), 165);
    }

    #[test]
    fn restart_on_ac_recovers_pending_or_confirmed_changes() {
        for confirmed in [false, true] {
            let hardware = Hardware::new(60);
            let saved = SavedControl {
                enabled: true,
                restore: Some(Restore {
                    before: 165,
                    target: 60,
                    confirmed,
                }),
            };
            let bytes = serde_json::to_string(&saved).unwrap();
            let mut control = Control::new(serde_json::from_str(&bytes).unwrap());
            control.observe_power(PowerSource::Ac);
            control.reconcile(&hardware, &save, false).unwrap();
            assert_eq!(hardware.value.get(), 165);
            assert!(control.saved.restore.is_none());
        }
    }

    #[test]
    fn unknown_power_and_hdr_lock_defer_without_losing_recovery() {
        let hardware = Hardware::new(165);
        let mut control = Control::new(SavedControl::default());
        control.reconcile(&hardware, &save, false).unwrap();
        assert!(hardware.writes.borrow().is_empty());
        control.observe_power(PowerSource::Battery);
        control.reconcile(&hardware, &save, false).unwrap();
        hardware.unavailable.set(true);
        control.observe_power(PowerSource::Ac);
        assert!(control.reconcile(&hardware, &save, false).is_err());
        assert!(control.saved.restore.is_some());
        hardware.unavailable.set(false);
        control.reconcile(&hardware, &save, false).unwrap();
        assert_eq!(hardware.value.get(), 165);
    }

    #[test]
    fn controls_are_independent_and_exit_releases_only_owned_changes() {
        let display = Hardware::new(165);
        let mode = Hardware::new(90);
        let mut refresh = Control::new(SavedControl::default());
        let mut power = Control::new(SavedControl::default());
        refresh.set_enabled(false, &save).unwrap();
        refresh.observe_power(PowerSource::Battery);
        power.observe_power(PowerSource::Battery);
        refresh.reconcile(&display, &save, false).unwrap();
        power.reconcile(&mode, &save, false).unwrap();
        assert_eq!(display.value.get(), 165);
        assert_eq!(mode.value.get(), 60);
        power.reconcile(&mode, &save, true).unwrap();
        assert_eq!(mode.value.get(), 90);
    }
}
