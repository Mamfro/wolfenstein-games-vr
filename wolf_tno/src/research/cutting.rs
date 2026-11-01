//! Probe (`probe_cutting=1`, change-only and capped): what the laser's cut does to what it hits
//! (`engine::Cutting`). Every damage a chain, a cuttable panel or a simple cuttable takes is
//! counted per thing hit; the first hit of each thing, the first one it takes and the one that
//! breaks or fells it are logged with the damage's decl, flags and amount, whether the thing takes
//! that damage, where the hit landed and where the right hand's gun was. Changes nothing.

use crate::providers::engine::{self, Build, CutDamageFn};
use monaka_hook::module::Module;
use monaka_hook::{Hooks, InFlight, Original, mem};
use monaka_producer::{Rejection, log};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::*};

static CHAIN_ORIGINAL: Original<CutDamageFn> = Original::new();
static CUTTABLE_ORIGINAL: Original<CutDamageFn> = Original::new();
static SIMPLE_ORIGINAL: Original<CutDamageFn> = Original::new();

static INSTALLED: AtomicBool = AtomicBool::new(false);
/// Log lines left, so a long run cannot flood the log.
static BUDGET: AtomicU64 = AtomicU64::new(200);
/// Things kept apart; hits on more are only counted.
const THINGS: usize = 32;
static SEEN: Mutex<Vec<Thing>> = Mutex::new(Vec::new());
static OTHERS: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Chain,
    Cuttable,
    Simple,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Chain => "chain",
            Kind::Cuttable => "cuttable panel",
            Kind::Simple => "simple cuttable",
        }
    }
}

struct Thing {
    entity: usize,
    kind: Kind,
    hits: u64,
    taken: u64,
    done: bool,
}

pub fn install(hooks: &mut Hooks, exe: &Module, build: &'static Build) -> Result<(), Rejection> {
    if !super::options().cutting {
        return Ok(());
    }
    let Some(cutting) = build.cutting.as_ref() else { return Ok(()) };
    hooks.site(&CHAIN_ORIGINAL, exe, &cutting.chain, chain)?;
    hooks.site(&CUTTABLE_ORIGINAL, exe, &cutting.cuttable, cuttable)?;
    hooks.site(&SIMPLE_ORIGINAL, exe, &cutting.simple, simple)?;
    INSTALLED.store(true, Relaxed);
    Ok(())
}

pub fn summary() -> Option<String> {
    INSTALLED.load(Relaxed).then_some(())?;
    let seen = SEEN.lock().ok()?;
    if seen.is_empty() {
        return Some("cutting: nothing the laser cuts was damaged".to_owned());
    }
    let things: Vec<String> = seen
        .iter()
        .map(|t| format!("{} {:#x}: {} hits, {} taken, {}", t.kind.name(), t.entity, t.hits, t.taken, if t.done { done_word(t.kind) } else { "still whole" }))
        .collect();
    let others = OTHERS.load(Relaxed);
    let more = if others > 0 { format!("; {others} hits on more things") } else { String::new() };
    Some(format!("cutting: {}{more}", things.join("; ")))
}

fn done_word(kind: Kind) -> &'static str {
    match kind {
        Kind::Chain => "broken",
        Kind::Cuttable => "cut away",
        Kind::Simple => "fallen",
    }
}

// `try_update`, its new name, needs Rust 1.95; the workspace builds on 1.88.
#[allow(deprecated)]
fn line(text: impl FnOnce() -> String) {
    if BUDGET.fetch_update(Relaxed, Relaxed, |b| b.checked_sub(1)).is_ok() {
        log!("{}", text());
    }
}

/// Whether `entity` takes `decl`'s damage, as its handler decides (none: unreadable).
fn takes(kind: Kind, entity: usize, decl: usize) -> Option<bool> {
    let flags = mem::read::<u32>(decl + engine::DAMAGE_FLAGS)?;
    Some(match kind {
        Kind::Chain => {
            let mask = mem::read::<u32>(entity + engine::CHAIN_MASK)?;
            (flags & engine::DAMAGE_ANY_CHAIN != 0 || flags & mask != 0) && !done(kind, entity)?
        }
        Kind::Cuttable => {
            let strength = mem::read::<f32>(entity + engine::CUTTABLE_STRENGTH)?;
            let least = mem::read::<f32>(decl + engine::DAMAGE_LEAST)?;
            flags & engine::DAMAGE_CUTS != 0 && strength <= least && !done(kind, entity)?
        }
        Kind::Simple => {
            let mask = mem::read::<u32>(entity + engine::SIMPLE_CUTTABLE_MASK)?;
            flags & mask != 0 && !done(kind, entity)?
        }
    })
}

