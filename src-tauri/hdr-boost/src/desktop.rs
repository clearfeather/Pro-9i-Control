use windows::{
    core::PWSTR,
    Win32::System::RemoteDesktop::{
        WTSActive, WTSFreeMemory, WTSQuerySessionInformationW, WTSSessionInfoEx, WTSINFOEXW,
        WTS_CURRENT_SESSION, WTS_SESSIONSTATE_UNLOCK,
    },
    Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_REMOTESESSION},
};

/// Check existing session state as well as subscribing to later lock events.
pub(crate) fn ensure_active() -> Result<(), String> {
    unsafe {
        if GetSystemMetrics(SM_REMOTESESSION) != 0 {
            return Err("HDR Boost requires the laptop's local desktop.".into());
        }
        let mut buffer = PWSTR::null();
        let mut bytes = 0;
        WTSQuerySessionInformationW(
            None,
            WTS_CURRENT_SESSION,
            WTSSessionInfoEx,
            &mut buffer,
            &mut bytes,
        )
        .map_err(|e| format!("Cannot verify the desktop session: {e}"))?;
        let valid = if !buffer.is_null() && bytes as usize >= std::mem::size_of::<WTSINFOEXW>() {
            let info = &*buffer.0.cast::<WTSINFOEXW>();
            info.Level == 1
                && info.Data.WTSInfoExLevel1.SessionState == WTSActive
                && info.Data.WTSInfoExLevel1.SessionFlags == WTS_SESSIONSTATE_UNLOCK as i32
        } else {
            false
        };
        WTSFreeMemory(buffer.0.cast());
        if valid {
            Ok(())
        } else {
            Err("Unlock the laptop and use its local desktop before enabling HDR Boost.".into())
        }
    }
}
