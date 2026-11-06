use serde::Serialize;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AlwaysOnUsbStatus {
    supported: bool,
    enabled: bool,
    message: Option<String>,
}

impl AlwaysOnUsbStatus {
    fn unsupported(message: impl Into<String>) -> Self {
        Self {
            supported: false,
            enabled: false,
            message: Some(message.into()),
        }
    }
}

#[cfg(all(target_os = "windows", target_arch = "x86_64"))]
mod platform {
    use super::AlwaysOnUsbStatus;
    use std::{
        env,
        ffi::c_void,
        fs,
        os::windows::ffi::OsStrExt,
        path::{Path, PathBuf},
        ptr, thread,
        time::Duration,
    };

    const CONSTRUCTOR_EXPORT: &[u8] = b"??0CUSBCharger@PowerBattery@@QEAA@XZ\0";
    const IS_SUPPORT_EXPORT: &[u8] = b"?IsSupport@CUSBCharger@PowerBattery@@UEAAHXZ\0";
    const GET_STATUS_EXPORT: &[u8] = b"?OpenOrClose@CUSBCharger@PowerBattery@@UEAAHXZ\0";
    const ENABLE_EXPORT: &[u8] = b"?OpenFeature@CUSBCharger@PowerBattery@@UEAAHXZ\0";
    const DISABLE_EXPORT: &[u8] = b"?CloseFeature@CUSBCharger@PowerBattery@@UEAAHXZ\0";

    const STATUS_ENABLED: i32 = 1;
    const STATUS_DISABLED: i32 = 2;

    type Constructor = unsafe extern "system" fn(*mut c_void) -> *mut c_void;
    type UsbMethod = unsafe extern "system" fn(*mut c_void) -> i32;

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(file_name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, proc_name: *const u8) -> *mut c_void;
        fn FreeLibrary(module: *mut c_void) -> i32;
    }

    #[repr(C, align(16))]
    struct UsbChargerInstance([u8; 64]);

    struct UsbApi {
        module: *mut c_void,
        _instance: Box<UsbChargerInstance>,
        this: *mut c_void,
        is_support: UsbMethod,
        get_status: UsbMethod,
        enable: UsbMethod,
        disable: UsbMethod,
    }

    impl UsbApi {
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
                let constructor: Constructor =
                    unsafe { std::mem::transmute(resolve(module, CONSTRUCTOR_EXPORT)?) };
                let is_support =
                    unsafe { std::mem::transmute(resolve(module, IS_SUPPORT_EXPORT)?) };
                let get_status =
                    unsafe { std::mem::transmute(resolve(module, GET_STATUS_EXPORT)?) };
                let enable = unsafe { std::mem::transmute(resolve(module, ENABLE_EXPORT)?) };
                let disable = unsafe { std::mem::transmute(resolve(module, DISABLE_EXPORT)?) };

                let mut instance = Box::new(UsbChargerInstance([0; 64]));
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
                    is_support,
                    get_status,
                    enable,
                    disable,
                })
            })();

            if result.is_err() {
                unsafe {
                    let _ = FreeLibrary(module);
                }
            }
            result
        }

        fn supported(&self) -> bool {
            unsafe { (self.is_support)(self.this) != 0 }
        }

        fn enabled(&self) -> Result<bool, String> {
            match unsafe { (self.get_status)(self.this) } {
                STATUS_ENABLED => Ok(true),
                STATUS_DISABLED => Ok(false),
                value => Err(format!(
                    "Lenovo returned an unknown Always on USB state ({value})."
                )),
            }
        }

        fn set_enabled(&self, enabled: bool) -> bool {
            unsafe {
                if enabled {
                    (self.enable)(self.this) != 0
                } else {
                    (self.disable)(self.this) != 0
                }
            }
        }
    }

    impl Drop for UsbApi {
        fn drop(&mut self) {
            if !self.module.is_null() {
                unsafe {
                    let _ = FreeLibrary(self.module);
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
            "Lenovo Vantage's IdeaNotebook power component is not installed.".to_string()
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

    fn status(enabled: bool) -> AlwaysOnUsbStatus {
        AlwaysOnUsbStatus {
            supported: true,
            enabled,
            message: None,
        }
    }

    pub fn get() -> Result<AlwaysOnUsbStatus, String> {
        let api = UsbApi::load()?;
        if !api.supported() {
            return Err("Always on USB is not supported on this Lenovo.".to_string());
        }
        api.enabled().map(status)
    }

    pub fn set(enabled: bool) -> Result<AlwaysOnUsbStatus, String> {
        let api = UsbApi::load()?;
        if !api.supported() {
            return Err("Always on USB is not supported on this Lenovo.".to_string());
        }
        if api.enabled()? == enabled {
            return Ok(status(enabled));
        }
        if !api.set_enabled(enabled) {
            return Err("Lenovo did not accept the requested Always on USB setting.".to_string());
        }

        for _ in 0..10 {
            thread::sleep(Duration::from_millis(100));
            if api.enabled()? == enabled {
                return Ok(status(enabled));
            }
        }
        Err("Lenovo did not confirm the requested Always on USB setting.".to_string())
    }
}

#[tauri::command]
pub fn get_always_on_usb() -> AlwaysOnUsbStatus {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        return platform::get().unwrap_or_else(AlwaysOnUsbStatus::unsupported);
    }

    #[cfg(not(all(target_os = "windows", target_arch = "x86_64")))]
    AlwaysOnUsbStatus::unsupported("Always on USB is supported only by the Windows x64 build.")
}

#[tauri::command]
pub fn set_always_on_usb(enabled: bool) -> Result<AlwaysOnUsbStatus, String> {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    {
        return platform::set(enabled);
    }

    #[cfg(not(all(target_os = "windows", target_arch = "x86_64")))]
    Err("Always on USB is supported only by the Windows x64 build.".to_string())
}
