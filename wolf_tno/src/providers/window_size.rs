//! Provider: the game's frame at the eye size (`monaka_stereo::eye_size::Resizes`). While VR runs,
//! The New Order renders in a window of both eyes side by side at the size the run asks for
//! (`eye_size`, each half one eye), not at its own resolution.
//!
//! The engine is asked as its own resolution menu asks it: windowed (`r_fullscreen` 0) at that size
//! (`r_windowWidth`, `r_windowHeight`), then `vid_restart` (`engine::Window`). That belongs on the
//! game's main thread, and the eye-size gate asks at a present, on the render thread: the request is
//! posted to the game's window, whose procedure ours stands in front of, and queued there for the
//! main loop to run between frames. The engine clamps its window to the monitor; ours stretches it
//! back to the size asked for, its client area exactly the frame (a window may be larger than the
//! screen). At the stop the game's own values go back, with `vid_restart` for its own window.

use crate::providers::engine::{QUEUE_COMMAND, QueueCommandFn, Window};
use crate::{Facts, Wolf};
use monaka_contract::manifest::{OWN_EYE_SIZE, Opt, RENDER_SIZE};
use monaka_core::sync::Locked;
use monaka_hook::mem;
use monaka_hook::module::Module;
use monaka_producer::game::{Component, Lack, Start};
use monaka_producer::log;
use monaka_stereo::eye_size::{Ask, Resizes};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::*};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{HDC, WindowFromDC};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    AdjustWindowRectEx, CallWindowProcA, GWL_EXSTYLE, GWL_STYLE, GWLP_WNDPROC, GetWindowLongA, GetWindowLongPtrA, MINMAXINFO, PostMessageA, SMTO_ABORTIFHUNG,
    SMTO_BLOCK, SWP_NOSIZE, SendMessageTimeoutA, SetWindowLongPtrA, WINDOW_EX_STYLE, WINDOW_STYLE, WINDOWPOS, WM_APP, WM_GETMINMAXINFO, WM_WINDOWPOSCHANGING, WNDPROC,
};

/// The New Order's eye size: the game's own resolution unless the player picks one.
const RENDER_SIZE_OWN: Opt = RENDER_SIZE.default_choice("game");

/// The eye size (`monaka_stereo::eye_size`) for The New Order, where the window is mapped.
pub const EYE_SIZE: Component<Wolf> = Component { games: &["wolf_tno"], options: &[RENDER_SIZE_OWN, OWN_EYE_SIZE], install, ..monaka_stereo::eye_size::eye_size() };

/// Our messages to the game's window: apply the size asked for, and put the game's own back.
const RESIZE: u32 = WM_APP + 0x4d5;
const PUT_BACK: u32 = WM_APP + 0x4d6;
/// How long the stop waits for the main thread to put the game's own size back (ms).
const PUT_BACK_WAIT_MS: u32 = 2000;

/// The run's facts for the render and main threads.
#[derive(Clone, Copy)]
struct Setup {
    exe: Module,
    window: Option<&'static Window>,
}

static SETUP: Mutex<Option<Setup>> = Mutex::new(None);
/// Both eyes side by side in each frame (stereo on): the frame is twice the eye's width.
static SIDE_BY_SIDE: AtomicBool = AtomicBool::new(true);
/// The frame size the window is held at (width and height packed; 0: none).
static WANTED: AtomicU64 = AtomicU64::new(0);
/// The device context of the frame presenting, for its window.
static PRESENTING_DC: AtomicUsize = AtomicUsize::new(0);

/// The game's own windowed-mode values, kept while ours stand.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Kept {
    fullscreen: i32,
    width: i32,
    height: i32,
}

static KEPT: Mutex<Option<Kept>> = Mutex::new(None);

fn install(start: &mut Start<'_, Wolf>) -> Result<(), Lack> {
    let Facts { exe, build } = *start.build;
    *SETUP.locked() = Some(Setup { exe, window: build.window.as_ref() });
    SIDE_BY_SIDE.store(crate::eyes::stereo_on(start.options), Release);
    monaka_stereo::eye_size::want(start)
}

/// The frame for an eye of `eye`: both eyes side by side, else the one.
pub fn frame_for(eye: (u32, u32), side_by_side: bool) -> (u32, u32) {
    if side_by_side { (eye.0 * 2, eye.1) } else { eye }
}

/// The eye in a frame of `frame` ([`frame_for`]'s other way).
pub fn eye_of(frame: (u32, u32), side_by_side: bool) -> (u32, u32) {
    if side_by_side { (frame.0 / 2, frame.1) } else { frame }
}

/// The loop's gate (`PairLoop::ready`) for a frame of `width` x `height`: its eye at the eye size,
/// or the game asked for it.
pub fn ready(width: u32, height: u32) -> bool {
    monaka_stereo::eye_size::ready::<Wolf>(eye_of((width, height), SIDE_BY_SIDE.load(Acquire)))
}

