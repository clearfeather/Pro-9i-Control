use std::time::Duration;
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

/// This limits only capture/render work, never the display's refresh or backlight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Budget {
    Ac,
    Battery,
    Saver,
}

impl Budget {
    pub(crate) fn current() -> Self {
        let mut status = SYSTEM_POWER_STATUS::default();
        if unsafe { GetSystemPowerStatus(&mut status) }.is_err() {
            return Self::Battery;
        }
        Self::from_status(status.ACLineStatus, status.SystemStatusFlag)
    }

    fn from_status(ac_line: u8, saver: u8) -> Self {
        if saver == 1 {
            Self::Saver
        } else if ac_line == 1 {
            Self::Ac
        } else {
            // Unknown/disconnected power must not select the unlimited budget.
            Self::Battery
        }
    }

    pub(crate) fn interval(self) -> Duration {
        match self {
            Self::Ac => Duration::ZERO,
            Self::Battery => Duration::from_nanos(16_666_700),
            Self::Saver => Duration::from_nanos(33_333_400),
        }
    }

    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::Ac => "Boost active · 60 Hz. Ctrl+Alt+B stops and restores.",
            Self::Battery => "Boost active · 60 Hz · battery: 60 fps max. Ctrl+Alt+B stops.",
            Self::Saver => "Boost active · 60 Hz · Battery Saver: 30 fps max. Ctrl+Alt+B stops.",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn power_limits_follow_ac_battery_saver_and_unknown_status() {
        assert_eq!(Budget::from_status(1, 0), Budget::Ac);
        for ac in [0, 255] {
            assert_eq!(Budget::from_status(ac, 0), Budget::Battery);
        }
        for ac in [0, 1, 255] {
            assert_eq!(Budget::from_status(ac, 1), Budget::Saver);
        }
        assert!(Budget::Battery.interval() >= Duration::from_secs_f64(1.0 / 60.0));
        assert!(Budget::Saver.interval() >= Duration::from_secs_f64(1.0 / 30.0));
    }
}
