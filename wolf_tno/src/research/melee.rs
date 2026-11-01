//! Probe (`probe_melee=1`): how the player's melee hit is found (`engine::Melee`). Logs each start
//! and end of the hands' melee trace with the event that opened it and the trace's settings, which
//! core sweeps it (the server one damages at once, the client one sends a message) with the points
//! it is handed next to where the right hand's item was placed, and whether an update hit. Changes
//! nothing.

use crate::providers::engine::{self, AnimEventFn, Build, Melee, MeleeCoreFn};
use monaka_hook::module::Module;
use monaka_hook::{Hooks, InFlight, Original, mem};
use monaka_producer::{Rejection, log};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering::*};

static START_JOINT_ORIGINAL: Original<AnimEventFn> = Original::new();
static START_TAG_ORIGINAL: Original<AnimEventFn> = Original::new();
static START_SIMPLE_ORIGINAL: Original<AnimEventFn> = Original::new();
static END_ORIGINAL: Original<AnimEventFn> = Original::new();
static CORE_SERVER_ORIGINAL: Original<MeleeCoreFn> = Original::new();
static CORE_CLIENT_ORIGINAL: Original<MeleeCoreFn> = Original::new();

struct Setup {
    exe: Module,
    melee: &'static Melee,
}

static SETUP: OnceLock<Setup> = OnceLock::new();
/// Log lines left, so a long run cannot flood the log.
static BUDGET: AtomicU64 = AtomicU64::new(800);
static EVENTS: [AtomicU64; 4] = [AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)];
static CORE_CALLS: [AtomicU64; 2] = [AtomicU64::new(0), AtomicU64::new(0)];
static HITS: AtomicU64 = AtomicU64::new(0);

const EVENT_NAMES: [&str; 4] = ["start (hands joint)", "start (item tag)", "start (eye line)", "end"];

pub fn install(hooks: &mut Hooks, exe: &Module, build: &'static Build) -> Result<(), Rejection> {
    if !super::options().melee {
        return Ok(());
    }
    let Some(melee) = build.melee.as_ref() else {
        log!("probe_melee: the melee trace is not mapped for {}", build.game);
        return Ok(());
    };
    let _ = SETUP.set(Setup { exe: *exe, melee });
    // SAFETY: each target is checked against its exact prologue; the detours have their signatures.
    unsafe {
        // Physical melee owns the animation events when it is on.
        if crate::providers::melee::hooked() {
            log!("probe_melee: the animation events are physical melee's; only the cores are watched");
        } else {
            hooks.inline(&START_JOINT_ORIGINAL, "melee start (joint)", exe.at(melee.start_joint), &engine::START_JOINT_PROLOGUE, start_joint)?;
            hooks.inline(&START_TAG_ORIGINAL, "melee start (tag)", exe.at(melee.start_tag), &engine::START_TAG_PROLOGUE, start_tag)?;
            hooks.inline(&START_SIMPLE_ORIGINAL, "melee start (eye)", exe.at(melee.start_simple), &engine::START_TAG_PROLOGUE, start_simple)?;
            hooks.inline(&END_ORIGINAL, "melee end", exe.at(melee.end), &engine::START_JOINT_PROLOGUE, end)?;
        }
        hooks.inline(&CORE_SERVER_ORIGINAL, "melee core (server)", exe.at(melee.core_server), melee.core_server_prologue, core_server)?;
        hooks.inline(&CORE_CLIENT_ORIGINAL, "melee core (client)", exe.at(melee.core_client), &engine::CORE_CLIENT_PROLOGUE, core_client)?;
    }
    log!("probe_melee: on; the game is the server: {:?}", is_server());
    Ok(())
}

pub fn summary() -> Option<String> {
    SETUP.get()?;
    let events: Vec<String> = EVENT_NAMES.iter().zip(&EVENTS).map(|(n, c)| format!("{n} {}", c.load(Relaxed))).collect();
    Some(format!(
        "probe_melee: {}; core server {} client {}, {} updates hit; the game is the server: {:?}",
        events.join(", "),
        CORE_CALLS[0].load(Relaxed),
        CORE_CALLS[1].load(Relaxed),
        HITS.load(Relaxed),
        is_server()
    ))
}

fn is_server() -> Option<bool> {
    let setup = SETUP.get()?;
    let game = mem::read::<usize>(setup.exe.at(setup.melee.game_local)).filter(|&p| p != 0)?;
    mem::read::<u8>(game + setup.melee.is_server).map(|b| b != 0)
}