/// At the game's `SwapBuffers`, before the loop sees the frame: whose window it presents to.
pub fn presenting(dc: HDC) {
    PRESENTING_DC.store(dc.0 as usize, Relaxed);
}

fn pack(size: (u32, u32)) -> u64 {
    u64::from(size.0) << 32 | u64::from(size.1)
}

fn wanted() -> Option<(i32, i32)> {
    let packed = WANTED.load(Acquire);
    (packed != 0).then(|| ((packed >> 32) as i32, packed as u32 as i32))
}

/// The command system and its method that queues a command, where this build keeps them.
fn queue_slot(exe: &Module, window: &Window) -> Option<(usize, QueueCommandFn)> {
    let system = mem::read::<usize>(exe.at(window.commands)).filter(|p| *p != 0)?;
    let vtable = mem::read::<usize>(system).filter(|v| *v == exe.at(window.commands_vtable))?;
    let function = mem::read::<usize>(vtable + QUEUE_COMMAND).filter(|f| exe.contains(*f))?;
    // SAFETY: the slot of this build's command system vtable, checked above, holds a function of
    // this type.
    Some((system, unsafe { std::mem::transmute::<usize, QueueCommandFn>(function) }))
}

/// Why `window` cannot be used in this build, if it cannot.
fn unusable(exe: &Module, window: &Window) -> Option<String> {
    if queue_slot(exe, window).is_none() {
        return Some("the command system is not where this build keeps it".to_owned());
    }
    [&window.fullscreen, &window.width, &window.height].into_iter().find(|cvar| !cvar.present(exe)).map(|cvar| format!("{} is not where this build keeps it", cvar.name))
}

/// Queues `command` for the main loop, as typed at the console. On the main thread only.
fn queue(exe: &Module, window: &Window, command: &str) -> bool {
    let Some((system, append)) = queue_slot(exe, window) else { return false };
    let text = format!("{command}\n\0");
    // SAFETY: the command system's own method that queues a command, on the game's main thread,
    // with a NUL-terminated text it copies.
    unsafe { append(system, text.as_ptr()) };
    true
}

impl Resizes for Wolf {
    fn ask(eye: (u32, u32), _: (u32, u32)) -> Ask {
        let Some(setup) = *SETUP.locked() else { return Ask::Cannot("not set up".to_owned()) };
        let Some(window) = setup.window else { return Ask::Cannot("this build's window is not mapped".to_owned()) };
        if let Some(why) = unusable(&setup.exe, window) {
            return Ask::Cannot(why);
        }
        // SAFETY: the device context the game presents with, from its SwapBuffers.
        let game_window = unsafe { WindowFromDC(HDC(PRESENTING_DC.load(Relaxed) as *mut _)) };
        if game_window.is_invalid() {
            return Ask::NotYet;
        }
        if !subclass(game_window) {
            return Ask::Cannot("the game window's procedure could not be replaced".to_owned());
        }
        WANTED.store(pack(frame_for(eye, SIDE_BY_SIDE.load(Acquire))), Release);
        // SAFETY: posts our own message to the game's window.
        match unsafe { PostMessageA(Some(game_window), RESIZE, WPARAM(0), LPARAM(0)) } {
            Ok(()) => Ask::Asked,
            Err(e) => {
                WANTED.store(0, Release);
                Ask::Cannot(format!("the game's window took no message: {e}"))
            }
        }
    }

    fn put_back() {
        WANTED.store(0, Release);
        let game_window = HWND(WINDOW.load(Acquire) as *mut _);
        if KEPT.locked().is_some() && !game_window.is_invalid() {
            // SAFETY: sends our own message to the game's window, which ours still stands in front of.
            let answered = unsafe { SendMessageTimeoutA(game_window, PUT_BACK, WPARAM(0), LPARAM(0), SMTO_BLOCK | SMTO_ABORTIFHUNG, PUT_BACK_WAIT_MS, None) };
            if answered.0 == 0 {
                // Its values at least: the game's own size comes back at its next resolution change.
                if let Some(setup) = *SETUP.locked() {
                    restore(&setup, false);
                }
                log!("window size: the game's main thread did not answer in {PUT_BACK_WAIT_MS} ms; its own window size comes back when it next applies one");
            }
        }
        unsubclass();
    }
}

/// On the main thread: windowed at the size wanted, then `vid_restart`, the game's own values kept.
fn resize() {
    let (Some(setup), Some((width, height))) = (*SETUP.locked(), wanted()) else { return };
    let Some(window) = setup.window else { return };
    let exe = setup.exe;
    let mut kept = KEPT.locked();
    if kept.is_none() {
        let read = |cvar: &eng_idt::Cvar| cvar.int(&exe).unwrap_or(0);
        *kept = Some(Kept { fullscreen: read(&window.fullscreen), width: read(&window.width), height: read(&window.height) });
    }
    let set = window.fullscreen.set_int(&exe, 0) && window.width.set_int(&exe, width) && window.height.set_int(&exe, height);
    let queued = set && queue(&exe, window, "vid_restart");
    // SAFETY: the calling thread's id.
    let thread = unsafe { GetCurrentThreadId() };
    match queued {
        true => log!("window size: windowed at {width}x{height}, vid_restart queued (main thread {thread}; the game's own: {:?})", *kept),
        false => log!("window size: the window cvars or the command system could not be written; the game renders at its own size"),
    }
}

