use std::mem::size_of;

use windows::Win32::{
    Devices::Display::{
        DisplayConfigGetDeviceInfo, DisplayConfigSetDeviceInfo, GetDisplayConfigBufferSizes,
        QueryDisplayConfig, DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
        DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL, DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
        DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME, DISPLAYCONFIG_DEVICE_INFO_HEADER,
        DISPLAYCONFIG_DEVICE_INFO_SET_ADVANCED_COLOR_STATE, DISPLAYCONFIG_DEVICE_INFO_TYPE,
        DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO, DISPLAYCONFIG_MODE_INFO,
        DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL, DISPLAYCONFIG_PATH_INFO,
        DISPLAYCONFIG_SDR_WHITE_LEVEL, DISPLAYCONFIG_SET_ADVANCED_COLOR_STATE,
        DISPLAYCONFIG_SOURCE_DEVICE_NAME, DISPLAYCONFIG_TARGET_DEVICE_NAME, QDC_ONLY_ACTIVE_PATHS,
    },
    Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, LUID},
    Graphics::Gdi::{
        ChangeDisplaySettingsExW, EnumDisplaySettingsExW, CDS_TEST, CDS_TYPE, DEVMODEW,
        DISP_CHANGE_SUCCESSFUL, DM_DISPLAYFREQUENCY, ENUM_CURRENT_SETTINGS,
        ENUM_DISPLAY_SETTINGS_FLAGS,
    },
};

const DISPLAYCONFIG_DEVICE_INFO_SET_SDR_WHITE_LEVEL: i32 = -18; // 0xFFFFFFEE

#[derive(Clone, Debug)]
pub struct DisplayInfo {
    pub adapter_id: LUID,
    pub target_id: u32,
    pub name: String,
    pub device_path: String,
    pub built_in: bool,
    pub refresh: (u32, u32),
    pub gdi_device_name: String,
    pub hdr_supported: bool,
    pub hdr_enabled: bool,
    pub advanced_color_force_disabled: bool,
    pub sdr_white_nits: Option<u32>,
}

#[repr(C)]
struct DisplayConfigSetSdrWhiteLevel {
    header: DISPLAYCONFIG_DEVICE_INFO_HEADER,
    sdr_white_level: u32,
    final_value: u8,
    padding: [u8; 3],
}

pub fn active_displays() -> Result<Vec<DisplayInfo>, String> {
    let paths = query_active_paths()?;
    paths.iter().map(display_info_for_path).collect()
}

/// WMI controls the internal panel, whereas DDC/CI addresses an external monitor.
/// Resolve the selected GDI source without depending on HDR capabilities or EDID.
pub fn uses_internal_brightness(device_name: &str) -> Result<bool, String> {
    let paths = query_active_paths()?;
    let mut targets = Vec::new();
    for path in &paths {
        let mut source_name = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
            header: header(
                DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
                size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>(),
                path.sourceInfo.adapterId,
                path.sourceInfo.id,
            ),
            ..Default::default()
        };
        win32_result(
            unsafe { DisplayConfigGetDeviceInfo(&mut source_name.header) },
            "matching the brightness display",
        )?;
        targets.push((
            utf16z_to_string(&source_name.viewGdiDeviceName).unwrap_or_default(),
            matches!(path.targetInfo.outputTechnology.0, 6 | 11 | 13 | -2147483648),
        ));
    }
    brightness_target(device_name, &targets)
}

fn brightness_target(device_name: &str, targets: &[(String, bool)]) -> Result<bool, String> {
    let selected: Vec<_> = targets.iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case(device_name))
        .collect();
    if selected.is_empty() {
        return Err("The selected brightness display is no longer connected.".into());
    }
    let internal = selected.iter().any(|(_, built_in)| *built_in);
    if internal && targets.iter().filter(|(_, built_in)| *built_in).count() != 1 {
        return Err("Cannot safely select brightness among multiple internal panels.".into());
    }
    Ok(internal)
}

