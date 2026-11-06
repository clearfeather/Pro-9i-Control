use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tauri::{Manager, Position, WindowEvent};

mod cli;
mod battery_controls;
mod brightness_monitor;
mod display_monitor;
mod displays;
mod fakeDisplays;
mod hdr_boost;
#[cfg(target_os = "windows")]
mod hotkeys;
mod lenovo_battery;
mod lenovo_power;
mod lenovo_usb;
mod positioning;
mod power_source;
mod power_brightness;
mod settings;
mod state_file;
mod syncthing;
mod system_theme;
#[cfg(target_os = "windows")]
mod winDisplays;
pub mod winHdr;

const AUTOSTART_BASE_LABEL: &str = "Start at login";
static MAIN_WINDOW_OPEN: AtomicBool = AtomicBool::new(false);
static FOCUS_OUT_TOKEN: AtomicU64 = AtomicU64::new(0);
static EXIT_CLEANUP_DONE: AtomicBool = AtomicBool::new(false);
static EXIT_CLEANUP_STARTED: AtomicBool = AtomicBool::new(false);

fn autostart_label(enabled: bool) -> String {
    if enabled {
        format!("{AUTOSTART_BASE_LABEL} ✓")
    } else {
        AUTOSTART_BASE_LABEL.to_string()
    }
}

pub(crate) fn reveal_main_window(app_handle: &tauri::AppHandle) -> bool {
    if let Some(window) = app_handle.get_webview_window("main") {
        FOCUS_OUT_TOKEN.fetch_add(1, Ordering::SeqCst);
        let should_reposition = window
            .outer_position()
            .map(|position| !positioning::is_position_on_any_monitor(app_handle, position))
            .unwrap_or(true);
        if should_reposition {
            let _ = window.set_position(positioning::compute_default_window_position(
                app_handle, &window,
            ));
        }
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_ignore_cursor_events(false);
        let _ = window.set_focus();
        MAIN_WINDOW_OPEN.store(true, Ordering::SeqCst);
        true
    } else {
        false
    }
}

fn park_main_window(app_handle: &tauri::AppHandle) -> bool {
    if let Some(window) = app_handle.get_webview_window("main") {
        FOCUS_OUT_TOKEN.fetch_add(1, Ordering::SeqCst);
        let _ = window.hide();
        MAIN_WINDOW_OPEN.store(false, Ordering::SeqCst);
        true
    } else {
        false
    }
}

fn toggle_main_window_for_tray_click(
    app_handle: &tauri::AppHandle,
    tray_position: Position,
) -> bool {
    if MAIN_WINDOW_OPEN.load(Ordering::SeqCst) {
        park_main_window(app_handle)
    } else if let Some(window) = app_handle.get_webview_window("main") {
        let pos =
            positioning::compute_window_position_for_tray_click(app_handle, &window, tray_position);
        let _ = window.set_position(pos);
        reveal_main_window(app_handle)
    } else {
        false
    }
}

