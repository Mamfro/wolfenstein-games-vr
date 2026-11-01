//! Provider part, to be: whether the drawn view follows the real head (a check of the eye site's,
//! once Monaka has one), in every run's summary.
//!
//! A view drawn from the head, with the head's pose taken back out, says where the headset's
//! tracking space sits in the world (`camera::tracking_space`). While only the head moves it holds
//! still; the stick's turn and walking move it, smoothly. A frame drawn from another pose than the
//! one it goes out with (the head's share baked into the camera a command off, a pose taken at
//! another moment) puts that frame's space off its neighbours': a jump there and back. Each player
//! view build's space is kept with the two before it, and the middle one is measured against the
//! line between its neighbours (by their times): the jump is what the eyes would see the world
//! twitch by. The summary gives the jumps over [`JUMP`], the largest, and the first few with the
//! head's tilt, the frame times around them and the commands built for that frame.

use monaka_core::camera::{self, Mat34};
use monaka_core::math::{Quat, Vec3, length, sub};
use monaka_core::sync::Locked;
use monaka_producer::log;
use std::sync::Mutex;
use std::time::Instant;

/// A frame's space this far off the line between its neighbours (degrees, or centimetres) is a
/// jump.
pub const JUMP: (f32, f32) = (0.25, 0.5);
/// Views further apart than this start the check over (a pause, a load, a cut).
const GAP_SECONDS: f32 = 0.25;
const LINES: u32 = 20;

/// One view's: when, the space (canonical, metres), the head's tilt (degrees), and the commands
/// built since the view before.
#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub at: f32,
    pub space: Mat34,
    /// The game camera alone (canonical, metres): which part of a jump is the game's own.
    pub camera: Mat34,
    pub tilt: f32,
    pub commands: u64,
}

struct State {
    start: Option<Instant>,
    recent: Vec<Sample>,
    checked: u64,
    jumps: u64,
    largest: (f32, f32),
    tilted_jumps: u64,
    tilted_checked: u64,
    odd_commands: u64,
    lines: u32,
    turn_lines: u32,
}

static STATE: Mutex<State> =
    Mutex::new(State { start: None, recent: Vec::new(), checked: 0, jumps: 0, largest: (0.0, 0.0), tilted_jumps: 0, tilted_checked: 0, odd_commands: 0, lines: 0, turn_lines: 0 });
/// A head tilted this far (degrees) counts as tilted, for the tally.
const TILTED: f32 = 10.0;

/// The head's tilt (degrees, right ear down positive): how far its right axis leaves level.
pub fn tilt(orientation: Quat) -> f32 {
    let right = monaka_core::math::rotate(orientation, [1.0, 0.0, 0.0]);
    -right[1].clamp(-1.0, 1.0).asin().to_degrees()
}

/// `w` of the way from rigid `a` to rigid `b` (rotation by the shorter way, normalised).
fn between(a: &Mat34, b: &Mat34, w: f32) -> Mat34 {
    let (qa, mut qb) = (camera::quaternion(a), camera::quaternion(b));
    if (0..4).map(|i| qa[i] * qb[i]).sum::<f32>() < 0.0 {
        qb = qb.map(|v| -v);
    }
    let q: [f32; 4] = std::array::from_fn(|i| qa[i] + (qb[i] - qa[i]) * w);
    let n = (0..4).map(|i| q[i] * q[i]).sum::<f32>().sqrt().max(1e-9);
    let r = camera::rotation(q.map(|v| v / n));
    let (pa, pb) = (camera::position(a), camera::position(b));
    let t: Vec3 = std::array::from_fn(|i| pa[i] + (pb[i] - pa[i]) * w);
    [r[0], r[1], r[2], t[0], r[3], r[4], r[5], t[1], r[6], r[7], r[8], t[2]]
}

/// How far rigid `a` is from `b`: degrees and centimetres.
pub fn apart(a: &Mat34, b: &Mat34) -> (f32, f32) {
    let r = camera::compose(&camera::rigid_inverse(a), b);
    let degrees = ((r[0] + r[5] + r[10] - 1.0) * 0.5).clamp(-1.0, 1.0).acos().to_degrees();
    (degrees, length(sub(camera::position(a), camera::position(b))) * 100.0)
}

