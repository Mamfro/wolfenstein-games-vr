//! Hotwiring by hand: in The New Order's hotwiring minigame each stick moves one of the player's
//! hands, holding a wire, until the two wires' tips touch; here each controller moves its own
//! hand instead.
//!
//! Each hand only blends between four poses (left, right, up, down), so it moves in a small flat
//! range, and the game reads each stick as a place in it, not a speed (`engine::Hotwire`). The
//! controllers' places are taken as their rest a second into the minigame ([`SETTLE_MS`]): from
//! there, moving one sideways (along the head's level right at that moment) or up and down moves
//! its hand the same way, [`THROW`] reaching the end of the range ([`sticks`]). The values go
//! straight into the four the minigame reads from the player's command, past the game's stick
//! dead zone and curve, and are given back after its update. The difficulty's tremble, the
//! touch, the stages and the button that gives up stay the game's.
//!
//! While the hands move the wires, nothing of ours moves them: the arms stay as animated, a held
//! item stays on its hand and a swing is no melee ([`active`]).

use crate::Wolf;
use crate::cooking::head;
use crate::providers::camera_taken;
use crate::providers::engine::{HOTWIRE_SPRINGS, HOTWIRE_STICKS, Hotwire, HotwireUpdateFn};
use monaka_body::hands::{self, At, In, Point};
use monaka_contract::protocol::{HeadPose, LEFT_HAND, RIGHT_HAND};
use monaka_core::math::{Vec3, forward_axis};
use monaka_core::sync::Locked;
use monaka_hook::{InFlight, Original, mem};
use monaka_producer::game::{Component, Lack, OrMissing, Start, place};
use monaka_producer::log;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::*};
use std::sync::{Mutex, OnceLock};

/// The hotwiring minigame's hands moved by the controllers.
pub const HOTWIRE: Component<Wolf> = Component {
    name: "hotwire",
    games: &["wolf_tno"],
    install,
    summary: Some(summary),
    ..Component::NONE
};

pub(crate) const HOTWIRE_AT: usize = place::<Wolf>(&HOTWIRE);

/// How long into the minigame the controllers' rest is taken (ms): time to bring the hands up.
pub const SETTLE_MS: u64 = 1000;
/// How far a controller moves from its rest to reach the end of its hand's range (metres).
pub const THROW: f32 = 0.15;
/// A gap between the minigame's updates this long (ms) starts a new one.
const GAP_MS: u64 = 500;
/// The hands count as moving the wires this long (ms) after the minigame's last update.
const ACTIVE_MS: u64 = 250;

pub static UPDATE_ORIGINAL: Original<HotwireUpdateFn> = Original::new();
static FACTS: OnceLock<Hotwire> = OnceLock::new();

/// One minigame as the game thread holds it.
struct Wiring {
    /// The minigame's event, when it began and when it last updated (tick, ms).
    event: usize,
    began: u64,
    last: u64,
    /// The controllers' rest (left, right) and the head's level right then, once taken.
    rest: Option<([Vec3; 2], Vec3)>,
}

static WIRING: Mutex<Wiring> = Mutex::new(Wiring { event: 0, began: 0, last: 0, rest: None });
/// The minigame's last update (tick, ms; 0 none yet).
static LAST: AtomicU64 = AtomicU64::new(0);
static GAMES: AtomicU64 = AtomicU64::new(0);
static DRIVEN: AtomicU64 = AtomicU64::new(0);
static STRANGERS: AtomicU64 = AtomicU64::new(0);
static LINES: AtomicU32 = AtomicU32::new(0);

fn install(start: &mut Start<'_, Wolf>) -> Result<(), Lack> {
    let crate::Facts { exe, build } = *start.build;
    let facts = build.hotwire.as_ref().or_missing(&format!("{} has no hotwiring mapped", build.game))?;
    let _ = FACTS.set(*facts);
    start.hooks.site(&UPDATE_ORIGINAL, &exe, &facts.update, update)?;
    Ok(())
}

fn summary() -> String {
    format!(
        "hotwire: {} minigames, {} of their frames moved by the hands, {} frames for a player not the one in view",
        GAMES.load(Relaxed),
        DRIVEN.load(Relaxed),
        STRANGERS.load(Relaxed)
    )
}

/// Whether the controllers move the minigame's hands now: our arms, held items and melee stand
/// down meanwhile.
pub fn active() -> bool {
    let last = LAST.load(Acquire);
    last != 0 && monaka_channel::tick().saturating_sub(last) <= ACTIVE_MS && crate::on(HOTWIRE_AT)
}

/// The head's level right (tracking space, unit length). None with the head looking straight up
/// or down.
pub fn level_right(head: &HeadPose) -> Option<Vec3> {
    let forward = forward_axis(head.orientation);
    let level = forward[0].hypot(forward[2]);
    (level >= 1e-3).then(|| [-forward[2] / level, 0.0, forward[0] / level])
}

