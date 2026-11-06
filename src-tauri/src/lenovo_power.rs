use serde::Serialize;
use tauri::Manager;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerModeStatus {
    supported: bool,
    pub(crate) mode: String,
    available_modes: Vec<String>,
    message: Option<String>,
}

impl PowerModeStatus {
    fn unsupported(message: impl Into<String>) -> Self {
        Self {
            supported: false,
            mode: "unsupported".into(),
            available_modes: Vec::new(),
            message: Some(message.into()),
        }
    }
}

#[cfg(target_os = "windows")]
mod platform {
    use super::PowerModeStatus;
    use std::{ptr, thread, time::Duration};

    type HKey = isize;
    type ScHandle = isize;

    const HKEY_LOCAL_MACHINE: HKey = -2147483646isize;
    const KEY_READ: u32 = 0x20019;
    const REG_DWORD: u32 = 4;
    const ERROR_SUCCESS: i32 = 0;
    const SC_MANAGER_CONNECT: u32 = 0x0001;
    const SERVICE_USER_DEFINED_CONTROL: u32 = 0x0100;

    const REGISTRY_KEY: &str = "SYSTEM\\CurrentControlSet\\Services\\LITSSVC\\LNBITS\\IC\\MMC";
    const SERVICE_NAME: &str = "LITSSVC";

    const MODE_AUTO: &str = "auto";
    const MODE_COOL: &str = "cool";
    const MODE_PERFORMANCE: &str = "performance";

    const COMMAND_AUTO: u32 = 135;
    const COMMAND_COOL: u32 = 146;
    const COMMAND_PERFORMANCE: u32 = 148;

    #[repr(C)]
    struct ServiceStatus {
        service_type: u32,
        current_state: u32,
        controls_accepted: u32,
        win32_exit_code: u32,
        service_specific_exit_code: u32,
        check_point: u32,
        wait_hint: u32,
    }

    #[link(name = "advapi32")]
    extern "system" {
        fn RegOpenKeyExW(
            key: HKey,
            sub_key: *const u16,
            options: u32,
            desired_access: u32,
            result: *mut HKey,
        ) -> i32;
        fn RegQueryValueExW(
            key: HKey,
            value_name: *const u16,
            reserved: *mut u32,
            value_type: *mut u32,
            data: *mut u8,
            data_size: *mut u32,
        ) -> i32;
        fn RegCloseKey(key: HKey) -> i32;
        fn OpenSCManagerW(
            machine_name: *const u16,
            database_name: *const u16,
            desired_access: u32,
        ) -> ScHandle;
        fn OpenServiceW(
            manager: ScHandle,
            service_name: *const u16,
            desired_access: u32,
        ) -> ScHandle;
        fn ControlService(service: ScHandle, control: u32, status: *mut ServiceStatus) -> i32;
        fn CloseServiceHandle(handle: ScHandle) -> i32;
    }

    struct RegistryKey(HKey);

    impl Drop for RegistryKey {
        fn drop(&mut self) {
            unsafe {
                let _ = RegCloseKey(self.0);
            }
        }
    }

    struct ServiceHandle(ScHandle);

