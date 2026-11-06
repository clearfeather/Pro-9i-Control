//! Bounded profiling host for the standalone Lenovo engine (no Tauri dependency).
use lenovo_hdr_boost::{diagnose, Controller, Scenario, DEFAULT_STRENGTH};
use std::{thread, time::Duration};

fn run() -> Result<(), String> {
    let before = diagnose()?;
    let engine = Controller::new()?;
    let device = engine
        .refresh_availability()
        .available_device
        .ok_or("Lenovo panel unavailable")?;
    println!("Before: {before}");
    let scenario = match std::env::args().nth(1).as_deref() {
        None | Some("live") => Scenario::Live,
        Some("idle") => Scenario::Idle,
        Some("battery") => Scenario::Battery,
        Some("saver") => Scenario::BatterySaver,
        Some("power-transition") => Scenario::PowerTransition,
        _ => return Err("Use live, idle, battery, saver, or power-transition".into()),
    };
    engine.start(device, DEFAULT_STRENGTH, true, scenario)?;
    let watchdog = engine.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(20));
        watchdog.stop();
    });
    let status = engine.wait();
    let after = diagnose()?;
    println!("After: {after}");
    if status.recovery_pending
        || status.message != "Off. Original display settings restored."
        || before != after
    {
        return Err(format!("Benchmark/restoration failed: {}", status.message));
    }
    println!("PASS: bounded benchmark and original display settings restored");
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
