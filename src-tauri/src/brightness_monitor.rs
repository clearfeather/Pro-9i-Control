//! Observe Windows brightness keys without intercepting or remapping them.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

static START: Once = Once::new();
static STOP: AtomicBool = AtomicBool::new(false);

fn visible(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .is_some_and(|window| window.is_visible().unwrap_or(false))
}

fn publish(app: &AppHandle, previous: &mut Option<u8>, level: u8) {
    if *previous != Some(level) && visible(app) {
        let _ = app.emit("brightness-changed", ());
    }
    *previous = Some(level);
}

fn poll_visible(app: &AppHandle, previous: &mut Option<u8>) {
    if visible(app) {
        if let Ok(level) = lenovo_hdr_boost::current_brightness() {
            publish(app, previous, level);
        }
    }
}

fn watch(app: &AppHandle) -> Result<(), String> {
    let events = lenovo_hdr_boost::BrightnessEvents::subscribe()?;
    let mut previous = None;
    while !STOP.load(Ordering::Relaxed) {
        match events.next(500)? {
            Some(level) => publish(app, &mut previous, level),
            // Also cover drivers which accept the subscription but omit events.
            None => poll_visible(app, &mut previous),
        }
    }
    Ok(())
}

pub fn start(app: AppHandle) {
    START.call_once(|| {
        std::thread::spawn(move || {
            while !STOP.load(Ordering::Relaxed) {
                let result = watch(&app);
                if STOP.load(Ordering::Relaxed) { break; }
                if let Err(error) = result { log::warn!("{error}; checking brightness while the panel is visible."); }
                let retry = Instant::now() + Duration::from_secs(30);
                let mut previous = None;
                while !STOP.load(Ordering::Relaxed) && Instant::now() < retry {
                    poll_visible(&app, &mut previous);
                    std::thread::sleep(Duration::from_millis(500));
                }
            }
        });
    });
}

pub fn stop() { STOP.store(true, Ordering::Relaxed); }
