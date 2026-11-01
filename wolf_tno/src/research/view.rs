//! Probe (`probe_view=1`): the player's render view at the view builder (`eye_site`), for the
//! first builds in full and then whenever the game camera's field of view changes (field checks);
//! the game camera's yaw and pitch beside the head share head aim put into the command it was made
//! from, whenever the difference moves (does the camera follow the command's angles?). With
//! `probe_player=1`, in The Old Blood: the player's fields that can hand the view to something else
//! (which one a cutscene uses), whenever they change.

use monaka_body::look::Share;
use monaka_hook::mem;
use monaka_producer::log;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::*};
use monaka_core::sync::Locked;

/// The game camera's last logged horizontal field of view (f32 bits), and lines so far.
static PROBED_FOV: AtomicU32 = AtomicU32::new(0);
static PROBED_FOV_LINES: AtomicU32 = AtomicU32::new(0);
/// The last logged camera-minus-baked yaw and pitch, the build it was logged at, and lines so far.
static PROBED_AIM: Mutex<(f32, f32, u64)> = Mutex::new((f32::NAN, f32::NAN, 0));
static PROBED_AIM_LINES: AtomicU32 = AtomicU32::new(0);
/// Player 0's latest camera-minus-baked yaw and pitch (kept in every run).
static LATEST_AIM: Mutex<(f32, f32)> = Mutex::new((f32::NAN, f32::NAN));
/// A logged camera-minus-baked change smaller than this (degrees) is not logged again, nor more
/// often than every this many builds otherwise.
const AIM_STEP: f32 = 0.3;
const AIM_HEARTBEAT: u64 = 120;

/// A view (origin, then axis rows forward, left, up) turned `probe_extra_yaw` degrees more about
/// the world's up axis (id: z up), left positive.
pub fn with_extra_yaw(origin_axis: &mut [f32; 12]) {
    let degrees = super::options().extra_yaw;
    if degrees == 0.0 {
        return;
    }
    let (s, c) = degrees.to_radians().sin_cos();
    for row in 0..3 {
        let (x, y) = (origin_axis[3 + row * 3], origin_axis[4 + row * 3]);
        origin_axis[3 + row * 3] = x * c - y * s;
        origin_axis[4 + row * 3] = x * s + y * c;
    }
}

/// Lines of [`trace`] so far, and the most.
static TRACE_LINES: AtomicU32 = AtomicU32::new(0);
const TRACE_MOST: u32 = 600;

/// The forward row's yaw (degrees, left positive) of a view (origin, then axis rows).
fn view_yaw(origin_axis: &[f32; 12]) -> f32 {
    origin_axis[4].atan2(origin_axis[3]).to_degrees()
}

/// Each player view build (`probe_view=1`): the head pose it was drawn from, the head share the
/// game camera carries (its command's), the camera's yaw and the drawn view's: which head a drawn
/// frame follows, set against its image (`harness_pose_match.py`).
pub fn trace(build: u64, pose: &monaka_contract::protocol::HeadPose, baked: Option<Share>, camera: &[f32; 12], drawn: &[f32; 12]) {
    if !super::options().view || TRACE_LINES.fetch_add(1, Relaxed) >= TRACE_MOST {
        return;
    }
    let (yaw, pitch) = monaka_core::camera::pointing_yaw_pitch(pose.orientation);
    let share = baked.map_or(f32::NAN, |s| s.yaw.to_degrees());
    log!(
        "trace build {build}: pose yaw {:.3} pitch {:.3}, share yaw {share:.3}, camera yaw {:.3}, drawn yaw {:.3}",
        yaw.to_degrees(),
        pitch.to_degrees(),
        view_yaw(camera),
        view_yaw(drawn)
    );
}

