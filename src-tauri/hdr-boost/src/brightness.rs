//! Panel brightness diagnostics, legacy recovery, and optional mode-switch preservation.
use windows::{
    core::{w, Result, BSTR},
    Win32::System::{
        Com::*,
        Rpc::{RPC_C_AUTHN_WINNT, RPC_C_AUTHZ_NONE},
        Variant::VARIANT,
        Wmi::*,
    },
};

struct Apartment;
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

fn connect() -> Result<(Apartment, IWbemServices)> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED).ok()?;
        let apartment = Apartment;
        let locator: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER)?;
        let service = locator.ConnectServer(
            &BSTR::from("ROOT\\WMI"),
            &BSTR::new(),
            &BSTR::new(),
            &BSTR::new(),
            0,
            &BSTR::new(),
            None,
        )?;
        CoSetProxyBlanket(
            &service,
            RPC_C_AUTHN_WINNT,
            RPC_C_AUTHZ_NONE,
            None,
            RPC_C_AUTHN_LEVEL_CALL,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            None,
            EOAC_NONE,
        )?;
        Ok((apartment, service))
    }
}

fn query(service: &IWbemServices, class: &str) -> Result<IWbemClassObject> {
    unsafe {
        let rows = service.ExecQuery(
            &BSTR::from("WQL"),
            &BSTR::from(format!("SELECT * FROM {class} WHERE Active=TRUE")),
            WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
            None,
        )?;
        let mut objects = [None];
        let mut count = 0;
        rows.Next(3000, &mut objects, &mut count).ok()?;
        objects[0]
            .take()
            .ok_or_else(windows::core::Error::from_win32)
    }
}

fn value(object: &IWbemClassObject, name: &str) -> Result<VARIANT> {
    let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
    let mut value = VARIANT::default();
    unsafe {
        object.Get(
            windows::core::PCWSTR(wide.as_ptr()),
            0,
            &mut value,
            None,
            None,
        )?;
    }
    Ok(value)
}

pub fn current() -> std::result::Result<u8, String> {
    (|| -> Result<u8> {
        let (_apartment, service) = connect()?;
        let row = query(&service, "WmiMonitorBrightness")?;
        let percent = u32::try_from(&value(&row, "CurrentBrightness")?)?;
        if percent > 100 {
            return Err(windows::core::Error::from_win32());
        }
        Ok(percent as u8)
    })()
    .map_err(|e| format!("Cannot read the panel brightness: {e}"))
}

fn write(percent: u8) -> std::result::Result<(), String> {
    if percent > 100 {
        return Err("Invalid panel brightness".into());
    }
    (|| -> Result<()> {
        let (_apartment, service) = connect()?;
        let row = query(&service, "WmiMonitorBrightnessMethods")?;
        let object_path = BSTR::try_from(&value(&row, "__PATH")?)?;
        unsafe {
            let mut class = None;
            service.GetObject(
                &BSTR::from("WmiMonitorBrightnessMethods"),
                WBEM_GENERIC_FLAG_TYPE(0),
                None,
                Some(&mut class),
                None,
            )?;
            let mut signature = None;
            class.unwrap().GetMethod(
                w!("WmiSetBrightness"),
                0,
                &mut signature,
                std::ptr::null_mut(),
            )?;
            let input = signature.unwrap().SpawnInstance(0)?;
            // WMI represents CIM uint8/uint32 method inputs as VT_I4.
            input.Put(w!("Timeout"), 0, &VARIANT::from(0_i32), 0)?;
            input.Put(w!("Brightness"), 0, &VARIANT::from(i32::from(percent)), 0)?;
            let mut output = None;
            service.ExecMethod(
                &object_path,
                &BSTR::from("WmiSetBrightness"),
                WBEM_GENERIC_FLAG_TYPE(0),
                None,
                &input,
                Some(&mut output),
                None,
            )?;
            // This panel provider can return no output object after ExecMethod.
            // Verify CurrentBrightness in the session's startup/restore checks.
            if let Some(output) = output {
                if let Ok(result) = value(&output, "ReturnValue") {
                    if u32::try_from(&result)? != 0 {
                        return Err(windows::core::Error::from_win32());
                    }
                }
            }
        }
        Ok(())
    })()
    .map_err(|e| format!("Cannot set the panel brightness: {e}"))
}

