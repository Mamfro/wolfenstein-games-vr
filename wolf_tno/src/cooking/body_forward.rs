//! Cooking, waiting for a Monaka ready-made for the run's heading (part of the look): the pad these
//! games read, and the run's heading on it (`eng_idt::heading`, shared with Wolfenstein II).

use crate::Wolf;
use crate::providers::camera_taken;
use eng_idt::heading;
use monaka_body::look;
use monaka_contract::gamepad::Gamepad;
use monaka_core::sync::Locked;
use monaka_producer::game::{Component, Lack, Start};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering::*};

pub use eng_idt::heading::{BODY_TURN, MOVE_HEADING, configure, share_out, take_snapped};

/// The run's heading as `move_heading` asks, read as the run starts.
pub const MOVE_HEADING_COMPONENT: Component<Wolf> = Component {
    name: "move heading",
    options: &[MOVE_HEADING],
    settings: |options, _| {
        share_out(options);
    },
    install,
    ..Component::NONE
};

fn install(start: &mut Start<'_, Wolf>) -> Result<(), Lack> {
    configure(start.options, start.manifest);
    Ok(())
}

/// The pad the game reads, last (`monaka_pad::rewrite`): the knife thrown by the trigger
/// (`cooking::knife_throw`), the right trigger let go while swings attack (`providers::melee`),
/// the right stick's snap turn, the left stick onto the body's heading, the left trigger let go
/// while it would only zoom (`cooking::sights`), and Y held opening the journal
/// (`cooking::journal`).
pub fn pad(pad: &mut Gamepad) {
    if crate::providers::eye_site::in_menu() {
        menu_stick(pad);
        return;
    }
    crate::cooking::sights::no_zoom(pad);
    crate::cooking::journal::pad(pad);
    // The knife's throw reads the trigger before the swings let go of it.
    crate::cooking::knife_throw::pad(pad);
    crate::providers::melee::let_go_of_trigger(pad);
    let share = look::baked::<Wolf>(None);
    // The look holds the head's share while the player's own angles turn the view (the command
    // delivery leaves `look::live` unset): not on a mounted gun, in the mech, on the weapon wheel
    // or under another camera.
    if share.is_some() && !camera_taken::holds() && !camera_taken::controlled() {
        heading::snap(pad);
    }
    heading::left_stick(pad, share.map(|share| share.yaw));
    // Last, on the game camera's own heading: room-scale following's push.
    crate::cooking::room_scale::pad(pad);
}

/// A stick past this share of its range is a menu step, held until it is back under
/// [`MENU_REST`] (a light push steps as on a pad; the stick's noise at the edge does not).
const MENU_STEP: f32 = 0.35;
const MENU_REST: f32 = 0.2;

/// In a menu: the left stick as a steady step along its main axis, full or centred. The VR stick's
/// own noise and the pad's dither change the state every few reads, and the menu takes a changing
/// stick as a push that has not settled, stepping seconds late.
fn menu_stick(pad: &mut Gamepad) {
    static LOGGED: AtomicU32 = AtomicU32::new(0);
    static LAST: Mutex<(i16, i16)> = Mutex::new((0, 0));
    let mut last = LAST.locked();
    let step = menu_step(pad.thumb_lx, pad.thumb_ly, *last != (0, 0));
    (pad.thumb_lx, pad.thumb_ly) = step;
    if *last != step {
        if LOGGED.fetch_add(1, Relaxed) < 200 {
            monaka_producer::log!("menu stick: {:?} -> {:?} handed to the game", *last, step);
        }
        *last = step;
    }
}

/// The step a left stick at `x`, `y` is in a menu: its main axis at full, past [`MENU_STEP`]
/// (past [`MENU_REST`] while a step is `held`).
pub fn menu_step(x: i16, y: i16, held: bool) -> (i16, i16) {
    let (fx, fy) = (f32::from(x) / 32767.0, f32::from(y) / 32767.0);
    if fx.hypot(fy) < if held { MENU_REST } else { MENU_STEP } {
        return (0, 0);
    }
    let full = |v: f32| if v < 0.0 { -32767 } else { 32767 };
    if fx.abs() > fy.abs() { (full(fx), 0) } else { (0, full(fy)) }
}

#[cfg(test)]
mod tests;
