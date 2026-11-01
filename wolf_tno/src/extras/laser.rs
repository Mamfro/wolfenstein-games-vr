//! The cutting laser aims where its gun points, with the guns on the controllers (installed with
//! them, [`held_item`]).
//!
//! The laser (the gun that cuts chains and locks) cuts only at the points it locks onto, and its draw locks onto a point only
//! within a small angle of the origin and direction it hands its target check
//! (`engine::Guns::laser_target`): the player's view. With the gun in a controller's hand that is
//! where the head looks, not where the gun points. The check is given the gun's own placement
//! instead (origin and forward axis, as the hand holding it placed it: `held_item::placed`), so it
//! locks onto what the gun points at. Its shots already go along its barrel (`providers::fire`).

use crate::providers::engine::{self, LaserTargetFn};
use crate::providers::held_item;
use monaka_hook::module::Module;
use monaka_hook::{Hooks, InFlight, Original};
use monaka_producer::game::Lack;
use monaka_producer::log;
use std::sync::atomic::{AtomicU64, Ordering::*};

pub static TARGET_ORIGINAL: Original<LaserTargetFn> = Original::new();
static FROM_BARREL: AtomicU64 = AtomicU64::new(0);
/// A placement older than this (ms) is not the gun's now.
const FRESH_MS: u64 = 200;

pub fn install(hooks: &mut Hooks, exe: &Module, guns: &engine::Guns) -> Result<(), Lack> {
    let Some(site) = &guns.laser_target else { return Ok(()) };
    hooks.site(&TARGET_ORIGINAL, exe, site, target)?;
    Ok(())
}

pub fn summary() -> String {
    format!("laser: {} cut point checks aimed along its barrel", FROM_BARREL.load(Relaxed))
}

/// The origin and forward axis of the hand's gun that is `weapon`, placed lately.
fn barrel_of(weapon: usize) -> Option<([f32; 3], [f32; 3])> {
    let now = monaka_channel::tick();
    held_item::placed().into_iter().flatten().find(|p| p.weapon == weapon && now.saturating_sub(p.at) < FRESH_MS).map(|p| (p.origin, [p.axis[0], p.axis[1], p.axis[2]]))
}

unsafe extern "system" fn target(laser: usize, b: usize, point: *const f32, origin: *const f32, direction: *const f32, cosine: *mut f32) -> u8 {
    let flight = InFlight::enter();
    let barrel = crate::on(crate::GUNS_AT).then(|| barrel_of(laser)).flatten();
    match barrel {
        Some((from, along)) => {
            if FROM_BARREL.fetch_add(1, Relaxed) == 0 {
                log!("laser: its cut points are aimed at along its barrel, not the view");
            }
            // SAFETY: forwards the game's own call with the gun's barrel in this frame's locals.
            unsafe { TARGET_ORIGINAL.get(&flight)(laser, b, point, from.as_ptr(), along.as_ptr(), cosine) }
        }
        // SAFETY: forwards the game's own call.
        None => unsafe { TARGET_ORIGINAL.get(&flight)(laser, b, point, origin, direction, cosine) },
    }
}