/// Whether `entity` is broken, cut away or fallen.
fn done(kind: Kind, entity: usize) -> Option<bool> {
    Some(match kind {
        Kind::Chain => mem::read::<i32>(entity + engine::CHAIN_BROKEN_AT)? >= 0,
        Kind::Cuttable => mem::read::<i32>(entity + engine::CUTTABLE_GRAPH)? < 0,
        Kind::Simple => mem::read::<i32>(entity + engine::SIMPLE_CUTTABLE_FALLEN_AT)? != -1,
    })
}

/// The damage, the thing's side of the check and the places, for a log line.
fn describe(kind: Kind, entity: usize, decl: usize, scale: f32, trace: usize) -> String {
    let name = crate::providers::held_item::decl_name(decl).unwrap_or_else(|| "?".to_owned());
    let flags = mem::read::<u32>(decl + engine::DAMAGE_FLAGS);
    let amount = mem::read::<f32>(decl + engine::DAMAGE_LEAST).zip(mem::read::<f32>(decl + engine::DAMAGE_MOST));
    let side = match kind {
        Kind::Chain => format!("chain mask {:x?}", mem::read::<u32>(entity + engine::CHAIN_MASK)),
        Kind::Cuttable => format!("strength {:?}", mem::read::<f32>(entity + engine::CUTTABLE_STRENGTH)),
        Kind::Simple => format!("mask {:x?}, health {:?}", mem::read::<u32>(entity + engine::SIMPLE_CUTTABLE_MASK), mem::read::<f32>(entity + engine::SIMPLE_CUTTABLE_HEALTH)),
    };
    let point = (trace != 0).then(|| mem::read::<[f32; 3]>(trace + engine::TRACE_POINT)).flatten();
    let origin = mem::read::<[f32; 3]>(entity + engine::ENTITY_ORIGIN);
    let gun = crate::providers::held_item::placed_origin(monaka_contract::protocol::RIGHT_HAND);
    format!("damage {name} (flags {flags:x?}, amount {amount:?} x {scale}), {side}; hit at {point:?}, the thing at {origin:?}, the right gun at {gun:?}")
}

fn damage(kind: Kind, original: &Original<CutDamageFn>, args: (usize, usize, usize, usize, f32, usize, usize)) -> usize {
    let flight = InFlight::enter();
    let (entity, inflictor, attacker, decl, scale, direction, trace) = args;
    let takes = (decl != 0).then(|| takes(kind, entity, decl)).flatten();
    let before = done(kind, entity);
    // SAFETY: forwards the game's own call.
    let result = unsafe { original.get(&flight)(entity, inflictor, attacker, decl, scale, direction, trace) };
    let after = done(kind, entity);
    let finished = before == Some(false) && after == Some(true);
    note(kind, entity, takes, finished, || describe(kind, entity, decl, scale, trace));
    result
}

fn note(kind: Kind, entity: usize, takes: Option<bool>, finished: bool, text: impl FnOnce() -> String) {
    let taken = takes == Some(true);
    let Ok(mut seen) = SEEN.lock() else { return };
    let index = match seen.iter().position(|t| t.entity == entity) {
        Some(index) => index,
        None if seen.len() < THINGS => {
            seen.push(Thing { entity, kind, hits: 0, taken: 0, done: false });
            seen.len() - 1
        }
        None => {
            OTHERS.fetch_add(1, Relaxed);
            return;
        }
    };
    let thing = &mut seen[index];
    thing.hits += 1;
    thing.taken += u64::from(taken);
    thing.done |= finished;
    let why = match (thing.hits == 1, taken && thing.taken == 1, finished) {
        (_, _, true) => Some(done_word(kind)),
        (true, _, _) => Some("first hit"),
        (_, true, _) => Some("first damage it takes"),
        _ => None,
    };
    if let Some(why) = why {
        let build = crate::providers::eye_site::builds();
        let takes = match takes {
            Some(true) => "takes it",
            Some(false) => "does not take it",
            None => "unknown whether it takes it",
        };
        line(|| format!("cutting: {} {entity:#x} {why} at view build {build} (hit {}): {takes}; {}", kind.name(), thing.hits, text()));
    }
}

unsafe extern "system" fn chain(entity: usize, inflictor: usize, attacker: usize, decl: usize, scale: f32, direction: usize, trace: usize) -> usize {
    damage(Kind::Chain, &CHAIN_ORIGINAL, (entity, inflictor, attacker, decl, scale, direction, trace))
}

unsafe extern "system" fn cuttable(entity: usize, inflictor: usize, attacker: usize, decl: usize, scale: f32, direction: usize, trace: usize) -> usize {
    damage(Kind::Cuttable, &CUTTABLE_ORIGINAL, (entity, inflictor, attacker, decl, scale, direction, trace))
}

unsafe extern "system" fn simple(entity: usize, inflictor: usize, attacker: usize, decl: usize, scale: f32, direction: usize, trace: usize) -> usize {
    damage(Kind::Simple, &SIMPLE_ORIGINAL, (entity, inflictor, attacker, decl, scale, direction, trace))
}
