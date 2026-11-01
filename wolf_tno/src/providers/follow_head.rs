//! Provider part, to be: the arms model where the head is (a hand rig's body following the head,
//! once Monaka has the trait).
//!
//! The eyes are drawn from the game camera moved to the head (its position, the dominant eye on
//! the camera), but the game places its arms model, and whatever rides it (the shoulders, an item
//! a scripted moment puts on, such as goggles), from the camera itself: leaning, the eyes leave
//! them behind and see them from outside. So at each view build the head's shift from the game
//! camera is kept ([`set_shift`]), and the hands' placement from the player's first-person view
//! (`engine::FollowHead::update_position`) sees that view moved by it, then gets it back. So does
//! the item placer for an item on no hand's tag (`held_item::place_item`), and a worn item's draw
//! (goggles: `engine::ITEM_ON_VIEW`), which places it again on the view object's position
//! (`engine::FollowHead::view_position`). The camera, the shots and everything else stay the
//! game's. The hand rig reads the arms' moved view ([`as_placed`]), so the hands still land on the
//! controllers.
//!
//! While the arms' animation moves the game's camera itself (a scripted moment:
//! `engine::CAMERA_JOINT`, asked as the engine asks it) the view is the arms', and moving the arms
//! by the head would move the view the next frame: the arms stay the game's then. So they do while
//! another camera has the view (a cutscene's). A worn item follows the head in both: it is drawn
//! on whatever view the game shows, turned with the head too, and feeds nothing back.

use crate::providers::engine::{self, FollowHead, HandsAnimatorFn, HandsLayout, ItemDrawFn, JointIndexFn, UpdatePositionFn};
use monaka_core::math::{Vec3, add};
use monaka_core::sync::Locked;
use monaka_hook::module::Module;
use monaka_hook::{Hooks, InFlight, Original, mem};
use monaka_producer::Rejection;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::*};
use std::sync::{Mutex, OnceLock};

pub static UPDATE_ORIGINAL: Original<UpdatePositionFn> = Original::new();
pub static ITEM_DRAW_ORIGINAL: Original<ItemDrawFn> = Original::new();

/// The engine's functions (absolute) and where the player's views are, from the install.
struct Facts {
    animator: HandsAnimatorFn,
    joint_index: JointIndexFn,
    /// The first-person view the arms and held items are placed from.
    player_view: usize,
    /// The view object's position a worn item is drawn on.
    view_position: usize,
    /// `idPresentableAnimatedEntity`: its animated entity (where the arms model's axis is).
    model_animated: usize,
}

/// The arms model's turn from the player's view at its last placement (`view rows · turn = model
/// rows`): none for a model turned as the view. The game can place it turned otherwise (an item
/// carried level whatever the head's pitch: the tea tray), and the rig must place the hands in the
/// model as it is drawn.
static PLACED_TURN: Mutex<Option<[f32; 9]>> = Mutex::new(None);
/// Whether the log said the model was placed turned from the view.
static TURN_LOGGED: AtomicBool = AtomicBool::new(false);
/// A turn under this (degrees) counts as none.
const NO_TURN: f32 = 0.5;

static FACTS: OnceLock<Facts> = OnceLock::new();
/// The head's shift from the game camera at the last view build (world units); none while it is
/// not to be followed.
static SHIFT: Mutex<Option<Vec3>> = Mutex::new(None);
/// The head's turn from the game camera at the last view build (row-major, applied to axis rows:
/// rows · turn), for a worn item; none while it is not to be followed.
static TURN: Mutex<Option<[f32; 9]>> = Mutex::new(None);
/// Whether the eyes were built on the player's own camera at the last view build: the arms and
/// held items follow the head only then (another camera's view, a cutscene's, has them as the
/// game places them); a worn item, drawn on whatever view the game shows, follows it always.
static OWN_CAMERA: AtomicBool = AtomicBool::new(false);
/// The shift the hands' last placement was moved by (none: placed as the game had it).
static PLACED: Mutex<Option<Vec3>> = Mutex::new(None);
/// Placements moved by the head, and those left the game's in a scripted moment.
static MOVED: AtomicU64 = AtomicU64::new(0);
static SCRIPTED: AtomicU64 = AtomicU64::new(0);

