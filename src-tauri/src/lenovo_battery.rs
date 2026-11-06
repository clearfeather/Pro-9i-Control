use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConservationModeStatus {
    pub supported: bool,
    pub enabled: bool,
    pub mode: String,
    pub message: Option<String>,
}

impl ConservationModeStatus {
    fn unsupported(message: impl Into<String>) -> Self {
        Self {
            supported: false,
            enabled: false,
            mode: "unsupported".to_string(),
            message: Some(message.into()),
        }
    }
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
mod platform {
    use super::ConservationModeStatus;
    use std::{
        env,
        ffi::c_void,
        fs,
        os::windows::ffi::OsStrExt,
        path::{Path, PathBuf},
        ptr,
        thread,
        time::Duration,
    };

    const CONSTRUCTOR_EXPORT: &[u8] = b"??0CChargingMode@PowerBattery@@QEAA@XZ\0";
    const GET_MODE_EXPORT: &[u8] = b"?GetChargingMode@CChargingMode@PowerBattery@@QEBAHXZ\0";
    const SET_MODE_EXPORT: &[u8] = b"?SetChargingMode@CChargingMode@PowerBattery@@QEBAHH@Z\0";

    const MODE_NORMAL: i32 = 0;
    const MODE_CONSERVATION: i32 = 1;
    const MODE_RAPID: i32 = 2;

    type Constructor = unsafe extern "system" fn(*mut c_void) -> *mut c_void;
    type GetChargingMode = unsafe extern "system" fn(*mut c_void) -> i32;
    type SetChargingMode = unsafe extern "system" fn(*mut c_void, i32) -> i32;

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(file_name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, proc_name: *const u8) -> *mut c_void;
        fn FreeLibrary(module: *mut c_void) -> i32;
    }

    #[repr(C, align(16))]
    struct ChargingModeInstance([u8; 64]);

    struct BatteryApi {
        module: *mut c_void,
        _instance: Box<ChargingModeInstance>,
        this: *mut c_void,
        get_mode: GetChargingMode,
        set_mode: SetChargingMode,
    }

    impl BatteryApi {
        fn load() -> Result<Self, String> {
            let dll_path = find_power_battery_dll()?;
            let wide_path: Vec<u16> = dll_path
                .as_os_str()
                .encode_wide()
                .chain(std::iter::once(0))
                .collect();

            let module = unsafe { LoadLibraryW(wide_path.as_ptr()) };
            if module.is_null() {
                return Err(format!(
                    "Could not load Lenovo's PowerBattery.dll: {}",
                    std::io::Error::last_os_error()
                ));
            }

            let result = (|| {
                let constructor_ptr = resolve(module, CONSTRUCTOR_EXPORT)?;
                let get_mode_ptr = resolve(module, GET_MODE_EXPORT)?;
                let set_mode_ptr = resolve(module, SET_MODE_EXPORT)?;

                let constructor: Constructor = unsafe { std::mem::transmute(constructor_ptr) };
                let get_mode: GetChargingMode = unsafe { std::mem::transmute(get_mode_ptr) };
                let set_mode: SetChargingMode = unsafe { std::mem::transmute(set_mode_ptr) };

                let mut instance = Box::new(ChargingModeInstance([0; 64]));
                let instance_ptr = instance.0.as_mut_ptr().cast::<c_void>();
                let constructed = unsafe { constructor(instance_ptr) };
                let this = if constructed.is_null() {
                    instance_ptr
                } else {
                    constructed
                };

                Ok(Self {
                    module,
                    _instance: instance,
                    this,
                    get_mode,
                    set_mode,
                })
            })();

            if result.is_err() {
                unsafe {
                    FreeLibrary(module);
                }
            }

            result
        }

        fn get(&self) -> Result<i32, String> {
            let mode = unsafe { (self.get_mode)(self.this) };
            match mode {
                MODE_NORMAL | MODE_CONSERVATION | MODE_RAPID => Ok(mode),
                _ => Err(format!("Lenovo returned an unknown charging mode ({mode}).")),
            }
        }

