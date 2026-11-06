#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerSource {
    Ac,
    Battery,
    Unknown,
}

impl PowerSource {
    pub fn on_battery(self) -> Option<bool> {
        match self {
            Self::Ac => Some(false),
            Self::Battery => Some(true),
            Self::Unknown => None,
        }
    }
}

#[cfg(target_os = "windows")]
pub fn read_power() -> PowerSource {
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut status = SYSTEM_POWER_STATUS::default();
    if unsafe { GetSystemPowerStatus(&mut status) }.is_err() {
        return PowerSource::Unknown;
    }
    match status.ACLineStatus {
        0 => PowerSource::Battery,
        1 => PowerSource::Ac,
        _ => PowerSource::Unknown,
    }
}

#[cfg(not(target_os = "windows"))]
pub fn read_power() -> PowerSource {
    PowerSource::Unknown
}
