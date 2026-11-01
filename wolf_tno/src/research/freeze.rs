//! Probe (`probe_freeze_at=<seconds>`): the world frozen (`g_stopTime`, as the debug menu's
//! "Freeze the world") that long after the first view build, to the end of the run: a PC check can
//! look around a moment it cannot pause at by hand.

use std::sync::OnceLock;
use std::time::Instant;

static FIRST_BUILD: OnceLock<Instant> = OnceLock::new();

/// Each view build: whether the world should be frozen now.
pub fn due() -> bool {
    let at = super::options().freeze_at;
    if at <= 0.0 {
        return false;
    }
    FIRST_BUILD.get_or_init(Instant::now).elapsed().as_secs_f32() >= at
}
