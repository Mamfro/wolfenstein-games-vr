//! Cooking, waiting for a Monaka ready-made for picking things up by walking to them (on
//! [`providers::pickups`](crate::providers::pickups)): ammo taken as the player comes near it.
//!
//! In VR, pointing the view at every box of ammo and pressing the use button is a chore. Each
//! think, an item on the ground that holds only ammo or guns (what the use prompt offers as ammo:
//! a gun an enemy dropped gives its ammo) is taken when it is within [`REACH`] of the player's
//! bounds, the player could use it now, and some of it fits, the game's own checks all three: by
//! the use button's own use, so it goes as if the button took it. What does not fit is left alone,
//! no use tried, as the button would leave it (an item's use can run its own events). Everything
//! else (a heavy gun carried in both hands, health, armour, a note) stays the button's; the summary
//! counts what else came within reach, by what it holds.

use crate::Wolf;
use crate::providers::camera_taken;
use crate::providers::pickups::{self, Holds};
use monaka_contract::manifest::{Kind, Opt, Shown, opt};
use monaka_producer::game::{Component, Lack, OrMissing, Start, Switch, place};
use monaka_producer::log;
use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering::*};

pub const AUTO_PICKUP_OPTION: Opt = opt("auto_pickup", "Pick up ammo by walking to it", "Off: only the use button picks ammo up.", Kind::Switch { default: true }).on(Shown::Game);

/// Ammo is taken by walking to it (`auto_pickup`, on by default).
pub const AUTO_PICKUP: Component<Wolf> = Component {
    name: "auto_pickup",
    options: &[AUTO_PICKUP_OPTION],
    switch: Some(SWITCH),
    install,
    summary: Some(summary),
    ..Component::NONE
};

/// Ammo left to the use button (`auto_pickup=0`).
pub const SWITCH: Switch = Switch::new("auto_pickup", true, "only the use button picks ammo up");

pub(crate) const AUTO_PICKUP_AT: usize = place::<Wolf>(&AUTO_PICKUP);

/// How far past the player's bounds an item is taken (units, about an arm's length).
pub const REACH: f32 = 40.0;
/// How many thinks an item whose use was refused is left alone before it is tried again.
pub const RETRY_THINKS: u64 = 60;
/// The most refused items remembered, and the most items whose type the summary keeps.
const REMEMBERED: usize = 16;
const SEEN: usize = 512;

static THINKS: AtomicU64 = AtomicU64::new(0);
static TAKEN: AtomicU64 = AtomicU64::new(0);
static REFUSED: AtomicU64 = AtomicU64::new(0);
/// The items whose use was refused, and at which think ([`Refused`]).
static TRIED: Mutex<Refused> = Mutex::new(Refused::new());
/// The other items that came within reach, each once, and how many of each type.
static OTHERS: Mutex<Others> = Mutex::new(Others { seen: None, kinds: Vec::new() });

struct Others {
    seen: Option<HashSet<usize>>,
    kinds: Vec<(String, u32)>,
}

fn install(start: &mut Start<'_, Wolf>) -> Result<(), Lack> {
    let crate::Facts { exe, build } = *start.build;
    let facts = build.pickups.as_ref().or_missing("the player's usables are not mapped for this build")?;
    pickups::install(start.hooks, &exe, build, facts)?;
    log!("auto pickup: ammo within {REACH} units is taken (auto_pickup=0 leaves it to the use button)");
    Ok(())
}

/// Whether `item` (absolute bounds: min, then max) is within `reach` of the box `player`.
pub fn in_reach(player: &[f32; 6], item: &[f32; 6], reach: f32) -> bool {
    (0..3).all(|axis| item[axis] <= player[axis + 3] + reach && item[axis + 3] >= player[axis] - reach)
}

/// The items whose use was refused lately: each left alone for [`RETRY_THINKS`].
pub struct Refused {
    items: [(usize, u64); REMEMBERED],
    next: usize,
}

impl Refused {
    pub const fn new() -> Self {
        Self { items: [(0, 0); REMEMBERED], next: 0 }
    }

    /// Whether `entity` was refused within [`RETRY_THINKS`] of `think`.
    pub fn waits(&self, entity: usize, think: u64) -> bool {
        self.items.iter().any(|&(e, at)| e == entity && think.saturating_sub(at) < RETRY_THINKS)
    }

    pub fn note(&mut self, entity: usize, think: u64) {
        self.items[self.next] = (entity, think);
        self.next = (self.next + 1) % REMEMBERED;
    }
}

/// After the player's think (`player`, an `idPlayer`, on the game thread): the ammo in reach
/// taken.
pub fn think(player: usize) {
    let think = THINKS.fetch_add(1, Relaxed) + 1;
    if !crate::on(AUTO_PICKUP_AT) || camera_taken::controlled() {
        return;
    }
    let Some(own) = pickups::bounds(player) else { return };
    pickups::items(player, &own, REACH, |item| {
        if !in_reach(&own, &item.bounds, REACH) {
            return;
        }
        match item.holds {
            Holds::Ammo { fits: true } => take(item.entity, player, think),
            Holds::Ammo { fits: false } => {}
            Holds::Other => note_other(item.entity),
        }
    });
}

fn take(entity: usize, player: usize, think: u64) {
    let Ok(mut tried) = TRIED.try_lock() else { return };
    if tried.waits(entity, think) {
        return;
    }
    if pickups::take(entity, player) {
        let n = TAKEN.fetch_add(1, Relaxed) + 1;
        if log::seldom(n, 10) {
            log!("auto pickup: ammo taken ({n} so far)");
        }
    } else {
        tried.note(entity, think);
        let n = REFUSED.fetch_add(1, Relaxed) + 1;
        if log::seldom(n, 10) {
            log!("auto pickup: the game refused an ammo item's use ({n} so far): left alone {RETRY_THINKS} thinks");
        }
    }
}

/// Counts an item in reach that is not only ammo by its type, once per item.
fn note_other(entity: usize) {
    let Ok(mut others) = OTHERS.try_lock() else { return };
    let Others { seen, kinds } = &mut *others;
    let seen = seen.get_or_insert_with(HashSet::new);
    if seen.len() >= SEEN || !seen.insert(entity) {
        return;
    }
    let name = pickups::described(entity);
    match kinds.iter_mut().find(|(k, _)| *k == name) {
        Some((_, n)) => *n += 1,
        None => kinds.push((name, 1)),
    }
}

fn summary() -> String {
    let others = OTHERS.lock().map(|o| o.kinds.iter().map(|(k, n)| format!("{n} {k}")).collect::<Vec<_>>().join(", ")).unwrap_or_default();
    let (read, seen) = pickups::counts();
    format!(
        "auto pickup: {read} gathers read ({seen} entities), {} ammo items taken, {} uses refused; other items in reach left to the use button: {}",
        TAKEN.load(Relaxed),
        REFUSED.load(Relaxed),
        if others.is_empty() { "none" } else { &others }
    )
}

#[cfg(test)]
mod tests;
