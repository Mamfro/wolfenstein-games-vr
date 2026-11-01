//! Cooking, waiting for a Monaka ready-made for the run's heading (part of the look): the left
//! stick halfway between the head's heading and the body's.
//!
//! With head aim the game camera carries the head's yaw (`monaka_body::look`'s share in the
//! command angles), and the game moves the player relative to its camera: the run goes where the
//! head looks. Fully on the body's heading (the right stick's turn) a glance aside felt like
//! moving sideways; fully on the head's, it steered the run. Before the game reads the pad, the
//! left stick is turned back by [`HEAD_SHARE_OUT`] of the head's share: the run goes halfway
//! between. The view, the aim and the shots stay the head's.

use crate::Wolf;
use monaka_body::look;
use monaka_contract::gamepad::Gamepad;

/// How much of the head's turn is taken out of the run's heading (0: where the head looks, 1:
/// where the body faces).
pub const HEAD_SHARE_OUT: f32 = 0.5;

/// The pad the game reads, last (`monaka_pad::rewrite`): the knife thrown by the trigger
/// (`cooking::knife_throw`), the left stick halfway onto the body's heading, and the right trigger
/// let go while swings attack (`providers::melee`).
pub fn pad(pad: &mut Gamepad) {
    // The knife's throw reads the trigger before the swings let go of it.
    crate::cooking::knife_throw::pad(pad);
    crate::providers::melee::let_go_of_trigger(pad);
    if let Some(share) = look::baked::<Wolf>(None) {
        (pad.thumb_lx, pad.thumb_ly) = turned_stick(pad.thumb_lx, pad.thumb_ly, share.yaw * HEAD_SHARE_OUT);
    }
}

/// The stick `(x, y)` (x right, y forward) given on a heading `yaw` radians left of the one wanted,
/// turned onto it.
pub fn turned_stick(x: i16, y: i16, yaw: f32) -> (i16, i16) {
    let (s, c) = (-yaw).sin_cos();
    let (x, y) = (f32::from(x), f32::from(y));
    let axis = |v: f32| v.round().clamp(-32768.0, 32767.0) as i16;
    (axis(x * c - y * s), axis(x * s + y * c))
}

#[cfg(test)]
mod tests;