    impl Drop for ServiceHandle {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseServiceHandle(self.0);
            }
        }
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }

    fn win32_error(context: &str) -> String {
        format!("{context}: {}", std::io::Error::last_os_error())
    }

    fn open_registry_key() -> Result<RegistryKey, String> {
        let mut key = 0;
        let path = wide(REGISTRY_KEY);
        let result =
            unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, path.as_ptr(), 0, KEY_READ, &mut key) };

        if result != ERROR_SUCCESS {
            return Err(format!(
                "Lenovo Intelligent Thermal Solution is unavailable (registry error {result})."
            ));
        }

        Ok(RegistryKey(key))
    }

    fn read_dword(key: &RegistryKey, name: &str) -> Result<u32, String> {
        let name = wide(name);
        let mut value_type = 0;
        let mut value = 0u32;
        let mut value_size = std::mem::size_of::<u32>() as u32;
        let result = unsafe {
            RegQueryValueExW(
                key.0,
                name.as_ptr(),
                ptr::null_mut(),
                &mut value_type,
                (&mut value as *mut u32).cast::<u8>(),
                &mut value_size,
            )
        };

        if result != ERROR_SUCCESS || value_type != REG_DWORD {
            return Err(format!(
                "Could not read Lenovo power mode state (error {result})."
            ));
        }

        Ok(value)
    }

    fn current_mode(automatic: u32, current: u32) -> &'static str {
        if automatic == 2 {
            MODE_AUTO
        } else if current == 1 {
            MODE_COOL
        } else if current == 3 {
            MODE_PERFORMANCE
        } else {
            "unknown"
        }
    }

    pub fn get() -> Result<PowerModeStatus, String> {
        let key = open_registry_key()?;
        let automatic = read_dword(&key, "AutomaticModeSetting")?;
        let current = read_dword(&key, "CurrentSetting")?;
        let capability = read_dword(&key, "Capability")?;

        let mut available_modes = Vec::new();
        if capability & 0x1 == 0 {
            available_modes.push(MODE_AUTO.into());
        }
        if capability & 0x2 != 0 {
            available_modes.push(MODE_COOL.into());
        }
        if capability & 0x8 != 0 {
            available_modes.push(MODE_PERFORMANCE.into());
        }

        if available_modes.is_empty() {
            return Err("This Lenovo does not report any supported power modes.".into());
        }

        Ok(PowerModeStatus {
            supported: true,
            mode: current_mode(automatic, current).into(),
            available_modes,
            message: None,
        })
    }

    fn send_control(command: u32) -> Result<(), String> {
        let manager = unsafe { OpenSCManagerW(ptr::null(), ptr::null(), SC_MANAGER_CONNECT) };
        if manager == 0 {
            return Err(win32_error(
                "Could not connect to Windows Service Control Manager",
            ));
        }
        let manager = ServiceHandle(manager);

        let service_name = wide(SERVICE_NAME);
        let service = unsafe {
            OpenServiceW(
                manager.0,
                service_name.as_ptr(),
                SERVICE_USER_DEFINED_CONTROL,
            )
        };
        if service == 0 {
            return Err(win32_error(
                "Lenovo Intelligent Thermal Solution service is unavailable",
            ));
        }
        let service = ServiceHandle(service);

        let mut status = unsafe { std::mem::zeroed::<ServiceStatus>() };
        let result = unsafe { ControlService(service.0, command, &mut status) };
        if result == 0 {
            return Err(win32_error("Lenovo rejected the requested power mode"));
        }

        Ok(())
    }

    pub fn set(mode: &str) -> Result<PowerModeStatus, String> {
        let before = get()?;
        if !before.available_modes.iter().any(|item| item == mode) {
            return Err(format!(
                "Power mode '{mode}' is not supported on this Lenovo."
            ));
        }
        if before.mode == mode {
            return Ok(before);
        }

        let command = match mode {
            MODE_AUTO => COMMAND_AUTO,
            MODE_COOL => COMMAND_COOL,
            MODE_PERFORMANCE => COMMAND_PERFORMANCE,
            _ => return Err(format!("Unknown power mode '{mode}'.")),
        };
        send_control(command)?;

        for _ in 0..20 {
            thread::sleep(Duration::from_millis(100));
            if let Ok(status) = get() {
                if status.mode == mode {
                    return Ok(status);
                }
            }
        }

        Err("Lenovo did not confirm the requested power mode.".into())
    }
}

#[cfg(not(target_os = "windows"))]
mod platform {
    use super::PowerModeStatus;

    pub fn get() -> Result<PowerModeStatus, String> {
        Err("Lenovo power modes are only available on Windows.".into())
    }

    pub fn set(_mode: &str) -> Result<PowerModeStatus, String> {
        get()
    }
}

#[tauri::command]
pub fn get_lenovo_power_mode() -> PowerModeStatus {
    read_power_mode().unwrap_or_else(PowerModeStatus::unsupported)
}

pub(crate) fn read_power_mode() -> Result<PowerModeStatus, String> {
    platform::get()
}
pub(crate) fn apply_power_mode(mode: &str) -> Result<PowerModeStatus, String> {
    platform::set(mode)
}

#[tauri::command]
pub async fn set_lenovo_power_mode(
    app: tauri::AppHandle,
    mode: String,
) -> Result<PowerModeStatus, String> {
    let controller = app
        .try_state::<crate::battery_controls::Controller>()
        .filter(|controller| controller.supported())
        .map(|controller| controller.inner().clone());
    tauri::async_runtime::spawn_blocking(move || match controller {
        Some(controller) => controller.manual_mode(mode),
        None => apply_power_mode(&mode),
    })
    .await
    .map_err(|error| error.to_string())?
}