        fn set(&self, mode: i32) {
            unsafe {
                (self.set_mode)(self.this, mode);
            }
        }
    }

    impl Drop for BatteryApi {
        fn drop(&mut self) {
            if !self.module.is_null() {
                unsafe {
                    FreeLibrary(self.module);
                }
                self.module = ptr::null_mut();
            }
        }
    }

    fn resolve(module: *mut c_void, name: &'static [u8]) -> Result<*mut c_void, String> {
        let function = unsafe { GetProcAddress(module, name.as_ptr()) };
        if function.is_null() {
            let printable = String::from_utf8_lossy(&name[..name.len().saturating_sub(1)]);
            Err(format!(
                "Lenovo's PowerBattery.dll does not export {printable}."
            ))
        } else {
            Ok(function)
        }
    }

    fn find_power_battery_dll() -> Result<PathBuf, String> {
        let program_data = env::var_os("PROGRAMDATA")
            .map(PathBuf::from)
            .ok_or_else(|| "Windows ProgramData directory was not found.".to_string())?;
        let addin_root = program_data
            .join("Lenovo")
            .join("Vantage")
            .join("Addins")
            .join("IdeaNotebookAddin");

        let direct = addin_root.join("PowerBattery.dll");
        if direct.is_file() {
            return Ok(direct);
        }

        let entries = fs::read_dir(&addin_root).map_err(|_| {
            "Lenovo Vantage's IdeaNotebook battery component is not installed.".to_string()
        })?;

        let mut candidates: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_dir())
            .map(|path| path.join("PowerBattery.dll"))
            .filter(|path| path.is_file())
            .collect();

        candidates.sort_by(|left, right| version_key(left).cmp(&version_key(right)));
        candidates.pop().ok_or_else(|| {
            "Lenovo Vantage's PowerBattery.dll was not found. Reinstall Lenovo Vantage Service."
                .to_string()
        })
    }

    fn version_key(path: &Path) -> Vec<u32> {
        path.parent()
            .and_then(Path::file_name)
            .and_then(|name| name.to_str())
            .map(|name| {
                name.split('.')
                    .map(|part| part.parse::<u32>().unwrap_or_default())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn status_from_mode(mode: i32) -> ConservationModeStatus {
        let (enabled, name) = match mode {
            MODE_CONSERVATION => (true, "conservation"),
            MODE_RAPID => (false, "rapid"),
            _ => (false, "normal"),
        };

        ConservationModeStatus {
            supported: true,
            enabled,
            mode: name.to_string(),
            message: None,
        }
    }

    pub fn get_status() -> Result<ConservationModeStatus, String> {
        let api = BatteryApi::load()?;
        api.get().map(status_from_mode)
    }

    pub fn set_enabled(enabled: bool) -> Result<ConservationModeStatus, String> {
        let api = BatteryApi::load()?;
        let target = if enabled {
            MODE_CONSERVATION
        } else {
            MODE_NORMAL
        };

        api.set(target);

        for _ in 0..10 {
            thread::sleep(Duration::from_millis(100));
            let actual = api.get()?;
            if actual == target {
                return Ok(status_from_mode(actual));
            }
        }

        Err("Lenovo did not apply the requested charging mode.".to_string())
    }
}

#[tauri::command]
pub fn get_conservation_mode() -> ConservationModeStatus {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        return platform::get_status().unwrap_or_else(ConservationModeStatus::unsupported);
    }

    #[cfg(not(all(target_os = "windows", target_arch = "x86_64")))]
    ConservationModeStatus::unsupported(
        "Lenovo Conservation Mode is supported only by the Windows x64 build.",
    )
}

#[tauri::command]
pub fn set_conservation_mode(enabled: bool) -> Result<ConservationModeStatus, String> {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        return platform::set_enabled(enabled);
    }

    #[cfg(not(all(target_os = "windows", target_arch = "x86_64")))]
    Err("Lenovo Conservation Mode is supported only by the Windows x64 build.".to_string())
}