/// One active player's render view as the game built it, in view build `build`, and the head
/// share baked into it (None without head aim).
pub fn seen(build: u64, player: usize, fov: [f32; 2], origin_axis: &[f32; 12], baked: Option<Share>) {
    // id axes: x forward, y left, z up; the forward row's yaw (left positive) and pitch (up positive).
    let [x, y, z] = [origin_axis[3], origin_axis[4], origin_axis[5]];
    let (yaw, pitch) = (y.atan2(x).to_degrees(), z.clamp(-1.0, 1.0).asin().to_degrees());
    let (baked_yaw, baked_pitch) = baked.map_or((f32::NAN, f32::NAN), |s| (s.yaw.to_degrees(), s.pitch.to_degrees()));
    let (dy, dp) = (yaw - baked_yaw, pitch - baked_pitch);
    if player == 0 {
        *LATEST_AIM.locked() = (dy, dp);
    }
    if !super::options().view {
        return;
    }
    if build < 3 {
        log!("player {player} view: fov {fov:?}, origin and axis {origin_axis:?}");
    }
    let mut last = PROBED_AIM.locked();
    let moved = !((dy - last.0).abs() < AIM_STEP && (dp - last.1).abs() < AIM_STEP);
    if (moved || build >= last.2 + AIM_HEARTBEAT) && PROBED_AIM_LINES.fetch_add(1, Relaxed) < 3000 {
        *last = (dy, dp, build);
        log!(
            "build {build}: camera yaw {yaw:.2} pitch {pitch:.2}; baked head yaw {baked_yaw:.2} pitch {baked_pitch:.2}; camera minus baked yaw {dy:.2} pitch {dp:.2}; origin {:.1} {:.1} {:.1}",
            origin_axis[0],
            origin_axis[1],
            origin_axis[2]
        );
    }
    drop(last);
    let last_fov = f32::from_bits(PROBED_FOV.load(Relaxed));
    if (fov[0] - last_fov).abs() > 0.25 && PROBED_FOV_LINES.fetch_add(1, Relaxed) < 400 {
        PROBED_FOV.store(fov[0].to_bits(), Relaxed);
        log!("build {build}: game fov {fov:?}");
    }
}

/// The Old Blood's `idPresentablePlayer` (P) and `idPlayer` (P + ENTITY) fields that can take the
/// view: the controlled presentables' count, the camera-joint animation state (0 off) and the view
/// angles it adds, the view flags byte; the player's attach entity and controlled camera entity
/// handles (0x3fff in the low 14 bits: none), its death, thrown, interaction, slow-motion and
/// spring cameras.
const TOB_CONTROLLED_COUNT: usize = 0x7e98;
const TOB_ANIM_CAMERA_STATE: usize = 0xccac;
const TOB_ADD_VIEW_ANGLES: usize = 0xcc90;
const TOB_VIEW_FLAGS: usize = 0xccf8;
const TOB_ENTITY: usize = 0x518;
const TOB_ATTACH_ENTITY: usize = 0x3f48;
const TOB_CAMERA_ENTITY: usize = 0x3f40;
const TOB_CAMERAS: usize = 0x3f20;
const TOB_SPRING_CAMERA: usize = 0x40e8;

/// Whether this run probes the fields above (`probe_player`, The Old Blood's only).
pub static PLAYER_FIELDS: AtomicBool = AtomicBool::new(false);
static PROBED_PLAYER: Mutex<String> = Mutex::new(String::new());
static PROBED_PLAYER_LINES: AtomicU32 = AtomicU32::new(0);

/// The view-taking fields of `player` (an `idPresentablePlayer`), logged when they change with
/// `probe_player`: which one a cutscene uses is found from play sessions.
pub fn player(build: u64, player: usize) {
    if !PLAYER_FIELDS.load(Relaxed) || player == 0 {
        return;
    }
    let handle = |at: usize| mem::read::<u32>(at).map(|h| if h & 0x3fff == 0x3fff { "none".to_string() } else { format!("{h:#x}") });
    let entity = mem::read::<usize>(player + TOB_ENTITY).filter(|&e| e != 0);
    let cameras = entity.and_then(|e| mem::read::<[usize; 4]>(e + TOB_CAMERAS)).map(|c| c.map(|p| u8::from(p != 0)));
    let add = mem::read::<[f32; 3]>(player + TOB_ADD_VIEW_ANGLES).map(|a| a.map(|v| (v * 10.0).round() / 10.0));
    let now = format!(
        "controlled {:?}, camera-joint state {:?} adding {:?}, view flags {:?}; player attach {:?}, camera entity {:?}, death/thrown/interaction/slow-motion cameras {:?}, spring camera {}",
        mem::read::<i32>(player + TOB_CONTROLLED_COUNT),
        mem::read::<i32>(player + TOB_ANIM_CAMERA_STATE),
        add,
        mem::read::<u8>(player + TOB_VIEW_FLAGS).map(|f| format!("{f:#04x}")),
        entity.and_then(|e| handle(e + TOB_ATTACH_ENTITY)),
        entity.and_then(|e| handle(e + TOB_CAMERA_ENTITY)),
        cameras,
        entity.and_then(|e| mem::read::<usize>(e + TOB_SPRING_CAMERA)).is_some_and(|p| p != 0),
    );
    let mut last = PROBED_PLAYER.locked();
    if *last != now && PROBED_PLAYER_LINES.fetch_add(1, Relaxed) < 400 {
        let (dy, dp) = *LATEST_AIM.locked();
        log!("build {build}: player {player:#x}: {now}; camera minus baked yaw {dy:.1} pitch {dp:.1}");
        *last = now;
    }
}
