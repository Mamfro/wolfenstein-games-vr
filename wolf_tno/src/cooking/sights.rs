//! Cooking, waiting for a Monaka aim-down-sights feature, which would own the dominant eye's shift
//! and the sight lock for every game with sights, reading the held item provider (the held gun's
//! sight line) and whether the game is aiming (a situation or an action; here the eye site reads it
//! from the game camera's zoom against the resting field of view,
//! [`eye_site`](crate::providers::eye_site)).
//!
//! The eyes on the sights: the dominant eye sits where the game camera is, so it looks straight
//! down the weapon's sights ([`put_dominant_eye`]), and while aiming down sights the head's
//! position slides onto the game camera, so the sights line up whatever `head_position` says
//! ([`sighting`], [`head_position`]).
//!
//! No aiming down sights from the pad, though: in VR the gun is aimed where it points, and the
//! game's zoom only narrows the view. On foot with one gun the left trigger (the game's zoom) is
//! let go before the game reads it ([`no_zoom`]).

use crate::eyes::DominantEye;
use crate::providers::{camera_taken, held_item};
use monaka_contract::gamepad::Gamepad;
use monaka_core::options::Shared;
use monaka_contract::protocol::HeadPose;

/// In the pad the game reads: the left trigger let go while it would only zoom ([`zooms`]).
pub fn no_zoom(pad: &mut Gamepad) {
    if zooms(held_item::dual(), camera_taken::controlled() || camera_taken::mounted()) {
        pad.left_trigger = 0;
    }
}

/// Whether the left trigger is the game's zoom: on foot with one gun. With two guns it fires the
/// left one; on a mounted gun, in the mech or a car (another camera on the view) it is that
/// machine's (firing, reversing).
pub fn zooms(dual: bool, other_camera: bool) -> bool {
    !dual && !other_camera
}

/// Where the head's position puts the view (metres): the head model's, slid onto the game camera
/// as far as the view is `sighted` ([`sighting`]).
pub fn head_position(shared: &Shared, pose: &HeadPose, sighted: f32) -> [f32; 3] {
    let position = shared.head_position.position(pose);
    if sighted > 0.0 { position.map(|v| v * (1.0 - sighted)) } else { position }
}

/// Degrees of zoom below the resting field of view that count as fully aiming down sights, and
/// the margin below which it is still rest (The Old Blood's hip view is 79.999985 of 80).
const SIGHT_ZOOM: f32 = 5.0;
const SIGHT_MARGIN: f32 = 0.5;

/// How far into aiming down sights the game camera is, from its field of view and the resting
/// one: 0 at rest (or wider, sprinting), 1 once zoomed in by [`SIGHT_ZOOM`] degrees or more. The
/// engine zooms over a few frames, so the eyes slide onto the camera rather than jump.
pub fn sighting(fov: f32, rest: f32) -> f32 {
    if !(fov.is_finite() && rest.is_finite() && rest > 1.0) {
        return 0.0;
    }
    ((rest - fov - SIGHT_MARGIN) / (SIGHT_ZOOM - SIGHT_MARGIN)).clamp(0.0, 1.0)
}

/// Moves the view sideways so the dominant eye sits where the game camera is: the engine puts
/// the left eye `separation` units along the view's left axis and the right eye as far the other
/// way.
pub fn put_dominant_eye(view: &mut [f32; 12], dominant: DominantEye, separation: f32) {
    let side = match dominant {
        DominantEye::None => return,
        DominantEye::Right => 1.0,
        DominantEye::Left => -1.0,
    };
    for k in 0..3 {
        view[k] += side * separation * view[6 + k];
    }
}

#[cfg(test)]
mod tests;