pub(crate) fn refresh_matches(actual: (u32, u32), expected: (u32, u32)) -> bool {
    actual.0 > 0
        && actual.1 > 0
        && expected.0 > 0
        && expected.1 > 0
        && u64::from(actual.0) * u64::from(expected.1)
            == u64::from(expected.0) * u64::from(actual.1)
}

pub(crate) fn nominal_refresh(refresh: (u32, u32)) -> Option<u32> {
    if refresh.0 == 0 || refresh.1 == 0 {
        return None;
    }
    u32::try_from((u64::from(refresh.0) + u64::from(refresh.1) / 2) / u64::from(refresh.1)).ok()
}

pub fn set_refresh(display: &DisplayInfo, refresh: (u32, u32)) -> Result<(), String> {
    let hz = nominal_refresh(refresh)
        .filter(|hz| *hz > 1)
        .ok_or("Cannot apply an invalid display refresh rate")?;
    let paths = query_active_paths()?;
    let index = paths
        .iter()
        .position(|p| {
            p.targetInfo.adapterId == display.adapter_id && p.targetInfo.id == display.target_id
        })
        .ok_or("The selected display is no longer active")?;
    let current = display_info_for_path(&paths[index])?;
    if !current
        .device_path
        .eq_ignore_ascii_case(&display.device_path)
    {
        return Err("The selected display changed before applying its refresh rate".into());
    }
    if refresh_matches(current.refresh, refresh) {
        return Ok(());
    }
    let source = paths[index].sourceInfo;
    if paths
        .iter()
        .filter(|p| p.sourceInfo.adapterId == source.adapterId && p.sourceInfo.id == source.id)
        .count()
        != 1
    {
        return Err("Use Extend displays before changing the HDR Boost refresh rate".into());
    }
    let name: Vec<u16> = current
        .gdi_device_name
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let name = windows::core::PCWSTR(name.as_ptr());
    let mut mode = DEVMODEW {
        dmSize: size_of::<DEVMODEW>() as u16,
        ..Default::default()
    };
    if !unsafe {
        EnumDisplaySettingsExW(
            name,
            ENUM_CURRENT_SETTINGS,
            &mut mode,
            ENUM_DISPLAY_SETTINGS_FLAGS(0),
        )
    }
    .as_bool()
    {
        return Err("Cannot read the current display mode for its refresh-rate change".into());
    }
    // Use the driver's nominal Hz mode, as the app's refresh control does. Change
    // only frequency on this device; retain resolution, position and orientation.
    // Keep the exact DisplayConfig fraction in recovery for the final read-back.
    mode.dmDisplayFrequency = hz;
    mode.dmFields = DM_DISPLAYFREQUENCY;
    for flags in [CDS_TEST, CDS_TYPE(0)] {
        let result = unsafe { ChangeDisplaySettingsExW(name, Some(&mode), None, flags, None) };
        if result != DISP_CHANGE_SUCCESSFUL {
            return Err(format!(
                "Windows failed while changing display refresh rate to {hz} Hz (error {})",
                result.0
            ));
        }
    }
    // No CDS_UPDATEREGISTRY: boost does not replace the user's saved mode.
    Ok(())
}

pub fn set_hdr(display: &DisplayInfo, enabled: bool) -> Result<(), String> {
    let mut packet = DISPLAYCONFIG_SET_ADVANCED_COLOR_STATE {
        header: header(
            DISPLAYCONFIG_DEVICE_INFO_SET_ADVANCED_COLOR_STATE,
            size_of::<DISPLAYCONFIG_SET_ADVANCED_COLOR_STATE>(),
            display.adapter_id,
            display.target_id,
        ),
        ..Default::default()
    };
    packet.Anonymous.value = u32::from(enabled);

    let result = unsafe { DisplayConfigSetDeviceInfo(&packet.header) };
    win32_result(result, "changing HDR state")
}

