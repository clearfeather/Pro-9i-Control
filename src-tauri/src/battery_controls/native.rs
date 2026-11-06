use super::policy::Setting;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshValue {
    pub device_path: String,
    pub numerator: u32,
    pub denominator: u32,
}

impl RefreshValue {
    fn hz(&self) -> Option<u64> {
        if self.numerator == 0 || self.denominator == 0 {
            return None;
        }
        Some(
            (u64::from(self.numerator) + u64::from(self.denominator) / 2)
                / u64::from(self.denominator),
        )
    }
}

impl PartialEq for RefreshValue {
    fn eq(&self, other: &Self) -> bool {
        self.device_path.eq_ignore_ascii_case(&other.device_path)
            && self.hz().is_some()
            && self.hz() == other.hz()
    }
}

pub struct RefreshSetting;

impl Setting<RefreshValue> for RefreshSetting {
    fn read(&self) -> Result<RefreshValue, String> {
        let status = crate::hdr_boost::controller()?.status();
        if status.busy() || status.recovery_pending {
            return Err("Waiting for HDR Boost to finish before changing refresh rate.".into());
        }
        let _lease = lenovo_hdr_boost::change_lease()?;
        let display = lenovo_hdr_boost::display_configurations()?
            .into_iter()
            .find(|d| d.built_in)
            .ok_or("The built-in laptop display is not active.")?;
        let value = RefreshValue {
            device_path: display.device_path,
            numerator: display.refresh.0,
            denominator: display.refresh.1,
        };
        if value.hz().is_none() {
            return Err("The laptop display refresh rate is unavailable.".into());
        }
        Ok(value)
    }

    fn target(&self, current: &RefreshValue) -> RefreshValue {
        RefreshValue {
            device_path: current.device_path.clone(),
            numerator: 60,
            denominator: 1,
        }
    }

    fn apply(&self, value: &RefreshValue) -> Result<(), String> {
        let _lease = lenovo_hdr_boost::change_lease()?;
        let display = lenovo_hdr_boost::display_configurations()?
            .into_iter()
            .find(|d| d.built_in && d.device_path.eq_ignore_ascii_case(&value.device_path))
            .ok_or("Waiting for the original laptop display to reconnect.")?;
        lenovo_hdr_boost::set_display_refresh(&display, (value.numerator, value.denominator))
    }

    fn compatible(&self, current: &RefreshValue, saved: &RefreshValue) -> bool {
        current.device_path.eq_ignore_ascii_case(&saved.device_path)
    }
}

pub struct PowerModeSetting<'a> {
    pub brightness: &'a RefCell<crate::power_brightness::BrightnessKeeper>,
}

impl Setting<String> for PowerModeSetting<'_> {
    fn read(&self) -> Result<String, String> {
        let status = crate::lenovo_power::read_power_mode()?;
        if !matches!(status.mode.as_str(), "auto" | "cool" | "performance") {
            return Err("The current Lenovo power mode is unknown.".into());
        }
        Ok(status.mode)
    }
    fn target(&self, _: &String) -> String {
        "cool".into()
    }
    fn apply(&self, value: &String) -> Result<(), String> {
        self.brightness.borrow_mut().apply(value).map(|_| ())
    }
}

pub fn manual_refresh(
    device_name: String,
    width: u32,
    height: u32,
    refresh_hz: Option<u32>,
) -> Result<bool, String> {
    let built_in = lenovo_hdr_boost::display_configurations()?
        .iter()
        .any(|d| d.built_in && d.gdi_device_name.eq_ignore_ascii_case(&device_name));
    // The existing provider acquires the HDR Boost display lease.
    crate::displays::active_provider().set_monitor_resolution(
        device_name,
        width,
        height,
        refresh_hz,
    )?;
    Ok(built_in)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refresh_ownership_uses_panel_identity_and_nominal_hz_but_keeps_exact_saved_timing() {
        let original = RefreshValue {
            device_path: "panel-one".into(),
            numerator: 110817000,
            denominator: 671609,
        };
        let target = RefreshSetting.target(&original);
        let actual = RefreshValue {
            device_path: "PANEL-ONE".into(),
            numerator: 59940,
            denominator: 1000,
        };
        assert_eq!(actual, target);
        assert!(RefreshSetting.compatible(&actual, &original));
        let other = RefreshValue {
            device_path: "other-panel".into(),
            numerator: 60,
            denominator: 1,
        };
        assert!(!RefreshSetting.compatible(&other, &target));
        assert_ne!(other, target);
        let restored: RefreshValue =
            serde_json::from_str(&serde_json::to_string(&original).unwrap()).unwrap();
        assert_eq!(
            (restored.numerator, restored.denominator),
            (110817000, 671609)
        );
    }
}
