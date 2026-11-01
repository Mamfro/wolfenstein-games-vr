//! Provider, to be: the items around the player it could pick up, and their taking, for a Monaka
//! ready-made that picks things up by walking to them (today
//! [`auto_pickup`](crate::cooking::auto_pickup)).
//!
//! After each of the player's thinks, the entities around it are read from the gather asked for at
//! an earlier think, and the next gather asked for (`engine::Pickups`: the game gathers so for the
//! player only in multiplayer): [`items`], each item on the ground the player could use now, with
//! its bounds and what its inventory holds. An item is taken by the use button's own use
//! ([`take`]), as if it were in focus.

use crate::providers::engine::{self, AbsBoundsFn, Entities, FitsFn, GatheredFn, InventoryFn, Pickups, TypeFn, TypeTestFn, UsableFn, UseFn, GatherFn, PlayerThinkFn};
use monaka_hook::module::Module;
use monaka_hook::{Hooks, InFlight, Original, mem};
use monaka_producer::game::Lack;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering::*};

pub static THINK_ORIGINAL: Original<PlayerThinkFn> = Original::new();

/// The key of the last gather asked for (0: none).
static KEY: AtomicU64 = AtomicU64::new(0);
/// Gathers read, and the entities they held.
static READ: AtomicU64 = AtomicU64::new(0);
static GATHERED_SEEN: AtomicU64 = AtomicU64::new(0);
/// The gather's label in the clip world's list (a C string).
const LABEL: &[u8] = b"monaka: pickups\0";

/// The executable's base, this build's pickup facts and its entities, from the install.
static FACTS: OnceLock<(usize, &'static Pickups, Entities)> = OnceLock::new();

/// The most gathered entities read in one think, and the most items in one inventory.
const GATHERED: usize = 1024;
const MOST_ITEMS: i32 = 64;

/// An item on the ground the player could use now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Item {
    pub entity: usize,
    /// Its absolute bounds: min x, y, z, then max.
    pub bounds: [f32; 6],
    pub holds: Holds,
}

/// What an item's inventory holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Holds {
    /// Ammo and guns only (what the use prompt shows as ammo: a gun the player has gives its ammo),
    /// none of them a gun carried in both hands; whether any of it fits in the player's inventory.
    Ammo { fits: bool },
    /// Anything else (a carried gun, health, armour, a note), or nothing.
    Other,
}

/// Hooks the player's think: after each, [`auto_pickup::think`](crate::cooking::auto_pickup::think).
pub fn install(hooks: &mut Hooks, exe: &Module, build: &'static engine::Build, pickups: &'static Pickups) -> Result<(), Lack> {
    let _ = FACTS.set((exe.base(), pickups, build.entities));
    hooks.site(&THINK_ORIGINAL, exe, &pickups.think, think)?;
    Ok(())
}

unsafe extern "system" fn think(player: usize) {
    let flight = InFlight::enter();
    // SAFETY: forwards the game's own call.
    unsafe { THINK_ORIGINAL.get(&flight)(player) };
    crate::cooking::auto_pickup::think(player);
}

/// A function of this build at `address` (absolute).
///
/// # Safety
/// `address` must start a function of type `F`.
unsafe fn function<F: Copy>(address: usize) -> F {
    debug_assert_eq!(std::mem::size_of::<F>(), std::mem::size_of::<usize>());
    // SAFETY: the caller's claim; `F` is a function pointer type.
    unsafe { std::mem::transmute_copy(&address) }
}

/// An entity's virtual method at `slot` (an offset into its vtable).
///
/// # Safety
/// `entity` must be a live entity whose vtable has a method of type `F` at `slot`.
unsafe fn method<F: Copy>(entity: usize, slot: usize) -> Option<F> {
    let vtable = mem::read::<usize>(entity).filter(|&v| v != 0)?;
    let address = mem::read::<usize>(vtable + slot).filter(|&a| a != 0)?;
    // SAFETY: the caller's claim.
    Some(unsafe { function(address) })
}