pub fn set_sdr_white(display: &DisplayInfo, nits: u32) -> Result<(), String> {
    if !(80..=480).contains(&nits) {
        return Err(format!(
            "SDR white must be between 80 and 480 nits, got {nits}"
        ));
    }

    set_sdr_white_packet(display, nits)
}

fn set_sdr_white_packet(display: &DisplayInfo, nits: u32) -> Result<(), String> {
    let rounded_nits = nits.div_ceil(4) * 4;
    let packet = DisplayConfigSetSdrWhiteLevel {
        header: header(
            DISPLAYCONFIG_DEVICE_INFO_TYPE(DISPLAYCONFIG_DEVICE_INFO_SET_SDR_WHITE_LEVEL),
            size_of::<DisplayConfigSetSdrWhiteLevel>(),
            display.adapter_id,
            display.target_id,
        ),
        sdr_white_level: rounded_nits * 1000 / 80,
        final_value: 1,
        padding: [0; 3],
    };

    let result = unsafe { DisplayConfigSetDeviceInfo(&packet.header) };
    win32_result(result, "changing SDR white level")
}

fn query_active_paths() -> Result<Vec<DISPLAYCONFIG_PATH_INFO>, String> {
    for _ in 0..4 {
        let mut path_count = 0;
        let mut mode_count = 0;
        let size_result = unsafe {
            GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
        };
        if size_result != ERROR_SUCCESS {
            return Err(format!(
                "GetDisplayConfigBufferSizes failed with Win32 error {}",
                size_result.0
            ));
        }

        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
        let query_result = unsafe {
            QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut path_count,
                paths.as_mut_ptr(),
                &mut mode_count,
                modes.as_mut_ptr(),
                None,
            )
        };

        if query_result == ERROR_INSUFFICIENT_BUFFER {
            continue;
        }
        if query_result != ERROR_SUCCESS {
            return Err(format!(
                "QueryDisplayConfig failed with Win32 error {}",
                query_result.0
            ));
        }

        paths.truncate(path_count as usize);
        return Ok(paths);
    }

    Err("The display topology kept changing; try again in a moment".into())
}

fn display_info_for_path(path: &DISPLAYCONFIG_PATH_INFO) -> Result<DisplayInfo, String> {
    let adapter_id = path.targetInfo.adapterId;
    let target_id = path.targetInfo.id;

    let mut target_name = DISPLAYCONFIG_TARGET_DEVICE_NAME {
        header: header(
            DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
            size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>(),
            adapter_id,
            target_id,
        ),
        ..Default::default()
    };
    let target_result = unsafe { DisplayConfigGetDeviceInfo(&mut target_name.header) };
    win32_result(target_result, "reading display name")?;
    let mut source_name = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
        header: header(
            DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
            size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>(),
            path.sourceInfo.adapterId,
            path.sourceInfo.id,
        ),
        ..Default::default()
    };
    win32_result(
        unsafe { DisplayConfigGetDeviceInfo(&mut source_name.header) },
        "reading source display name",
    )?;

    let mut advanced = DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO {
        header: header(
            DISPLAYCONFIG_DEVICE_INFO_GET_ADVANCED_COLOR_INFO,
            size_of::<DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO>(),
            adapter_id,
            target_id,
        ),
        ..Default::default()
    };
    let advanced_result = unsafe { DisplayConfigGetDeviceInfo(&mut advanced.header) };
    win32_result(advanced_result, "reading HDR capabilities")?;
    let advanced_flags = unsafe { advanced.Anonymous.value };

    let mut white = DISPLAYCONFIG_SDR_WHITE_LEVEL {
        header: header(
            DISPLAYCONFIG_DEVICE_INFO_GET_SDR_WHITE_LEVEL,
            size_of::<DISPLAYCONFIG_SDR_WHITE_LEVEL>(),
            adapter_id,
            target_id,
        ),
        ..Default::default()
    };
    let white_result = unsafe { DisplayConfigGetDeviceInfo(&mut white.header) };

    let name = utf16z_to_string(&target_name.monitorFriendlyDeviceName).unwrap_or_else(|| {
        if path.targetInfo.outputTechnology == DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL {
            "Built-in Mini-LED Display".to_string()
        } else {
            format!("Display {}", target_id + 1)
        }
    });

    Ok(DisplayInfo {
        adapter_id,
        target_id,
        name,
        device_path: utf16z_to_string(&target_name.monitorDevicePath).unwrap_or_default(),
        built_in: matches!(
            path.targetInfo.outputTechnology.0,
            6 | 11 | 13 | -2147483648
        ),
        refresh: (
            path.targetInfo.refreshRate.Numerator,
            path.targetInfo.refreshRate.Denominator,
        ),
        gdi_device_name: utf16z_to_string(&source_name.viewGdiDeviceName).unwrap_or_default(),
        hdr_supported: advanced_flags & 0b0001 != 0,
        hdr_enabled: advanced_flags & 0b0010 != 0,
        advanced_color_force_disabled: advanced_flags & 0b1000 != 0,
        sdr_white_nits: (white_result == 0).then_some(white.SDRWhiteLevel * 80 / 1000),
    })
}