// `try_update`, its new name, needs Rust 1.95; the workspace builds on 1.88.
#[allow(deprecated)]
fn line(text: impl FnOnce() -> String) {
    if BUDGET.fetch_update(Relaxed, Relaxed, |b| b.checked_sub(1)).is_ok() {
        log!("{}", text());
    }
}

/// The trace's settings as the event left them.
fn describe(trace: usize, f: &engine::MeleeTraceFields) -> String {
    let samples = mem::read::<i32>(trace + f.num_samples);
    let kinds = mem::read::<[u8; 2]>(trace + f.melee_type);
    let joint2 = mem::read::<i16>(trace + engine::MELEE_JOINT2);
    let tag = mem::read::<u8>(trace + engine::MELEE_USE_TAG);
    format!("samples {samples:?}, melee type and bounds {kinds:?}, second joint {joint2:?}, tag {tag:?}")
}

fn event(kind: usize, original: &Original<AnimEventFn>, hands: usize, args: [usize; 5]) -> usize {
    let flight = InFlight::enter();
    EVENTS[kind].fetch_add(1, Relaxed);
    // SAFETY: forwards the game's own call.
    let result = unsafe { original.get(&flight)(hands, args[0], args[1], args[2], args[3], args[4]) };
    if let Some(setup) = SETUP.get() {
        let trace = hands + setup.melee.trace;
        line(|| format!("probe_melee: {} at view build {}: {}", EVENT_NAMES[kind], crate::providers::eye_site::builds(), describe(trace, &setup.melee.fields)));
    }
    result
}

unsafe extern "system" fn start_joint(hands: usize, a: usize, b: usize, c: usize, d: usize, e: usize) -> usize {
    event(0, &START_JOINT_ORIGINAL, hands, [a, b, c, d, e])
}

unsafe extern "system" fn start_tag(hands: usize, a: usize, b: usize, c: usize, d: usize, e: usize) -> usize {
    event(1, &START_TAG_ORIGINAL, hands, [a, b, c, d, e])
}

unsafe extern "system" fn start_simple(hands: usize, a: usize, b: usize, c: usize, d: usize, e: usize) -> usize {
    event(2, &START_SIMPLE_ORIGINAL, hands, [a, b, c, d, e])
}

unsafe extern "system" fn end(hands: usize, a: usize, b: usize, c: usize, d: usize, e: usize) -> usize {
    event(3, &END_ORIGINAL, hands, [a, b, c, d, e])
}

fn read_point(p: *const f32) -> Option<[f32; 3]> {
    (!p.is_null()).then(|| mem::read::<[f32; 3]>(p as usize)).flatten()
}

fn core(which: usize, original: &Original<MeleeCoreFn>, trace: usize, attacker: usize, point: *const f32, point2: *const f32, secondary: usize) -> usize {
    let flight = InFlight::enter();
    CORE_CALLS[which].fetch_add(1, Relaxed);
    let before = SETUP.get().map(|s| (mem::read::<[f32; 3]>(trace + s.melee.fields.prev_pos), mem::read::<i32>(trace + s.melee.fields.num_samples)));
    // SAFETY: forwards the game's own call.
    let hit = unsafe { original.get(&flight)(trace, attacker, point, point2, secondary) };
    if hit as u8 != 0 {
        HITS.fetch_add(1, Relaxed);
    }
    // The core runs every frame; with no updates left (closed) it returns at once.
    if let (Some(setup), Some((prev, samples))) = (SETUP.get(), before)
        && (samples.is_some_and(|s| s > 0) || hit as u8 != 0)
    {
        let hit_entity = mem::read::<usize>(trace + setup.melee.fields.hit_entity);
        let has_hit = mem::read::<u8>(trace + engine::MELEE_HAS_HIT);
        line(|| {
            format!(
                "probe_melee: core {} at view build {}: samples {samples:?}, from {prev:?} to {:?}, second point {:?}, secondary {}; right item placed at {:?}; hit {} (has hit {has_hit:?}, entity {hit_entity:x?})",
                ["server", "client"][which],
                crate::providers::eye_site::builds(),
                read_point(point),
                read_point(point2),
                secondary as u8,
                crate::providers::held_item::placed_origin(monaka_contract::protocol::RIGHT_HAND),
                hit as u8
            )
        });
    }
    hit
}

unsafe extern "system" fn core_server(trace: usize, attacker: usize, point: *const f32, point2: *const f32, secondary: usize) -> usize {
    core(0, &CORE_SERVER_ORIGINAL, trace, attacker, point, point2, secondary)
}

unsafe extern "system" fn core_client(trace: usize, attacker: usize, point: *const f32, point2: *const f32, secondary: usize) -> usize {
    core(1, &CORE_CLIENT_ORIGINAL, trace, attacker, point, point2, secondary)
}