/// Hooks the hands' placement (once).
pub fn install(hooks: &mut Hooks, exe: &Module, facts: &FollowHead, layout: &HandsLayout) -> Result<(), Rejection> {
    if UPDATE_ORIGINAL.is_set() {
        return Ok(());
    }
    // SAFETY: the engine's functions of this build (its fingerprint checked), of these signatures.
    let (animator, joint_index) = unsafe { (std::mem::transmute::<usize, HandsAnimatorFn>(exe.at(facts.animator)), std::mem::transmute::<usize, JointIndexFn>(exe.at(facts.joint_index))) };
    let _ = FACTS.set(Facts { animator, joint_index, player_view: layout.player_view, view_position: facts.view_position, model_animated: layout.model_animated });
    // SAFETY: the detours have the targets' signatures; their prologues are decoded and moved.
    unsafe {
        hooks.inline_decoded(&UPDATE_ORIGINAL, "hands placement", exe.at(facts.update_position), update_position as UpdatePositionFn)?;
        hooks.inline_decoded(&ITEM_DRAW_ORIGINAL, "hand item draw", exe.at(facts.item_draw), item_draw as ItemDrawFn)?;
    }
    Ok(())
}

/// The head's shift from the game camera now (world units), or none.
pub fn set_shift(shift: Option<Vec3>, own_camera: bool) {
    *SHIFT.locked() = shift.filter(|s| s.iter().all(|v| v.is_finite()));
    OWN_CAMERA.store(own_camera, Relaxed);
}

/// The head's turn from the game camera now, from the camera's axis rows `camera` and the head's
/// `head` (forward, left, up): `camera^T · head`, so `camera · turn = head`. None: not followed.
pub fn set_turn(rows: Option<([f32; 9], [f32; 9])>) {
    *TURN.locked() = rows.map(|(camera, head)| turn_between(&camera, &head)).filter(|t| t.iter().all(|v| v.is_finite()));
}

/// `camera^T · head` for axis rows `camera` and `head`.
pub fn turn_between(camera: &[f32; 9], head: &[f32; 9]) -> [f32; 9] {
    std::array::from_fn(|i| {
        let (row, col) = (i / 3, i % 3);
        (0..3).map(|k| camera[k * 3 + row] * head[k * 3 + col]).sum()
    })
}

/// Axis rows `rows` turned by `turn` (`rows · turn`).
pub fn turned_rows(rows: &[f32; 9], turn: &[f32; 9]) -> [f32; 9] {
    std::array::from_fn(|i| {
        let (row, col) = (i / 3, i % 3);
        (0..3).map(|k| rows[row * 3 + k] * turn[k * 3 + col]).sum()
    })
}

/// The head's shift and turn from the game camera at the last view build, whichever camera that
/// was (the player's own or a cutscene's).
pub fn head_from_camera() -> Option<(Vec3, [f32; 9])> {
    Some(((*SHIFT.locked())?, (*TURN.locked())?))
}

/// The shift the arms model is placed with now, if it follows the head.
fn shift() -> Option<Vec3> {
    FACTS.get().and(*SHIFT.locked())
}

/// The player's view `view` (origin, then axis rows) moved and turned as the arms model's last
/// placement was: where that model is drawn.
pub fn as_placed(mut view: [f32; 12]) -> [f32; 12] {
    if let Some(shift) = *PLACED.locked() {
        let moved = add([view[0], view[1], view[2]], shift);
        view[..3].copy_from_slice(&moved);
    }
    if let Some(turn) = *PLACED_TURN.locked() {
        let rows: [f32; 9] = std::array::from_fn(|i| view[3 + i]);
        view[3..].copy_from_slice(&turned_rows(&rows, &turn));
    }
    view
}

/// The angle (degrees) of the turn `turn` (a rotation).
pub fn turn_degrees(turn: &[f32; 9]) -> f32 {
    ((turn[0] + turn[4] + turn[8] - 1.0) * 0.5).clamp(-1.0, 1.0).acos().to_degrees()
}

/// After the game placed the arms model of the hands `hands`: its turn from the player's view
/// (none when it is turned as the view).
fn measure_turn(facts: &Facts, hands: usize) -> Option<[f32; 9]> {
    let player = mem::read::<usize>(hands + engine::HANDS_PLAYER).filter(|&p| p != 0)?;
    let view = mem::read::<[f32; 9]>(player + facts.player_view + 12)?;
    let presentable = mem::read::<usize>(hands + engine::HANDS_ARMS_MODEL).filter(|&p| p != 0)?;
    let entity = mem::read::<usize>(presentable + facts.model_animated).filter(|&e| e != 0)?;
    let model = mem::read::<[f32; 9]>(entity + engine::ANIMATED_AXIS)?;
    let turn = turn_between(&view, &model);
    if !turn.iter().all(|v| v.is_finite()) {
        return None;
    }
    let degrees = turn_degrees(&turn);
    if degrees < NO_TURN {
        return None;
    }
    if !TURN_LOGGED.swap(true, Relaxed) {
        monaka_producer::log!("arms: the game placed the arms model turned {degrees:.1} degrees from the player's view: the hands are placed in it as turned");
    }
    Some(turn)
}

