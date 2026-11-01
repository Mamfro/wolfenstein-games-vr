//! Provider, to be: the pad's connection mark (where the game marks pad 0 connected, written as the
//! game writes it), once Monaka's gamepad ready-made (`monaka_pad::feature::gamepad`) asks for
//! one; today it keeps the mark set itself ([`PAD_CONNECTED`]).
//!
//! The VR controllers as the game's pad 0. `monaka_pad` answers XInput for pad 0, but the game's
//! joystick thread reads XInput only for pads it marks connected (`engine::Build::pad_connected`),
//! and its search for new pads can stay blocked for over half an hour. While the viewer keeps the
//! VR pad live, pad 0 is marked connected each frame, as the game marks a pad it finds.

use crate::Wolf;
use monaka_hook::mem;
use monaka_producer::game::{Component, Need};
use monaka_producer::log;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering::*};

/// The game polls only pads it has marked connected, and its search for new pads can stall: the
/// mark is kept set while the controllers are its pad.
pub const PAD_CONNECTED: Component<Wolf> = Component {
    name: "pad connected",
    needs: &[Need::On(&crate::GAMEPAD)],
    install: |start| {
        if let Some(flag) = start.build.build.pad_connected {
            install(start.build.exe.at(flag));
        }
        Ok(())
    },
    quiet: || install(0),
    summary: Some(summary),
    ..Component::NONE
};

/// The connected byte's address (0: not kept).
static FLAG: AtomicUsize = AtomicUsize::new(0);
static MARKED: AtomicU64 = AtomicU64::new(0);

pub fn install(address: usize) {
    FLAG.store(address, Release);
}

pub fn summary() -> String {
    format!("pad 0 marked connected {} times", MARKED.load(Relaxed))
}

/// Once per frame, on the game thread.
pub fn frame() {
    let flag = FLAG.load(Acquire);
    if flag == 0 || !monaka_pad::live() || mem::read::<u8>(flag) != Some(0) {
        return;
    }
    // Connected, and was connected (what the game writes when it finds a pad).
    if mem::write(flag, [1u8, 1u8]) && MARKED.fetch_add(1, Relaxed) < 3 {
        log!("pad 0 was not marked connected; marked it (the VR pad is live)");
    }
}