/// The four values the minigame reads ([`HOTWIRE_STICKS`]: left sideways, left up, right
/// sideways, right up; right and up positive) for the controllers at `hands` (left, right;
/// tracking space, metres) against their `rest`, sideways along `right`: [`THROW`] either way is
/// the end of the range.
pub fn sticks(rest: [Vec3; 2], right: Vec3, hands: [Vec3; 2]) -> [i8; 4] {
    let byte = |metres: f32| ((metres / THROW).clamp(-1.0, 1.0) * 127.0).round() as i8;
    let mut out = [0; 4];
    for side in 0..2 {
        let moved: Vec3 = std::array::from_fn(|k| hands[side][k] - rest[side][k]);
        out[2 * side] = byte(moved[0] * right[0] + moved[1] * right[1] + moved[2] * right[2]);
        out[2 * side + 1] = byte(moved[1]);
    }
    out
}

/// Both controllers' positions in the tracking space, if both are tracked.
fn both_hands() -> Option<[Vec3; 2]> {
    let at = |side| hands::pose::<Wolf>(side, Point::Controller, At::Latest, In::Tracking).filter(|p| p.valid != 0).map(|p| p.position);
    Some([at(LEFT_HAND)?, at(RIGHT_HAND)?])
}

/// The values for this update of `event`'s minigame: centred until the rest is taken, None for
/// the game's own (untracked controllers).
fn wanted(event: usize) -> Option<[i8; 4]> {
    let now = monaka_channel::tick();
    let mut wiring = WIRING.locked();
    if wiring.event != event || now.saturating_sub(wiring.last) > GAP_MS {
        *wiring = Wiring { event, began: now, last: now, rest: None };
        GAMES.fetch_add(1, Relaxed);
        if LINES.fetch_add(1, Relaxed) < 40 {
            log!("hotwire: the minigame began: each controller moves its own hand in a second");
        }
    }
    wiring.last = now;
    LAST.store(now, Release);
    let hands = both_hands()?;
    let (rest, right) = match wiring.rest {
        Some(rest) => rest,
        None if now.saturating_sub(wiring.began) >= SETTLE_MS => {
            let right = level_right(&head::HEAD.current()?)?;
            wiring.rest = Some((hands, right));
            if LINES.fetch_add(1, Relaxed) < 40 {
                let apart = (0..3).map(|k| (hands[1][k] - hands[0][k]).powi(2)).sum::<f32>().sqrt();
                log!("hotwire: the controllers' rest taken ({:.0} cm apart): {:.0} cm from it moves a hand to the end of its range", apart * 100.0, THROW * 100.0);
            }
            (hands, right)
        }
        None => return Some([0; 4]),
    };
    DRIVEN.fetch_add(1, Relaxed);
    Some(sticks(rest, right, hands))
}

/// Writes the values the controllers give into `player`'s command, if it is the player in view;
/// the command and the values it held.
fn drive(event: usize, player: usize) -> Option<(usize, [u8; 4])> {
    if !crate::on(HOTWIRE_AT) || player == 0 {
        return None;
    }
    let facts = FACTS.get()?;
    let (presentable, in_view) = camera_taken::game_player()?;
    if in_view != player {
        if STRANGERS.fetch_add(1, Relaxed) == 0 {
            log!("hotwire: the minigame's player {player:#x} is not the one in view ({in_view:#x}): the sticks move the wires");
        }
        return None;
    }
    let values = wanted(event)?;
    let command = presentable + facts.command;
    let mut held = [0u8; 4];
    for (slot, &field) in HOTWIRE_STICKS.iter().enumerate() {
        held[slot] = mem::read::<u8>(command + field)?;
    }
    for (slot, &field) in HOTWIRE_STICKS.iter().enumerate() {
        mem::write(command + field, values[slot] as u8);
    }
    Some((command, held))
}

/// Logs where the minigame put the hands' springs for the first frames the controllers moved.
fn note_springs(event: usize) {
    let driven = DRIVEN.load(Relaxed);
    if !matches!(driven, 1 | 60 | 300) || LINES.fetch_add(1, Relaxed) >= 40 {
        return;
    }
    let spring = |at: usize| mem::read_finite::<2>(event + at).map(|v| format!("({:.2}, {:.2})", v[0], v[1])).unwrap_or_else(|| "unread".to_owned());
    log!("hotwire: frame {driven} by the hands: springs left {} right {}", spring(HOTWIRE_SPRINGS[0]), spring(HOTWIRE_SPRINGS[1]));
}

/// `idHotWireEvent::UpdateHotwiring`: the controllers' values in the player's command for the
/// update, the command's own given back after it.
unsafe extern "system" fn update(event: usize, player: usize) {
    let flight = InFlight::enter();
    let written = drive(event, player);
    // SAFETY: forwards the game's own call, the command's stick values ours meanwhile.
    unsafe { UPDATE_ORIGINAL.get(&flight)(event, player) };
    if let Some((command, held)) = written {
        for (slot, &field) in HOTWIRE_STICKS.iter().enumerate() {
            mem::write(command + field, held[slot]);
        }
        note_springs(event);
    }
}

#[cfg(test)]
mod tests;