fn header(
    kind: DISPLAYCONFIG_DEVICE_INFO_TYPE,
    size: usize,
    adapter_id: LUID,
    target_id: u32,
) -> DISPLAYCONFIG_DEVICE_INFO_HEADER {
    DISPLAYCONFIG_DEVICE_INFO_HEADER {
        r#type: kind,
        size: size as u32,
        adapterId: adapter_id,
        id: target_id,
    }
}

fn win32_result(code: i32, action: &str) -> Result<(), String> {
    if code == 0 {
        Ok(())
    } else {
        Err(format!("Windows failed while {action} (error {code})"))
    }
}

fn utf16z_to_string(value: &[u16]) -> Option<String> {
    let end = value
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(value.len());
    (end > 0).then(|| String::from_utf16_lossy(&value[..end]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brightness_routes_only_the_selected_internal_panel_to_wmi() {
        let targets = vec![("DISPLAY1".into(), true), ("DISPLAY2".into(), false)];
        assert_eq!(brightness_target("display1", &targets), Ok(true));
        assert_eq!(brightness_target("DISPLAY2", &targets), Ok(false));
        assert!(brightness_target("DISCONNECTED", &targets).is_err());
    }

    #[test]
    fn brightness_does_not_guess_between_internal_panels() {
        let targets = vec![("DISPLAY1".into(), true), ("DISPLAY2".into(), true)];
        assert!(brightness_target("DISPLAY1", &targets).is_err());
    }

    #[test]
    fn private_sdr_packet_matches_windows_layout() {
        assert_eq!(size_of::<DisplayConfigSetSdrWhiteLevel>(), 28);
    }

    #[test]
    fn utf16_names_stop_at_nul() {
        assert_eq!(
            utf16z_to_string(&['H' as u16, 'D' as u16, 'R' as u16, 0, 'x' as u16]),
            Some("HDR".to_string())
        );
    }

    #[test]
    fn refresh_comparison_preserves_fractional_rates() {
        assert!(refresh_matches((165_000, 1000), (165, 1)));
        assert!(refresh_matches((60_000, 1001), (120_000, 2002)));
        assert!(!refresh_matches((60_000, 1001), (60, 1)));
        assert!(!refresh_matches((0, 0), (60, 1)));
        assert!(!refresh_matches((60, 1), (0, 0)));
        assert!(refresh_matches((u32::MAX, u32::MAX), (1, 1)));
        assert_eq!(nominal_refresh((110_817_000, 671_609)), Some(165));
        assert_eq!(nominal_refresh((60_000, 1001)), Some(60));
        assert_eq!(nominal_refresh((0, 1)), None);
        assert_eq!(nominal_refresh((60, 0)), None);
    }
}
