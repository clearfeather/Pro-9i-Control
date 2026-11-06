//! Lenovo 83BV only. This crate has no Tauri or web UI dependency.
mod brightness;
mod desktop;
mod display;
mod power;
mod renderer;
mod session;

pub use brightness::current as current_brightness;
pub use brightness::set as set_panel_brightness;
pub use brightness::{set_user as set_user_panel_brightness, BrightnessEvents};
pub use display::{active_displays as display_configurations, set_refresh as set_display_refresh, uses_internal_brightness};
pub use session::{
    change_lease, diagnose, recovery_path, verify_boost_display, Controller, Scenario, Status,
};

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

pub const MIN_STRENGTH: f32 = 1.0;
pub const MAX_STRENGTH: f32 = 2.5;
pub const DEFAULT_STRENGTH: f32 = MAX_STRENGTH;

pub(crate) struct Signals {
    stop: AtomicBool,
    gain: AtomicU32,
}
impl Signals {
    fn new(gain: f32) -> Self {
        Self {
            stop: AtomicBool::new(false),
            gain: AtomicU32::new(gain.to_bits()),
        }
    }
    pub(crate) fn stopped(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }
    pub(crate) fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
    pub(crate) fn gain(&self) -> f32 {
        f32::from_bits(self.gain.load(Ordering::SeqCst))
    }
}

fn validate_gain(gain: f32) -> Result<(), String> {
    if !gain.is_finite() || !(MIN_STRENGTH..=MAX_STRENGTH).contains(&gain) {
        Err("HDR Boost strength must be between 1× and 2.5×".into())
    } else {
        Ok(())
    }
}
