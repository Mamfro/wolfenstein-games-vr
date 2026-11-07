//! The run's heading, waiting for a Monaka ready-made (part of the look): the left stick on the
//! head's heading, the body's, or halfway between ([`MOVE_HEADING`]), the body's turning with the
//! player ([`BODY_TURN`]), and the right stick's snap turn.
//!
//! With head aim the game camera carries the head's yaw (`monaka_body::look`'s share in the
//! command angles), and the game moves the player relative to its camera: the run goes where the
//! head looks. Fully on the body's heading (the right stick's turn) a glance aside felt like
//! moving sideways; fully on the head's, the head steers the run, which is what physical turning
//! needs (as KF2VR's head-relative movement, its default). Before the game reads the pad, the
//! left stick is turned back by the chosen share of the head's turn ([`left_stick`]). The view,
//! the aim and the shots stay the head's.
//!
//! Off the head's heading, with `body_turn` ([`BODY_TURN`]) the player turning in the room turns
//! the body: a heading of the mod's own follows the head's ([`Body`]), never more than
//! [`DEAD_ZONE`] behind and settling onto it over [`SETTLE_SECONDS`], so a glance aside barely
//! steers but a real turn does, and the left stick is turned onto that heading. The right stick
//! still turns the body as before.
//!
//! With `turn=snap` (Monaka's [`monaka_contract::manifest::TURN`]) the right stick turns the body
//! [`StickTurn::SNAP`]'s degrees at a push instead of smoothly ([`snap`]; [`take_snapped`] into the
//! command's angles).
//!
//! Each game decides when these apply: its pad rewrite (`monaka_pad::rewrite`) calls [`snap`] while
//! the player's own angles turn the view and [`left_stick`] outside its menus.

use monaka_contract::gamepad::Gamepad;
use monaka_contract::manifest::{Kind, Manifest, Opt, Shown, choice, opt};
use monaka_core::options::Options;
use monaka_core::pad::{StickTurn, TurnLatch};
use monaka_core::sync::Locked;
use std::f32::consts::{PI, TAU};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::*};
use std::time::Instant;

/// Where the left stick moves the player (`move_heading`): where the body faces (the default, only
/// the right stick steers), halfway, or where the head looks (physical turns steer; experimental,
/// offered in developer mode only).
pub const MOVE_HEADING: Opt = opt(
    "move_heading",
    "Movement direction",
    "Body: only the right stick turns. Halfway: between where you look and where the body faces.",
    choice(&[("head", "Head, experimental"), ("halfway", "Halfway"), ("body", "Body")], "body"),
)
.on(Shown::Controls)
.turns_off("head", BODY_TURN.id, "Movement follows your head already; room-scale movement is its own setting.");

/// The body turning with the player (the left stick walking where they face, not halfway to where
/// the right stick last turned them).
pub const BODY_TURN: Opt =
    opt("body_turn", "Room-scale turning", "Turning in your room turns your character.", Kind::Switch { default: true }).on(Shown::Controls);

/// The shares of the head's turn taken out of the run's heading, by `move_heading` id.
const SHARES: [(&str, f32); 3] = [("head", 0.0), ("halfway", 0.5), ("body", 1.0)];

/// The run's share taken out, as `f32` bits (0.0 until read: the head's heading).
static SHARE_OUT: AtomicU32 = AtomicU32::new(0);

/// How much of the head's turn `options` take out of the run's heading (0: where the head looks,
/// 1: where the body faces).
pub fn share_out(options: &Options) -> f32 {
    options.choice("move_heading", &SHARES, 1.0)
}

/// The run's share taken out ([`configure`]).
pub fn head_share_out() -> f32 {
    f32::from_bits(SHARE_OUT.load(Relaxed))
}

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

/// The options this module reads, for a component's `settings`.
pub fn read(options: &Options, manifest: &Manifest) {
    share_out(options);
    options.switch(BODY_TURN.id, true);
    StickTurn::from_options(options, manifest, StickTurn::smooth(0.0), StickTurn::SNAP);
}

/// As the run starts: the run's heading, whether the body turns with the player, and how the right
/// stick turns.
pub fn configure(options: &Options, manifest: &Manifest) {
    SHARE_OUT.store(share_out(options).to_bits(), Relaxed);
    TURNS.store(options.switch(BODY_TURN.id, true), Relaxed);
    let turn = StickTurn::from_options(options, manifest, StickTurn::smooth(0.0), StickTurn::SNAP);
    *SNAP.locked() = matches!(turn, StickTurn::Snap { .. }).then(|| (turn, TurnLatch::new()));
}

/// The snap turn on the pad: the right stick's sideways push turns once per push, into the next
/// command ([`take_snapped`]), and the game sees no sideways push. For while the player's own
/// angles turn the view (not a mounted gun, the weapon wheel or another camera).
pub fn snap(pad: &mut Gamepad) {
    let mut snap = SNAP.locked();
    let Some((turn, latch)) = snap.as_mut() else { return };
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

/// The left stick onto the run's heading, `head` the yaw of the head's share in the game camera
/// (none: not looking by the head, as in a cutscene or under another camera, and the body starts
/// again from where the head faces).
pub fn left_stick(pad: &mut Gamepad, head: Option<f32>) {
    let share_out = head_share_out();
    let out = match (share_out == 0.0, TURNS.load(Relaxed), head) {
        // On the head's heading: as pushed (the head steers, physical turns with it).
        (true, ..) => None,
        (false, true, Some(head)) => Some(BODY.locked().follow(head, Instant::now())),
        (false, true, None) => {
            BODY.locked().reset();
            None
        }
        (false, false, head) => head.map(|head| head * share_out),
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