/// The game's own values back, with `vid_restart` for its window when `restart` (on the main
/// thread only).
fn restore(setup: &Setup, restart: bool) {
    let Some(window) = setup.window else { return };
    let Some(kept) = KEPT.locked().take() else { return };
    let exe = setup.exe;
    let _ = window.fullscreen.set_int(&exe, kept.fullscreen) && window.width.set_int(&exe, kept.width) && window.height.set_int(&exe, kept.height);
    if restart && queue(&exe, window, "vid_restart") {
        log!("window size: the game's own back ({kept:?}), vid_restart queued");
    }
}

/// The window's outer size for a client area of `width` x `height` with its styles now.
fn outer_size(game_window: HWND, width: i32, height: i32) -> (i32, i32) {
    let mut rect = RECT { left: 0, top: 0, right: width, bottom: height };
    // SAFETY: plain queries on a window of this process and a local rectangle.
    unsafe {
        let style = WINDOW_STYLE(GetWindowLongA(game_window, GWL_STYLE) as u32);
        let extended = WINDOW_EX_STYLE(GetWindowLongA(game_window, GWL_EXSTYLE) as u32);
        if AdjustWindowRectEx(&mut rect, style, false, extended).is_err() {
            return (width, height);
        }
    }
    (rect.right - rect.left, rect.bottom - rect.top)
}

/// The window's own procedure, while ours stands in front of it, and the window.
static ORIGINAL: AtomicUsize = AtomicUsize::new(0);
static WINDOW: AtomicUsize = AtomicUsize::new(0);

unsafe extern "system" fn procedure(game_window: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match message {
        RESIZE => {
            resize();
            return LRESULT(0);
        }
        PUT_BACK => {
            if let Some(setup) = *SETUP.locked() {
                restore(&setup, true);
            }
            return LRESULT(0);
        }
        _ => {}
    }
    // SAFETY: the window's own procedure, as it was before ours (the game's window is ANSI).
    let result = unsafe {
        let original: WNDPROC = std::mem::transmute(ORIGINAL.load(Acquire));
        CallWindowProcA(original, game_window, message, w, l)
    };
    // The window at the size wanted, whatever the game's own clamping and the system's track size
    // made of it: after the game's procedure (and the system's default) have had their say.
    if let Some((width, height)) = wanted() {
        let (outer_width, outer_height) = outer_size(game_window, width, height);
        match message {
            WM_GETMINMAXINFO => {
                // SAFETY: the system's MINMAXINFO for this message.
                let info = unsafe { &mut *(l.0 as *mut MINMAXINFO) };
                info.ptMaxTrackSize.x = info.ptMaxTrackSize.x.max(outer_width);
                info.ptMaxTrackSize.y = info.ptMaxTrackSize.y.max(outer_height);
            }
            WM_WINDOWPOSCHANGING => {
                // SAFETY: the system's WINDOWPOS for this message.
                let position = unsafe { &mut *(l.0 as *mut WINDOWPOS) };
                if (position.flags & SWP_NOSIZE).0 == 0 {
                    (position.cx, position.cy) = (outer_width, outer_height);
                }
            }
            _ => {}
        }
    }
    result
}

/// Puts our procedure in front of the game window's (once).
fn subclass(game_window: HWND) -> bool {
    if ORIGINAL.load(Acquire) != 0 {
        return WINDOW.load(Acquire) == game_window.0 as usize;
    }
    // SAFETY: subclasses a window of this process; ours forwards everything but its own messages.
    let previous = unsafe { SetWindowLongPtrA(game_window, GWLP_WNDPROC, procedure as *const () as usize as _) };
    if previous == 0 {
        return false;
    }
    ORIGINAL.store(previous as usize, Release);
    WINDOW.store(game_window.0 as usize, Release);
    true
}

/// Puts the window's own procedure back (if ours is still the one in front).
fn unsubclass() {
    let original = ORIGINAL.swap(0, AcqRel);
    if original == 0 {
        return;
    }
    let game_window = HWND(WINDOW.swap(0, AcqRel) as *mut _);
    // SAFETY: queries and restores the procedure of a window of this process.
    unsafe {
        if GetWindowLongPtrA(game_window, GWLP_WNDPROC) as usize == procedure as *const () as usize {
            SetWindowLongPtrA(game_window, GWLP_WNDPROC, original as _);
        } else {
            log!("window size: someone subclassed the game window after us; ours stays (it forwards)");
            ORIGINAL.store(original, Release);
            WINDOW.store(game_window.0 as usize, Release);
        }
    }
}

#[cfg(test)]
mod tests;
