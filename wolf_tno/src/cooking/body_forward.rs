//! Cooking, waiting for a Monaka ready-made for the run's heading (part of the look): the left
//! stick halfway between the head's heading and the body's, or (`body_turn`) on a body that turns
//! with the player.
//!
//! With head aim the game camera carries the head's yaw (`monaka_body::look`'s share in the
//! command angles), and the game moves the player relative to its camera: the run goes where the
//! head looks. Fully on the body's heading (the right stick's turn) a glance aside felt like
//! moving sideways; fully on the head's, it steered the run. Before the game reads the pad, the
//! left stick is turned back by [`HEAD_SHARE_OUT`] of the head's share: the run goes halfway
//! between. The view, the aim and the shots stay the head's.
//!
//! With `body_turn` ([`BODY_TURN`]) the player turning in the room turns the body: a heading of
//! the mod's own follows the head's ([`Body`]), never more than [`DEAD_ZONE`] behind and settling
//! onto it over [`SETTLE_SECONDS`], so a glance aside barely steers but a real turn does, and the
//! left stick is turned onto that heading. The right stick still turns the body as before.
//!
//! With `turn=snap` (Monaka's [`monaka_contract::manifest::TURN`]) the right stick turns the body
//! [`StickTurn::SNAP`]'s degrees at a push instead of smoothly ([`take_snapped`]: into the command's
//! angles, `providers::look`).

use crate::Wolf;
use crate::providers::camera_taken;
use monaka_body::look;
use monaka_contract::gamepad::Gamepad;
use monaka_contract::manifest::{Kind, Opt, Shown, opt};
use monaka_core::pad::{StickTurn, TurnLatch};
use monaka_core::sync::Locked;
use std::f32::consts::{PI, TAU};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering::*};
use std::time::Instant;

/// How much of the head's turn is taken out of the run's heading (0: where the head looks, 1:
/// where the body faces).
pub const HEAD_SHARE_OUT: f32 = 0.5;

/// The body turning with the player (the left stick walking where they face, not halfway to where
/// the right stick last turned them).
pub const BODY_TURN: Opt =
    opt("body_turn", "Room-scale turning", "Turning in your room turns your character.", Kind::Switch { default: true }).on(Shown::Controls);

/// How far the head may turn from the body before it takes the body along (radians).
pub const DEAD_ZONE: f32 = 45.0 * PI / 180.0;
/// How long the body takes to settle onto where the head faces (seconds, to about a third).
pub const SETTLE_SECONDS: f32 = 1.0;
/// A head turn this far between two pad reads is a recentre, not a turn (radians): the body jumps
/// with it.
const RECENTRE: f32 = 60.0 * PI / 180.0;
/// The longest step the settling takes at once (seconds): a pause is not a long settle.
const LONGEST_STEP: f32 = 0.1;

static TURNS: AtomicBool = AtomicBool::new(false);

/// The right stick's snap turn (`turn=snap`), and its latch; none: the game's own smooth turn.
static SNAP: Mutex<Option<(StickTurn, TurnLatch)>> = Mutex::new(None);
/// Snapped degrees (right positive) not yet in a command ([`take_snapped`]).
static SNAPPED: Mutex<f32> = Mutex::new(0.0);

/// At the look's install: whether the body turns with the player this run, and how the right
/// stick turns.
pub fn configure(options: &monaka_core::options::Options, manifest: &monaka_contract::manifest::Manifest) {
    TURNS.store(options.switch(BODY_TURN.id, true), Relaxed);
    let turn = StickTurn::from_options(options, manifest, StickTurn::smooth(0.0), StickTurn::SNAP);
    *SNAP.locked() = matches!(turn, StickTurn::Snap { .. }).then(|| (turn, TurnLatch::new()));
}