/// Gathers read and the entities they held, for the summary.
pub fn counts() -> (u64, u64) {
    (READ.load(Relaxed), GATHERED_SEEN.load(Relaxed))
}

/// Each item on the ground the player (`idPlayer`, the one whose think it is) could use now, among
/// the entities the last gather found; then the next gather is asked for, of the player's bounds
/// (`own`) grown by `around`. On the game thread, after the player's think.
pub fn items(player: usize, own: &[f32; 6], around: f32, mut each: impl FnMut(Item)) {
    let Some(&(base, pickups, entities)) = FACTS.get() else { return };
    let Some(clip) = mem::read::<usize>(base + pickups.client_game).filter(|&c| c != 0).map(|c| c + pickups.clip) else { return };
    let key = KEY.load(Relaxed);
    if let (Some(oldest), Some(newest)) = (mem::read::<u64>(clip + pickups.keys), mem::read::<u64>(clip + pickups.keys + 8))
        // A gather is read only while its key is in the clip world's window, as the game reads it.
        && key != 0
        && (oldest..=newest).contains(&key)
        && let Some(game) = mem::read::<usize>(base + entities.game_local).filter(|&g| g != 0)
    {
        read(base, pickups, entities, clip, game, key, player, &mut each);
    }
    let bounds: [f32; 6] = std::array::from_fn(|k| if k < 3 { own[k] - around } else { own[k] + around });
    let number = mem::read::<u16>(player + pickups.entity_number).unwrap_or(0);
    let mut asked = 0u64;
    // SAFETY: `engine::Pickups::gather` of this build (its fingerprint checked), as the player's
    // usables update asks it, on the game thread inside the player's think; the label is static.
    let _ = unsafe { function::<GatherFn>(base + pickups.gather)(clip, &mut asked, &bounds, pickups.contents, number, LABEL.as_ptr()) };
    KEY.store(asked, Relaxed);
}

/// The items among the entities of the gather `key`.
#[allow(clippy::too_many_arguments)]
fn read(base: usize, pickups: &Pickups, entities: Entities, clip: usize, game: usize, key: u64, player: usize, each: &mut impl FnMut(Item)) {
    let mut numbers = [0i32; GATHERED];
    // SAFETY: `engine::Pickups::gathered` of this build (its fingerprint checked), given a key in
    // the clip world's window and room for `GATHERED` numbers, on the game thread.
    let count = unsafe { function::<GatheredFn>(base + pickups.gathered)(clip, &key, numbers.as_mut_ptr(), GATHERED as i32) } as usize;
    READ.fetch_add(1, Relaxed);
    GATHERED_SEEN.fetch_add(count.min(GATHERED) as u64, Relaxed);
    for &number in &numbers[..count.min(GATHERED)] {
        if !(0..1 << entities.bits).contains(&number) {
            continue;
        }
        let Some(entity) = mem::read::<usize>(game + entities.entities + number as usize * 8).filter(|&e| e != 0 && e != player) else { continue };
        if let Some(item) = item(base, pickups, entity, player) {
            each(item);
        }
    }
}

