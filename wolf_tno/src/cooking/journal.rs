//! Cooking, waiting for a Monaka remap of a held button (a long press reaching the game as another
//! button): the journal on Y held.
//!
//! The journal is on Back, which Quest's controllers lack. Y held for
//! [`HOLD`](monaka_core::hold::HOLD) presses Back instead; a shorter press reaches the game as a
//! tap of Y (the previous weapon) when let go ([`LongPress`]).

use monaka_contract::gamepad::{Gamepad, button};
use monaka_core::hold::{LongPress, TAP};
use monaka_core::sync::Locked;
use std::sync::Mutex;
use std::time::Instant;

struct Held {
    y: LongPress,
    /// Back shown to the game until then, once Y has been held.
    back_until: Option<Instant>,
}

static HELD: Mutex<Held> = Mutex::new(Held { y: LongPress::new(button::Y), back_until: None });

/// In the pad the game reads: Y held opens the journal, Y tapped is the game's Y.
pub fn pad(pad: &mut Gamepad) {
    step(&mut HELD.locked(), pad, Instant::now());
}

fn step(held: &mut Held, pad: &mut Gamepad, now: Instant) {
    if held.y.step(pad, now) {
        held.back_until = Some(now + TAP);
    }
    match held.back_until {
        Some(until) if now < until => pad.buttons |= button::BACK,
        Some(_) => held.back_until = None,
        None => {}
    }
}

#[cfg(test)]
mod tests;