pub fn summary() -> String {
    format!("arms follow the head: {} placements moved, {} left the game's in a scripted moment", MOVED.load(Relaxed), SCRIPTED.load(Relaxed))
}

/// Whether the arms' animation of the hands `hands` moves the game's camera (a scripted moment).
fn camera_on_arms(facts: &Facts, hands: usize) -> bool {
    // SAFETY: the engine's own query, on the hands it is placing, on the game thread.
    let animator = unsafe { (facts.animator)(hands) };
    if animator == 0 {
        return false;
    }
    let mut joint = -1i16;
    // SAFETY: as above; the name is NUL-terminated and `joint` takes the answer.
    unsafe { (facts.joint_index)(animator, &mut joint, engine::CAMERA_JOINT.as_ptr()) };
    joint >= 0
}

pub unsafe extern "system" fn update_position(hands: usize) {
    let flight = InFlight::enter();
    let shift = moved_view(hands, || {
        // SAFETY: forwards the game's own call.
        unsafe { UPDATE_ORIGINAL.get(&flight)(hands) };
    });
    *PLACED.locked() = shift;
    *PLACED_TURN.locked() = FACTS.get().and_then(|facts| measure_turn(facts, hands));
}

pub unsafe extern "system" fn item_draw(item: usize, hands: usize) {
    let flight = InFlight::enter();
    // SAFETY: forwards the game's own call.
    let draw = || unsafe { ITEM_DRAW_ORIGINAL.get(&flight)(item, hands) };
    if mem::read::<i32>(item) != Some(engine::ITEM_ON_VIEW) {
        draw();
        return;
    }
    // A worn item is drawn on the view object's position: that follows the head, turned with it
    // too (its tilt, which the game camera never has).
    let rows = FACTS.get().and_then(|facts| mem::read::<usize>(hands + engine::HANDS_PLAYER).filter(|&p| p != 0).map(|p| p + facts.view_position + 12));
    let turned = rows.zip(*TURN.locked()).and_then(|(at, turn)| {
        let before = mem::read::<[f32; 9]>(at)?;
        let after = turned_rows(&before, &turn);
        mem::write(at, after);
        Some((at, before, after))
    });
    // Drawing it feeds nothing back to the camera: it follows in a scripted moment too.
    let _ = moved_at(hands, |facts| facts.view_position, false, draw);
    if let Some((at, before, after)) = turned
        && mem::read::<[f32; 9]>(at) == Some(after)
    {
        mem::write(at, before);
    }
}

/// `place` (the game placing something of the hands `hands` from the player's view) with that view
/// moved by the head, then given back, while the eyes are on the player's own camera; the shift it
/// was placed with (none: as the game had it).
pub fn moved_view(hands: usize, place: impl FnOnce()) -> Option<Vec3> {
    if !OWN_CAMERA.load(Relaxed) {
        place();
        return None;
    }
    moved_at(hands, |facts| facts.player_view, true, place)
}

/// `place` with the player's view at `field` (an offset in the player, origin first) moved by the
/// head, then given back; with `arms_camera`, not while the arms' animation moves the camera (the
/// placement would move it the next frame).
fn moved_at(hands: usize, field: fn(&Facts) -> usize, arms_camera: bool, place: impl FnOnce()) -> Option<Vec3> {
    let moved = match (FACTS.get(), shift()) {
        (Some(facts), Some(shift)) => {
            let view = mem::read::<usize>(hands + engine::HANDS_PLAYER).filter(|&p| p != 0).map(|p| p + field(facts));
            match view.and_then(|v| Some((v, mem::read::<Vec3>(v)?))) {
                Some(_) if arms_camera && camera_on_arms(facts, hands) => {
                    SCRIPTED.fetch_add(1, Relaxed);
                    None
                }
                Some((view, origin)) => {
                    let shifted = add(origin, shift);
                    mem::write(view, shifted);
                    Some((view, origin, shifted))
                }
                None => None,
            }
        }
        _ => None,
    };
    place();
    let (view, origin, shifted) = moved?;
    MOVED.fetch_add(1, Relaxed);
    // The view back as the game made it, unless the placement made it anew meanwhile.
    if mem::read::<Vec3>(view) == Some(shifted) {
        mem::write(view, origin);
    }
    Some(std::array::from_fn(|k| shifted[k] - origin[k]))
}

#[cfg(test)]
mod tests;