pub fn run() {
    match cli::run_cli() {
        Ok(true) => {
            return;
        }
        Ok(false) => {
            // No CLI command or explicit --ui, continue with GUI
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }

    use tauri::{
        menu::{Menu, MenuItem},
        tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
        WebviewUrl, WebviewWindowBuilder,
    };
    use tauri_plugin_notification::NotificationExt;

    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        // Keep the legacy autostart key so upgrades preserve the user's
        // existing Start at login preference after the product rename.
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("HZHDR")
                .build(),
        )
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            hdr_boost::get_hdr_boost_status,
            hdr_boost::start_hdr_boost,
            hdr_boost::stop_hdr_boost,
            hdr_boost::set_hdr_boost_strength,
            hdr_boost::restore_hdr_boost,
            displays::get_all_monitors,
            displays::set_monitor_resolution,
            displays::set_monitor_orientation,
            displays::get_monitor_brightness,
            displays::set_monitor_brightness,
            displays::identify_monitors,
            displays::set_monitor_scale,
            displays::enable_hdr,
            displays::set_monitor_input_source,
            displays::get_monitor_input_source,
            displays::get_monitor_ddc_caps,
            displays::set_monitor_power,
            lenovo_battery::get_conservation_mode,
            lenovo_battery::set_conservation_mode,
            lenovo_power::get_lenovo_power_mode,
            lenovo_power::set_lenovo_power_mode,
            lenovo_usb::get_always_on_usb,
            lenovo_usb::set_always_on_usb,
            syncthing::get_syncthing_status,
            syncthing::set_syncthing_paused,
            syncthing::set_syncthing_auto_pause,
            battery_controls::get_battery_controls,
            battery_controls::set_battery_control,
            system_theme::get_system_theme,
            system_theme::set_system_theme,
        ])
        .setup(|app| {
            let preview = cli::preview_mode();
            if let Ok(engine) = hdr_boost::controller() {
                if engine.status().recovery_pending { let _ = engine.recover(); }
            }
            // Log settings file location
            if let Ok(mut settings_path) = app.path().app_data_dir() {
                settings_path.push("settings.json");
                log::info!("Settings file: {}", settings_path.display());
            }

            // Start at login defaults to enabled. Unlike the old one-time marker,
            // the stored preference lets us recreate a missing registry entry
            // after an uninstall, rename, or Windows cleanup without overriding
            // a choice the user makes from the tray menu.
            #[cfg(all(desktop, not(debug_assertions)))]
            {
                use tauri_plugin_autostart::ManagerExt;
                if !preview {
                let preferred = crate::settings::autostart_preference_app(&app).unwrap_or(true);
                let manager = app.autolaunch();

                if preferred {
                    // Re-register enabled Windows entries so upgrades always
                    // point at the current installed executable.
                    #[cfg(target_os = "windows")]
                    if manager.is_enabled().unwrap_or(false) {
                        if let Err(error) = manager.disable() {
                            log::warn!("Failed to refresh Start at login entry: {error}");
                        }
                    }

                    if !manager.is_enabled().unwrap_or(false) {
                        if let Err(error) = manager.enable() {
                            log::warn!("Failed to enable Start at login: {error}");
                        }
                    }
                } else if manager.is_enabled().unwrap_or(false) {
                    if let Err(error) = manager.disable() {
                        log::warn!("Failed to disable Start at login: {error}");
                    }
                }

                crate::settings::save_autostart_preference(app.handle(), preferred);
                }
            }
            // Show a notification on startup to inform the user the app is running in the tray
            if !preview && crate::settings::should_show_startup_notification_app(&app) {
                if let Err(error) = app.notification()
                    .builder()
                    .title("Pro 9i Control")
                    .body("Pro 9i Control is running in the system tray.")
                    .show()
                {
                    log::warn!("Failed to show startup notification: {error}");
                }
            }

            // Build a tray context menu
            let show_item = MenuItem::with_id(app, "show", "Show", true, Some(""))?;
            let autostart_item = {
                use tauri_plugin_autostart::ManagerExt;
                let enabled = app.autolaunch().is_enabled().unwrap_or(false);
                let label = autostart_label(enabled);
                MenuItem::with_id(app, "autostart_toggle", label.as_str(), true, Some(""))?
            };
            let about_item = MenuItem::with_id(app, "about", "About", true, Some(""))?;
            let boost_stop = MenuItem::with_id(app, "boost_stop", "Stop HDR Boost / restore", true, Some(""))?;
            let exit_item = MenuItem::with_id(app, "exit", "Exit", true, Some(""))?;
            let menu =
                Menu::with_items(app, &[&show_item, &autostart_item, &about_item, &boost_stop, &exit_item])?;

            MAIN_WINDOW_OPEN.store(false, Ordering::SeqCst);

            let tray_builder = TrayIconBuilder::new()
                .menu(&menu)
                .show_menu_on_left_click(false)
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Pro 9i Control");

            let autostart_item_handle = autostart_item.clone();
            let tray_builder =
                tray_builder.on_menu_event(move |app_handle, event| match event.id().as_ref() {
                    "show" => {
                        reveal_main_window(&app_handle);
                    }
                    "autostart_toggle" => {
                        use tauri_plugin_autostart::ManagerExt;
                        let manager = app_handle.autolaunch();
                        let currently_enabled = manager.is_enabled().unwrap_or(false);
                        let desired = !currently_enabled;
                        let result = if currently_enabled {
                            manager.disable()
                        } else {
                            manager.enable()
                        };

                        match result {
                            Ok(()) => {
                                crate::settings::save_autostart_preference(
                                    app_handle,
                                    desired,
                                );
                                let new_label = autostart_label(desired);
                                let _ = autostart_item_handle.set_text(new_label.as_str());
                            }
                            Err(error) => {
                                log::warn!("Failed to change Start at login: {error}");
                            }
                        }
                    }
                    "about" => {
                        if let Some(window) = app_handle.get_webview_window("about") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        } else {
                            let _ = WebviewWindowBuilder::new(
                                app_handle,
                                "about",
                                WebviewUrl::App("index.html#about".into()),
                            )
                            .title("About Pro 9i Control")
                            .inner_size(360.0, 280.0)
                            .resizable(false)
                            .minimizable(false)
                            .maximizable(false)
                            .skip_taskbar(true)
                            .visible(true)
                            .build();
                        }
                    }
                    "boost_stop" => {
                        if let Ok(engine) = hdr_boost::controller() {
                            if engine.status().recovery_pending && !engine.status().busy() { let _ = engine.recover(); }
                            else { engine.stop(); }
                        }
                    }
                    "exit" => {
                        app_handle.exit(0);
                    }
                    _ => {}
                });

            let tray_builder = tray_builder.on_tray_icon_event(move |tray, event| {
                if let TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    rect,
                    ..
                } = event
                {
                    let app = tray.app_handle();

                    toggle_main_window_for_tray_click(&app, rect.position);
                }
            });

            tray_builder.build(app)?;

            // Start monitoring for display changes
            app.manage(battery_controls::Controller::start(app.handle().clone(), preview));
            app.manage(syncthing::Controller::start(app.handle().clone(), preview));
            brightness_monitor::start(app.handle().clone());
            if let Err(e) = display_monitor::start_display_monitor(app.handle().clone()) {
                log::warn!("Failed to start display monitor: {}", e);
            }

            // Start global hotkey service (Windows only) if enabled by settings
            #[cfg(target_os = "windows")]
            {
                if crate::settings::should_register_brightness_hotkeys_app(&app) {
                    crate::hotkeys::start_hotkey_service(app.handle().clone());
                } else {
                    log::info!(
                        "Skipping brightness hotkeys startup: keyboardBrightnessShortcut is 'system'"
                    );
                }
            }

            // Keep the main window genuinely hidden until a tray click. Parking a visible
            // window far off-screen lets Windows move it back on-screen after display or
            // DPI changes, which makes the panel appear unexpectedly.
            if let Some(window) = app.get_webview_window("main") {
                let app_handle_for_event = app.handle().clone();
                window.on_window_event(move |event| {
                    match event {
                        WindowEvent::Resized(_) => {
                            if let Some(window) = app_handle_for_event.get_webview_window("main") {
                                if let Err(error) = positioning::keep_window_in_work_area(&window) {
                                    log::warn!("Failed to keep resized controls in the work area: {error}");
                                }
                            }
                        }
                        WindowEvent::Focused(true) => {
                            FOCUS_OUT_TOKEN.fetch_add(1, Ordering::SeqCst);
                        }
                        WindowEvent::Focused(false) => {
                            if preview { return; }
                            if !crate::settings::should_hide_ui_on_focus_out_handle(
                                &app_handle_for_event,
                            ) {
                                return;
                            }

                            let token = FOCUS_OUT_TOKEN.fetch_add(1, Ordering::SeqCst) + 1;
                            let app_handle = app_handle_for_event.clone();
                            tauri::async_runtime::spawn(async move {
                                std::thread::sleep(Duration::from_millis(300));
                                if FOCUS_OUT_TOKEN.load(Ordering::SeqCst) == token
                                    && MAIN_WINDOW_OPEN.load(Ordering::SeqCst)
                                {
                                    park_main_window(&app_handle);
                                }
                            });
                        }
                        WindowEvent::Destroyed => {
                            MAIN_WINDOW_OPEN.store(false, Ordering::SeqCst);
                        }
                        _ => {}
                    }
                });
                // Ensure the window does not appear in the taskbar
                let _ = window.set_skip_taskbar(true);
                let _ = window.hide();
                MAIN_WINDOW_OPEN.store(false, Ordering::SeqCst);
            }
            if preview { reveal_main_window(app.handle()); }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            tauri::RunEvent::ExitRequested { api, .. } => {
                if !EXIT_CLEANUP_DONE.load(Ordering::SeqCst) {
                    api.prevent_exit();
                    if !EXIT_CLEANUP_STARTED.swap(true, Ordering::SeqCst) {
                        let app = app.clone();
                        std::thread::spawn(move || {
                            hdr_boost::stop_and_wait();
                            brightness_monitor::stop();
                            battery_controls::shutdown(&app);
                            syncthing::shutdown(&app);
                            EXIT_CLEANUP_DONE.store(true, Ordering::SeqCst);
                            app.exit(0);
                        });
                    }
                }
            }
            tauri::RunEvent::Exit if !EXIT_CLEANUP_DONE.load(Ordering::SeqCst) => {
                hdr_boost::stop_and_wait();
                brightness_monitor::stop();
                battery_controls::shutdown(app);
                syncthing::shutdown(app);
            }
            _ => (),
        });
}