/// Recovery retains exact verification of its saved, previously observed value.
pub fn set(percent: u8) -> std::result::Result<(), String> {
    write(percent)?;
    wait_for_brightness(percent, 0).map(|_| ())
}

/// A manual request may be rounded by the panel or superseded by Windows keys.
/// Return the observed level; never keep enforcing an older slider request.
pub fn set_user(percent: u8) -> std::result::Result<u8, String> {
    write(percent)?;
    wait_for_brightness(percent, 1)
}

fn matches_readback(requested: u8, actual: u8, tolerance: u8) -> bool {
    actual.abs_diff(requested) <= tolerance
}

fn wait_for_brightness(percent: u8, tolerance: u8) -> std::result::Result<u8, String> {
    // The provider can acknowledge a write before CurrentBrightness catches up.
    // Wait for read-back rather than treating that short lag as restore failure.
    let timeout_ms = if tolerance == 0 { 3000 } else { 400 };
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(timeout_ms);
    let mut nearby_since = None;
    loop {
        let actual = current();
        if actual == Ok(percent) {
            return Ok(percent);
        }
        match actual.as_ref() {
            Ok(level) if matches_readback(percent, *level, tolerance) => {
                let since = nearby_since.get_or_insert((*level, std::time::Instant::now()));
                if since.0 != *level {
                    *since = (*level, std::time::Instant::now());
                } else if since.1.elapsed() >= std::time::Duration::from_millis(250) {
                    return Ok(*level);
                }
            }
            _ => nearby_since = None,
        }
        if std::time::Instant::now() >= deadline {
            if tolerance > 0 { return actual; }
            return Err(format!(
                "Panel brightness did not settle to {percent}% (read-back: {actual:?})"
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}

/// A read-only subscription to changes made by Windows brightness keys,
/// Settings, or other brightness controls. COM objects stay on this thread.
pub struct BrightnessEvents {
    events: IEnumWbemClassObject,
    _service: IWbemServices,
    _apartment: Apartment,
}

impl BrightnessEvents {
    pub fn subscribe() -> std::result::Result<Self, String> {
        (|| -> Result<Self> {
            let (apartment, service) = connect()?;
            let events = unsafe {
                service.ExecNotificationQuery(
                    &BSTR::from("WQL"),
                    &BSTR::from("SELECT * FROM WmiMonitorBrightnessEvent WHERE Active=TRUE"),
                    WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
                    None,
                )?
            };
            Ok(Self { events, _service: service, _apartment: apartment })
        })().map_err(|e| format!("Cannot watch Windows brightness changes: {e}"))
    }

    pub fn next(&self, timeout_ms: i32) -> std::result::Result<Option<u8>, String> {
        (|| -> Result<Option<u8>> {
            let mut objects = [None];
            let mut count = 0;
            unsafe { self.events.Next(timeout_ms, &mut objects, &mut count).ok()?; }
            let Some(row) = objects[0].take() else { return Ok(None); };
            let percent = u32::try_from(&value(&row, "Brightness")?)?;
            Ok((percent <= 100).then_some(percent as u8))
        })().map_err(|e| format!("Cannot receive Windows brightness changes: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_brightness_accepts_the_reported_one_point_rounding() {
        assert!(matches_readback(17, 18, 1));
        assert!(matches_readback(18, 17, 1));
        assert!(matches_readback(18, 18, 1));
        assert!(!matches_readback(17, 19, 1));
        assert!(!matches_readback(17, 70, 1));
    }

    #[test]
    fn recovery_still_requires_its_exact_saved_brightness() {
        assert!(matches_readback(18, 18, 0));
        assert!(!matches_readback(17, 18, 0));
    }
}
