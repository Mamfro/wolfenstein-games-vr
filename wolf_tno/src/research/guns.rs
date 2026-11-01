//! Probe aid (`probe_guns=1`): synthetic controllers in front of the head hold the guns, for PC
//! checks of the guns in the hands without a headset ([`crate::providers::held_item`] asks here
//! for each palm).

use monaka_core::math::multiply;
use monaka_contract::protocol::{HandPose, HeadPose, RIGHT_HAND};

/// A turn of `degrees` about +Y (positive left) then `pitch` degrees about the turned +X (positive up).
fn yaw_pitch(degrees: f32, pitch: f32) -> [f32; 4] {
    let (sy, cy) = (degrees.to_radians() * 0.5).sin_cos();
    let (sp, cp) = (pitch.to_radians() * 0.5).sin_cos();
    multiply([0.0, sy, 0.0, cy], [sp, 0.0, 0.0, cp])
}

/// A controller held up for PC checks, where the game's own animation never holds a gun: 30 cm
/// out to its side, 8 cm below the eyes, 45 cm ahead, pointing 15 degrees outward and 5 up, the
/// whole turning with the head's yaw.
pub fn synthetic_palm(side: usize, head: &HeadPose) -> HandPose {
    let (yaw, _) = monaka_core::camera::head_yaw_pitch(head.orientation);
    let out = if side == RIGHT_HAND { 1.0 } else { -1.0 };
    let body = [0.0, (yaw * 0.5).sin(), 0.0, (yaw * 0.5).cos()];
    let offset = [0.30 * out, -0.08, -0.45];
    let r = monaka_core::camera::rotation(body);
    let turned = [0, 1, 2].map(|i| r[3 * i] * offset[0] + r[3 * i + 1] * offset[1] + r[3 * i + 2] * offset[2]);
    HandPose {
        orientation: multiply(body, yaw_pitch(-15.0 * out, 5.0)),
        position: [head.position[0] + turned[0], head.position[1] + turned[1], head.position[2] + turned[2]],
        valid: 1,
        received_tick: 0,
    }
}

#[cfg(test)]
mod tests;
