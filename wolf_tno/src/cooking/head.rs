//! Cooking, waiting for the OpenGL HUD-layer ready-made that lifts [`hud`](super::hud): what the
//! HUD's placement reads of the head, both the same-frame loop's (`monaka_stereo::same_frame`),
//! which owns the head source and the frames' tickets.

use monaka_channel::pose::HeadSource;
use monaka_contract::protocol::HeadPose;
use monaka_stereo::same_frame::PAIRS;

/// The loop's head source.
pub static HEAD: &HeadSource = PAIRS.head();

/// The pose of the frame the render thread is drawing (the oldest built and not shown).
pub fn rendering() -> Option<HeadPose> {
    PAIRS.rendering()
}
