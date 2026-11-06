use crate::displays::DisplayInfo;
use clap::{Parser, Subcommand};

#[cfg(target_os = "windows")]
fn attach_console_if_cli_invocation() {
    use std::os::windows::io::AsRawHandle;

    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::System::Console::{
        AttachConsole, GetStdHandle, SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE,
        STD_HANDLE, STD_INPUT_HANDLE, STD_OUTPUT_HANDLE,
    };

    // If no extra args were provided, we likely want to start the GUI (no console work needed).
    let mut args = std::env::args_os();
    let _exe = args.next();
    if args.next().is_none() {
        return;
    }

    // If the parent has a console (PowerShell/cmd), attach so stdout/stderr are visible.
    // If it fails (e.g., launched from Explorer), we intentionally do nothing to avoid
    // spawning a new console window.
    unsafe {
        let _ = AttachConsole(ATTACH_PARENT_PROCESS);
    }

    fn handle_invalid(h: HANDLE) -> bool {
        h.0 == 0 || h.0 == -1
    }

    fn std_handle_invalid(which: STD_HANDLE) -> bool {
        unsafe {
            match GetStdHandle(which) {
                Ok(h) => handle_invalid(h),
                Err(_) => true,
            }
        }
    }

    // Ensure the process std handles are usable (GUI-subsystem apps often start with null stdio).
    // We open CONIN$/CONOUT$ only after attaching to a parent console.
    unsafe {
        if std_handle_invalid(STD_OUTPUT_HANDLE) {
            if let Ok(conout) = std::fs::OpenOptions::new().write(true).open("CONOUT$") {
                let handle = HANDLE(conout.as_raw_handle() as isize);
                let _ = SetStdHandle(STD_OUTPUT_HANDLE, handle);
                std::mem::forget(conout);
            }
        }

        if std_handle_invalid(STD_ERROR_HANDLE) {
            if let Ok(conerr) = std::fs::OpenOptions::new().write(true).open("CONOUT$") {
                let handle = HANDLE(conerr.as_raw_handle() as isize);
                let _ = SetStdHandle(STD_ERROR_HANDLE, handle);
                std::mem::forget(conerr);
            }
        }

        if std_handle_invalid(STD_INPUT_HANDLE) {
            if let Ok(conin) = std::fs::OpenOptions::new().read(true).open("CONIN$") {
                let handle = HANDLE(conin.as_raw_handle() as isize);
                let _ = SetStdHandle(STD_INPUT_HANDLE, handle);
                std::mem::forget(conin);
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn attach_console_if_cli_invocation() {}

#[derive(Parser)]
#[command(name = "pro-9i-control")]
#[command(about = "Pro 9i Control CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Open the local preview without registering it for Start at login
    Preview,
    /// Read-only Lenovo HDR Boost diagnostic
    BoostDiagnose,
    /// Read the current Lenovo mode, panel brightness, and Windows Energy Saver state
    PowerBrightnessDiagnose,
    /// Observe Windows brightness changes without changing settings
    BrightnessWatch {
        #[arg(long, default_value_t = 10, value_parser = clap::value_parser!(u64).range(1..=60))]
        seconds: u64,
    },
    /// Briefly test Saver brightness restoration, then restore starting settings
    PowerBrightnessTest {
        #[arg(long)]
        journal: std::path::PathBuf,
        #[arg(long)]
        recover: bool,
    },
    /// Restore a saved boost session without starting the GUI
    BoostRestore,
    /// Bounded HDR renderer test that verifies Windows brightness stays unchanged
    BoostTest {
        #[arg(long, default_value = "live")]
        scenario: String,
    },
    /// List all monitors with their properties
    List,
    /// Set input source for a monitor
    SetInput {
        /// Monitor index (0-based)
        #[arg(long)]
        monitor_idx: usize,
        /// Input source value
        #[arg(long)]
        source: String,
    },
    /// Get current input source for a monitor
    GetInput {
        /// Monitor index (0-based)
        #[arg(long)]
        monitor_idx: usize,
    },
    /// Set resolution for a monitor
    SetResolution {
        /// Monitor index (0-based)
        #[arg(long)]
        monitor_idx: usize,
        /// Width in pixels
        #[arg(long)]
        width: u32,
        /// Height in pixels
        #[arg(long)]
        height: u32,
        /// Refresh rate in Hz (optional)
        #[arg(long)]
        refresh_hz: Option<u32>,
    },
    /// Set orientation for a monitor
    SetOrientation {
        /// Monitor index (0-based)
        #[arg(long)]
        monitor_idx: usize,
        /// Orientation in degrees (0, 90, 180, 270)
        #[arg(long)]
        degrees: u32,
    },
    /// Set brightness for a monitor
    SetBrightness {
        /// Monitor index (0-based)
        #[arg(long)]
        monitor_idx: usize,
        /// Brightness percentage (0-100)
        #[arg(long)]
        percent: u32,
    },
    /// Get brightness for a monitor
    GetBrightness {
        /// Monitor index (0-based)
        #[arg(long)]
        monitor_idx: usize,
    },
    /// Set scale for a monitor
    SetScale {
        /// Monitor index (0-based)
        #[arg(long)]
        monitor_idx: usize,
        /// Scale percentage (100, 125, 150, 175, 200, etc.)
        #[arg(long)]
        percent: u32,
    },
    /// Enable or disable HDR for a monitor
    SetHdr {
        /// Monitor index (0-based)
        #[arg(long)]
        monitor_idx: usize,
        /// Enable HDR (true/false)
        #[arg(long)]
        enable: bool,
    },
    /// Get DDC/CI capabilities for a monitor
    GetCaps {
        /// Monitor index (0-based)
        #[arg(long)]
        monitor_idx: usize,
    },
    /// Start the GUI application
    Ui,
}

pub fn run_cli() -> Result<bool, String> {
    attach_console_if_cli_invocation();
    let cli = Cli::parse();

    match cli.command {
        None => {
            // No command provided, should start GUI
            Ok(false)
        }
        Some(Commands::Ui | Commands::Preview) => {
            // Explicit UI command, start GUI
            Ok(false)
        }
        Some(command) => {
            // Process CLI command
            handle_command(command)?;
            Ok(true)
        }
    }
}

fn handle_command(command: Commands) -> Result<(), String> {
    let provider = crate::displays::active_provider();

    match command {
        Commands::BrightnessWatch { seconds } => {
            use std::io::Write;
            let events = lenovo_hdr_boost::BrightnessEvents::subscribe()?;
            println!("{}", serde_json::json!({"phase":"watching"}));
            let _ = std::io::stdout().flush();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(seconds);
            while std::time::Instant::now() < deadline {
                if let Some(brightness) = events.next(250)? {
                    println!("{}", serde_json::json!({"brightness":brightness}));
                    let _ = std::io::stdout().flush();
                }
            }
        },
        Commands::PowerBrightnessDiagnose => println!("{}", serde_json::to_string_pretty(&crate::power_brightness::snapshot()?).map_err(|e| e.to_string())?),
        Commands::PowerBrightnessTest { journal, recover } => crate::power_brightness::live_test(&journal, recover)?,
        Commands::BoostDiagnose => println!("{}", lenovo_hdr_boost::diagnose()?),
        Commands::BoostRestore => {
            let engine = crate::hdr_boost::controller()?;
            engine.recover()?;
            let status = engine.wait();
            println!("{}", status.message);
            if status.recovery_pending {
                return Err(status.message);
            }
        }
        Commands::BoostTest { scenario } => {
            use lenovo_hdr_boost::Scenario;
            let scenario = match scenario.as_str() {
                "live" => Scenario::Live,
                "idle" => Scenario::Idle,
                "battery" => Scenario::Battery,
                "saver" => Scenario::BatterySaver,
                "power-transition" => Scenario::PowerTransition,
                "display-change" => Scenario::DisplayChange,
                "suspend" => Scenario::Suspend,
                "emergency" => Scenario::Emergency,
                "failure" => Scenario::Failure,
                "panic" => Scenario::Panic,
                "stall" => Scenario::PresentationStall,
                "capture-stall" => Scenario::CaptureStall,
                _ => {
                    return Err(
                        "Use live, idle, battery, saver, power-transition, display-change, suspend, emergency, failure, panic, stall, or capture-stall".into(),
                    )
                }
            };
            let engine = crate::hdr_boost::controller()?;
            let before = lenovo_hdr_boost::diagnose()?;
            let before_brightness = lenovo_hdr_boost::current_brightness()?;
            println!("Before: {before}");
            let available = engine
                .refresh_availability()
                .available_device
                .ok_or("Lenovo panel unavailable")?;
            engine.start(
                available.clone(),
                lenovo_hdr_boost::DEFAULT_STRENGTH,
                true,
                scenario,
            )?;
            let watchdog = engine.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_secs(20));
                watchdog.stop();
            });
            let started = std::time::Instant::now();
            while engine.status().phase == "starting" && started.elapsed().as_secs() < 12 {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            let reached_running = engine.status().phase == "running";
            let interlock = reached_running
                && provider
                    .set_monitor_brightness(available, u32::from(before_brightness))
                    .is_err();
            let mut brightness_unchanged = reached_running;
            let mut boost_display_correct = reached_running;
            let mut brightness_samples = 0;
            while engine.status().phase == "running" {
                let display_check = lenovo_hdr_boost::verify_boost_display();
                if engine.status().phase != "running" {
                    break;
                }
                boost_display_correct &= display_check.is_ok();
                if let Err(error) = display_check {
                    println!("Display during boost: {error}");
                }
                let actual = lenovo_hdr_boost::current_brightness();
                brightness_unchanged &= actual == Ok(before_brightness);
                brightness_samples += 1;
                println!(
                    "Windows brightness during boost: {actual:?}; expected {before_brightness}%"
                );
                std::thread::sleep(std::time::Duration::from_millis(500));
            }
            let status = engine.wait();
            println!("{}", status.message);
            let after = lenovo_hdr_boost::diagnose()?;
            println!("After: {after}");
            let expected = match scenario {
                Scenario::Failure => status.message.contains("Injected rendering failure"),
                Scenario::Panic => status.message.contains("HDR renderer panicked"),
                Scenario::PresentationStall => status.message.contains("HDR presentation stalled"),
                Scenario::CaptureStall => status.message.contains("Desktop capture stalled"),
                _ => status.message == "Off. Original display settings restored.",
            };
            if status.recovery_pending
                || before != after
                || !interlock
                || !expected
                || !brightness_unchanged
                || !boost_display_correct
                || brightness_samples == 0
            {
                return Err(format!("Bounded boost test failed: running={reached_running}, interlock={interlock}, recovery={}, expected_result={expected}, brightness_unchanged={brightness_unchanged}, hdr_at_60_hz={boost_display_correct}, brightness_samples={brightness_samples}", status.recovery_pending));
            }
            println!("PASS: {scenario:?}; HDR at 60 Hz, {brightness_samples} unchanged Windows brightness samples, display-control interlock and HDR/SDR-white/refresh restoration verified.");
        }
        Commands::List => {
            let monitors = provider.get_all_monitors()?;
            if monitors.is_empty() {
                println!("No monitors found.");
                return Ok(());
            }

            for (idx, monitor) in monitors.iter().enumerate() {
                print_monitor_info(idx, monitor);
                if idx < monitors.len() - 1 {
                    println!();
                }
            }
        }
        Commands::SetInput {
            monitor_idx,
            source,
        } => {
            let monitors = provider.get_all_monitors()?;
            let device_name = get_device_name(&monitors, monitor_idx)?;
            provider.set_monitor_input_source(device_name, source.clone())?;
            println!(
                "Successfully set input source to '{}' for monitor {}",
                source, monitor_idx
            );
        }
        Commands::GetInput { monitor_idx } => {
            let monitors = provider.get_all_monitors()?;
            let device_name = get_device_name(&monitors, monitor_idx)?;
            let input = provider.get_monitor_input_source(device_name)?;
            println!("Monitor {} input source: {}", monitor_idx, input);
        }
        Commands::SetResolution {
            monitor_idx,
            width,
            height,
            refresh_hz,
        } => {
            let monitors = provider.get_all_monitors()?;
            let device_name = get_device_name(&monitors, monitor_idx)?;
            provider.set_monitor_resolution(device_name, width, height, refresh_hz)?;
            let refresh_str = refresh_hz
                .map(|hz| format!("@{}Hz", hz))
                .unwrap_or_default();
            println!(
                "Successfully set resolution to {}x{}{} for monitor {}",
                width, height, refresh_str, monitor_idx
            );
        }
        Commands::SetOrientation {
            monitor_idx,
            degrees,
        } => {
            if ![0, 90, 180, 270].contains(&degrees) {
                return Err("Orientation must be 0, 90, 180, or 270 degrees".to_string());
            }
            let monitors = provider.get_all_monitors()?;
            let device_name = get_device_name(&monitors, monitor_idx)?;
            provider.set_monitor_orientation(device_name, degrees)?;
            println!(
                "Successfully set orientation to {} degrees for monitor {}",
                degrees, monitor_idx
            );
        }
        Commands::SetBrightness {
            monitor_idx,
            percent,
        } => {
            if percent > 100 {
                return Err("Brightness must be between 0 and 100".to_string());
            }
            let monitors = provider.get_all_monitors()?;
            let device_name = get_device_name(&monitors, monitor_idx)?;
            provider.set_monitor_brightness(device_name, percent)?;
            println!(
                "Successfully set brightness to {}% for monitor {}",
                percent, monitor_idx
            );
        }
        Commands::GetBrightness { monitor_idx } => {
            let monitors = provider.get_all_monitors()?;
            let device_name = get_device_name(&monitors, monitor_idx)?;
            let brightness = provider.get_monitor_brightness(device_name)?;
            println!(
                "Monitor {} brightness: {}% (min: {}, max: {})",
                monitor_idx, brightness.current, brightness.min, brightness.max
            );
        }
        Commands::SetScale {
            monitor_idx,
            percent,
        } => {
            let monitors = provider.get_all_monitors()?;
            let device_name = get_device_name(&monitors, monitor_idx)?;
            provider.set_monitor_scale(device_name, percent)?;
            println!(
                "Successfully set scale to {}% for monitor {}",
                percent, monitor_idx
            );
        }
        Commands::SetHdr {
            monitor_idx,
            enable,
        } => {
            let monitors = provider.get_all_monitors()?;
            let device_name = get_device_name(&monitors, monitor_idx)?;
            provider.enable_hdr(device_name, enable)?;
            let status = if enable { "enabled" } else { "disabled" };
            println!("Successfully {} HDR for monitor {}", status, monitor_idx);
        }
        Commands::GetCaps { monitor_idx } => {
            let monitors = provider.get_all_monitors()?;
            let device_name = get_device_name(&monitors, monitor_idx)?;
            let caps = provider.get_monitor_ddc_caps(device_name)?;
            println!("Monitor {} DDC/CI Capabilities:", monitor_idx);
            println!("{}", caps);
        }
        Commands::Ui | Commands::Preview => unreachable!(),
    }

    Ok(())
}

fn get_device_name(monitors: &[DisplayInfo], idx: usize) -> Result<String, String> {
    monitors
        .get(idx)
        .map(|m| m.device_name.clone())
        .ok_or_else(|| {
            format!(
                "Monitor index {} not found. Use --list to see available monitors.",
                idx
            )
        })
}

fn print_monitor_info(idx: usize, monitor: &DisplayInfo) {
    println!("Monitor {} - {}", idx, monitor.friendly_name);
    println!("  Device:       {}", monitor.device_name);
    println!(
        "  Resolution:   {}x{}@{}Hz ({}bpp)",
        monitor.current.width,
        monitor.current.height,
        monitor.current.refresh_hz,
        monitor.current.bits_per_pixel
    );
    println!(
        "  Max Native:   {}x{}@{}Hz",
        monitor.max_native.width, monitor.max_native.height, monitor.max_native.refresh_hz
    );
    println!("  Orientation:  {}°", monitor.orientation);
    println!(
        "  Scale:        {}% (might be incorrect)",
        (monitor.scale * 100.0).round() as u32
    );

    // Show available scales
    if !monitor.scales.is_empty() {
        let scales_str: Vec<String> = monitor
            .scales
            .iter()
            .map(|s| {
                let percent = (s.scale * 100.0) as u32;
                if s.is_recommended {
                    format!("{}% (recommended)", percent)
                } else {
                    format!("{}%", percent)
                }
            })
            .collect();
        println!("  Scales:       {}", scales_str.join(", "));
    }

    println!("  HDR:          {}", monitor.hdr_status);
    println!(
        "  Position:     ({}, {})",
        monitor.position_x, monitor.position_y
    );
    println!(
        "  Primary:      {}",
        if monitor.is_primary { "Yes" } else { "No" }
    );
    println!(
        "  Active:       {}",
        if monitor.active { "Yes" } else { "No" }
    );
    println!(
        "  Built-in:     {}",
        if monitor.built_in { "Yes" } else { "No" }
    );
    println!("  Connection:   {}", monitor.connection);

    if !monitor.manufacturer.is_empty() || !monitor.model.is_empty() {
        println!("  Info:         {} {}", monitor.manufacturer, monitor.model);
    }

    if !monitor.serial.is_empty() {
        println!("  Serial:       {}", monitor.serial);
    }

    if monitor.year_of_manufacture > 0 {
        println!(
            "  Manufactured: Week {} of {}",
            monitor.week_of_manufacture, monitor.year_of_manufacture
        );
    }

    if let Some(supports_input) = monitor.supports_input_switch {
        println!(
            "  Input Switch: {}",
            if supports_input {
                "Supported"
            } else {
                "Not supported"
            }
        );
    }
}

/// Explicit UI checks must never redirect the installed app's autostart entry.
pub fn preview_mode() -> bool {
    std::env::args().any(|a| a == "preview")
}