/// The middle camera's speed (metres a second, from its neighbours) and how far it is off the line
/// between them upward (centimetres; canonical y is up).
pub fn camera_motion(before: &Sample, middle: &Sample, after: &Sample) -> (f32, f32) {
    let span = (after.at - before.at).max(1e-6);
    let speed = length(sub(camera::position(&after.camera), camera::position(&before.camera))) / span;
    let w = ((middle.at - before.at) / span).clamp(0.0, 1.0);
    let line = between(&before.camera, &after.camera, w);
    (speed, (camera::position(&middle.camera)[1] - camera::position(&line)[1]) * 100.0)
}

/// How far the middle of three samples is off the line between the other two, by their times.
pub fn jump(before: &Sample, middle: &Sample, after: &Sample) -> (f32, f32) {
    let span = after.at - before.at;
    let w = if span > 1e-6 { ((middle.at - before.at) / span).clamp(0.0, 1.0) } else { 0.5 };
    apart(&between(&before.space, &after.space, w), &middle.space)
}

/// Each player view build drawn from the head (`space`: the tracking space the drawn view and its
/// pose give; `tilt`: the head's; `commands`: built so far). `None` (another camera has the view, a
/// flat frame) starts the check over.
pub fn note(space: Option<(Mat34, Mat34)>, tilt: f32, commands: u64) {
    let mut s = STATE.locked();
    let now = Instant::now();
    let at = s.start.get_or_insert(now).elapsed().as_secs_f32();
    let Some((space, camera)) = space else {
        s.recent.clear();
        return;
    };
    let since = s.recent.last().map_or(0, |last| commands.saturating_sub(last.commands));
    if s.recent.last().is_some_and(|last| at - last.at > GAP_SECONDS) {
        s.recent.clear();
    }
    s.recent.push(Sample { at, space, camera, tilt, commands });
    if s.recent.len() > 3 {
        s.recent.remove(0);
    }
    if since != 1 && s.recent.len() > 1 {
        s.odd_commands += 1;
    }
    let [before, middle, after] = match s.recent.as_slice() {
        [a, b, c] => [*a, *b, *c],
        _ => return,
    };
    let (degrees, cm) = jump(&before, &middle, &after);
    s.checked += 1;
    let tilted = middle.tilt.abs() >= TILTED;
    s.tilted_checked += u64::from(tilted);
    s.largest = (s.largest.0.max(degrees), s.largest.1.max(cm));
    if degrees > JUMP.0 || cm > JUMP.1 {
        s.jumps += 1;
        s.tilted_jumps += u64::from(tilted);
        // Turns and moves each get their own lines.
        let turn = degrees > JUMP.0;
        let logged = if turn { &mut s.turn_lines } else { &mut s.lines };
        if *logged < LINES {
            *logged += 1;
            let commands = middle.commands.saturating_sub(before.commands);
            let alone = |s: &Sample| Sample { space: s.camera, ..*s };
            let (camera_degrees, camera_cm) = jump(&alone(&before), &alone(&middle), &alone(&after));
            let (speed, up) = camera_motion(&before, &middle, &after);
            log!(
                "head conformance: a frame {degrees:.2} degrees / {cm:.2} cm off its neighbours, the game camera alone {camera_degrees:.2} degrees / {camera_cm:.2} cm ({up:+.2} cm of it up; moving {speed:.2} m/s) (head tilted {:.0} degrees; frames {:.1} ms and {:.1} ms apart; {commands} commands for it)",
                middle.tilt,
                (middle.at - before.at) * 1000.0,
                (after.at - middle.at) * 1000.0
            );
        }
    }
}

pub fn summary() -> String {
    let s = STATE.locked();
    let share = |n: u64, of: u64| if of == 0 { 0.0 } else { n as f32 * 100.0 / of as f32 };
    format!(
        "head conformance: {} frames checked, {} jumped (over {} degrees or {} cm; {:.2}%), the largest {:.2} degrees / {:.2} cm; with the head tilted {} of {} jumped ({:.2}%); {} view builds with other than one command",
        s.checked,
        s.jumps,
        JUMP.0,
        JUMP.1,
        share(s.jumps, s.checked),
        s.largest.0,
        s.largest.1,
        s.tilted_jumps,
        s.tilted_checked,
        share(s.tilted_jumps, s.tilted_checked),
        s.odd_commands
    )
}

#[cfg(test)]
mod tests;