/// `entity` as an item on the ground the player could use now, asked as the usables update asks.
fn item(base: usize, pickups: &Pickups, entity: usize, player: usize) -> Option<Item> {
    // SAFETY (each call below): this build's functions and vtable slots (its fingerprint checked),
    // on a live entity from the game's table, on the game thread inside the player's think.
    let is_moveable = unsafe { function::<TypeTestFn>(base + pickups.is_moveable) };
    if unsafe { is_moveable(entity) } == 0 {
        return None;
    }
    let ever = unsafe { method::<UsableFn>(entity, pickups.ever_usable)? };
    let now = unsafe { method::<UsableFn>(entity, pickups.usable_now)? };
    if unsafe { ever(entity, player) } == 0 || unsafe { now(entity, player) } == 0 {
        return None;
    }
    let bounds = bounds(entity)?;
    let (items, count) = inventory(pickups, entity)?;
    let is_ammo = unsafe { function::<TypeTestFn>(base + pickups.is_ammo) };
    let is_weapon = unsafe { function::<TypeTestFn>(base + pickups.is_weapon) };
    let (mut ammo, mut any_fits) = (0, false);
    for i in 0..count {
        let Some(it) = mem::read::<usize>(items + i * 8).filter(|&it| it != 0) else { continue };
        let gun = unsafe { is_weapon(it) } != 0;
        let carried = gun && mem::read::<usize>(it + engine::INVENTORY_ITEM_DECL).filter(|&d| d != 0).and_then(|d| mem::read::<u8>(d + pickups.carried_gun)).is_none_or(|c| c != 0);
        if (unsafe { is_ammo(it) } != 0 || gun) && !carried {
            ammo += 1;
            let Some(fits) = (unsafe { method::<FitsFn>(it, pickups.fits) }) else { continue };
            any_fits |= unsafe { fits(it, player, 0) } != 0;
        }
    }
    let holds = if count > 0 && ammo == count { Holds::Ammo { fits: any_fits } } else { Holds::Other };
    Some(Item { entity, bounds, holds })
}

/// An entity's inventory: its items (pointers) and their count.
fn inventory(pickups: &Pickups, entity: usize) -> Option<(usize, usize)> {
    // SAFETY: this build's vtable slot (its fingerprint checked), on an `idProp_Moveable` from the
    // game's table, on the game thread.
    let inventory = unsafe { method::<InventoryFn>(entity, pickups.inventory)? };
    let list = Some(unsafe { inventory(entity) }).filter(|&l| l != 0)?;
    let items = mem::read::<usize>(list)?;
    let count = mem::read::<i32>(list + engine::LIST_COUNT)?.clamp(0, MOST_ITEMS) as usize;
    Some((items, count))
}

/// An item's type and its inventory's by type, for the summary (`idProp_Moveable of idHealthItem`).
pub fn described(entity: usize) -> String {
    let own = type_name(entity).unwrap_or_else(|| "?".to_owned());
    let Some((items, count)) = FACTS.get().and_then(|&(_, pickups, _)| inventory(pickups, entity)) else { return own };
    let held: Vec<String> = (0..count).filter_map(|i| mem::read::<usize>(items + i * 8).filter(|&it| it != 0)).map(|it| type_name(it).unwrap_or_else(|| "?".to_owned())).collect();
    if held.is_empty() { own } else { format!("{own} of {}", held.join(" + ")) }
}

/// An entity's absolute bounds (the player's too).
pub fn bounds(entity: usize) -> Option<[f32; 6]> {
    let &(_, pickups, _) = FACTS.get()?;
    let physics = mem::read::<usize>(entity + pickups.physics).filter(|&p| p != 0)?;
    // SAFETY: this build's physics vtable slot (its fingerprint checked), on a live entity's
    // physics, on the game thread.
    let abs_bounds = unsafe { method::<AbsBoundsFn>(physics, pickups.abs_bounds)? };
    let at = unsafe { abs_bounds(physics, -1) };
    mem::read::<[f32; 6]>(at as usize)
}

/// Takes `entity` (an [`Item`] just found) by the use button's use: whether the game took it.
pub fn take(entity: usize, player: usize) -> bool {
    let Some(&(_, pickups, _)) = FACTS.get() else { return false };
    // SAFETY: this build's vtable slot (its fingerprint checked), on an item [`items`] just found, by
    // the player whose think it is, on the game thread as the use button's use runs.
    let Some(use_it) = (unsafe { method::<UseFn>(entity, pickups.use_it) }) else { return false };
    unsafe { use_it(entity, player, 0) != 0 }
}

/// An entity's type name (`idProp_Moveable`).
pub fn type_name(entity: usize) -> Option<String> {
    // SAFETY: every entity's first virtual method gives its type, on a live entity.
    let get = unsafe { method::<TypeFn>(entity, 0)? };
    let info = Some(unsafe { get(entity) }).filter(|&i| i != 0)?;
    mem::read_c_string(mem::read::<usize>(info + engine::TYPE_NAME)?, 64)
}
