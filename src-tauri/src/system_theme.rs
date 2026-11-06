#[cfg(target_os = "windows")]
fn wide_null(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(target_os = "windows")]
fn write_theme_value(
    key: windows::Win32::System::Registry::HKEY,
    name: &str,
    value: u32,
) -> Result<(), String> {
    use windows::Win32::System::Registry::{RegSetValueExW, REG_DWORD};

    let value_name = name;
    let name = wide_null(value_name);
    let bytes = value.to_le_bytes();
    let result = unsafe {
        RegSetValueExW(
            key,
            windows::core::PCWSTR(name.as_ptr()),
            0,
            REG_DWORD,
            Some(&bytes),
        )
    };

    if result.is_err() {
        Err(format!(
            "Windows could not update {value_name}: {result:?}"
        ))
    } else {
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn read_windows_theme() -> Result<&'static str, String> {
    use windows::Win32::System::Registry::{
        RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
    };

    let path = wide_null(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let mut key = HKEY::default();
    let open_result = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            windows::core::PCWSTR(path.as_ptr()),
            0,
            KEY_READ,
            &mut key,
        )
    };
    if open_result.is_err() {
        return Err(format!(
            "Windows could not read the Personalize settings: {open_result:?}"
        ));
    }

    let name = wide_null("AppsUseLightTheme");
    let mut value = 1u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let read_result = unsafe {
        RegQueryValueExW(
            key,
            windows::core::PCWSTR(name.as_ptr()),
            None,
            None,
            Some((&mut value as *mut u32).cast::<u8>()),
            Some(&mut size),
        )
    };
    let _ = unsafe { RegCloseKey(key) };

    if read_result.is_err() {
        Err(format!(
            "Windows could not read AppsUseLightTheme: {read_result:?}"
        ))
    } else if value == 0 {
        Ok("dark")
    } else {
        Ok("light")
    }
}

#[cfg(target_os = "windows")]
fn apply_windows_theme(use_light_theme: bool) -> Result<(), String> {
    use windows::Win32::Foundation::{LPARAM, WPARAM};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegCreateKeyW, HKEY, HKEY_CURRENT_USER,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG,
        WM_DWMCOLORIZATIONCOLORCHANGED, WM_SETTINGCHANGE, WM_THEMECHANGED,
    };

    let path = wide_null(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let mut key = HKEY::default();
    let result = unsafe {
        RegCreateKeyW(
            HKEY_CURRENT_USER,
            windows::core::PCWSTR(path.as_ptr()),
            &mut key,
        )
    };
    if result.is_err() {
        return Err(format!(
            "Windows could not open the Personalize settings: {result:?}"
        ));
    }

    let value = u32::from(use_light_theme);
    let write_result = write_theme_value(key, "AppsUseLightTheme", value)
        .and_then(|_| write_theme_value(key, "SystemUsesLightTheme", value))
        // Windows disables taskbar accent coloring in light mode. Keeping the
        // old dark-mode value can leave secondary Explorer taskbars stale.
        .and_then(|_| {
            if use_light_theme {
                write_theme_value(key, "ColorPrevalence", 0)
            } else {
                Ok(())
            }
        });
    let _ = unsafe { RegCloseKey(key) };
    write_result?;

    // Match the complete notification sequence used by PowerToys Light Switch.
    // WM_SETTINGCHANGE updates theme preferences; WM_THEMECHANGED refreshes
    // every top-level Explorer surface, including secondary-monitor taskbars.
    let setting = wide_null("ImmersiveColorSet");
    unsafe {
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            WPARAM(0),
            LPARAM(setting.as_ptr() as isize),
            SMTO_ABORTIFHUNG,
            1_000,
            None,
        );
        let _ = SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_THEMECHANGED,
            WPARAM(0),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            1_000,
            None,
        );

        if use_light_theme {
            let _ = SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_DWMCOLORIZATIONCOLORCHANGED,
                WPARAM(0),
                LPARAM(0),
                SMTO_ABORTIFHUNG,
                1_000,
                None,
            );
        }
    }

    Ok(())
}

#[tauri::command]
pub fn set_system_theme(theme: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        match theme.as_str() {
            "light" => apply_windows_theme(true),
            "dark" => apply_windows_theme(false),
            _ => Err("Theme must be either 'light' or 'dark'.".to_string()),
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = theme;
        Err("System theme switching is only available on Windows.".to_string())
    }
}

#[tauri::command]
pub fn get_system_theme() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        read_windows_theme().map(str::to_string)
    }

    #[cfg(not(target_os = "windows"))]
    {
        Err("System theme detection is only available on Windows.".to_string())
    }
}