/// The snap turn on the pad: while the player's own angles turn the view (not a mounted gun, the
/// mech, the weapon wheel or another camera), the right stick's sideways push turns once per push,
/// into the next command ([`take_snapped`]), and the game sees no sideways push.
fn snap(pad: &mut Gamepad) {
    let mut snap = SNAP.locked();
    let Some((turn, latch)) = snap.as_mut() else { return };
    // The look holds the head's share while the player's own angles turn the view (the command
    // delivery leaves `look::live` unset).
    if look::baked::<Wolf>(None).is_none() || camera_taken::holds() || camera_taken::controlled() {
        return;
    }
    let degrees = turn.turn(f32::from(pad.thumb_rx) / 32767.0, 0.0, latch);
    if degrees != 0.0 {
        *SNAPPED.locked() += degrees;
    }
    pad.thumb_rx = 0;
}

/// The snapped degrees (right positive) for the command being built, taken.
pub fn take_snapped() -> f32 {
    std::mem::take(&mut *SNAPPED.locked())
}

/// The pad the game reads, last (`monaka_pad::rewrite`): the knife thrown by the trigger
/// (`cooking::knife_throw`), the right trigger let go while swings attack (`providers::melee`),
/// the right stick's snap turn, the left stick onto the body's heading, the left trigger let go
/// while it would only zoom (`cooking::sights`), and Y held opening the journal
/// (`cooking::journal`).
pub fn pad(pad: &mut Gamepad) {
    crate::cooking::sights::no_zoom(pad);
    crate::cooking::journal::pad(pad);
    // The knife's throw reads the trigger before the swings let go of it.
    crate::cooking::knife_throw::pad(pad);
    crate::providers::melee::let_go_of_trigger(pad);
    snap(pad);
    let share = look::baked::<Wolf>(None);
    let out = match (TURNS.load(Relaxed), share) {
        (true, Some(share)) => Some(BODY.locked().follow(share.yaw, Instant::now())),
        (true, None) => {
            // Not looking by the head (a cutscene, another camera): the body starts again from
            // where the head faces.
            BODY.locked().reset();
            None
        }
        (false, share) => share.map(|share| share.yaw * HEAD_SHARE_OUT),
    };
    if let Some(out) = out {
        (pad.thumb_lx, pad.thumb_ly) = turned_stick(pad.thumb_lx, pad.thumb_ly, out);
    }
}

/// The body's heading of its own, in tracking space (radians, positive left, as the head's share).
#[derive(Clone, Copy, Debug, Default)]
pub struct Body {
    /// Its heading and the head's at the last read, and when.
    yaw: Option<(f32, f32, Instant)>,
}

static BODY: Mutex<Body> = Mutex::new(Body { yaw: None });

impl Body {
    /// The head faces `head` at `now`: the body follows, and what the head is turned from it (what
    /// the left stick is turned back by).
    pub fn follow(&mut self, head: f32, now: Instant) -> f32 {
        let body = match self.yaw {
            None => head,
            Some((body, last_head, at)) => {
                let turned = wrapped(head - last_head);
                // A recentre: the body goes round with the head.
                let body = if turned.abs() > RECENTRE { body + turned } else { body };
                let step = now.duration_since(at).as_secs_f32().min(LONGEST_STEP);
                followed(body, head, step)
            }
        };
        self.yaw = Some((body, head, now));
        wrapped(head - body)
    }

    pub fn reset(&mut self) {
        self.yaw = None;
    }
}

/// The body at `body` after the head facing `head` for `seconds`: settled toward it, and never
/// more than [`DEAD_ZONE`] away.
pub fn followed(body: f32, head: f32, seconds: f32) -> f32 {
    let off = wrapped(head - body).clamp(-DEAD_ZONE, DEAD_ZONE) * (-seconds / SETTLE_SECONDS).exp();
    wrapped(head - off)
}

/// `angle` in (-pi, pi].
fn wrapped(angle: f32) -> f32 {
    let a = (angle + PI).rem_euclid(TAU) - PI;
    if a <= -PI { a + TAU } else { a }
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
