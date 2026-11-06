//! Provider: the eye size (`monaka_stereo::eye_size`, its [`Resizes`]): the game's window made the
//! size of both eyes side by side (or of the one eye without stereo), borderless and past the
//! desktop's edges where need be. Windowed, the engine renders at its window's client size and
//! follows its resizes (`r_windowWidth` and `r_windowHeight` track `WM_SIZE`), so each eye is drawn
//! at the headset's size instead of half the monitor's. In full screen the engine keeps the display
//! mode's size, so the game is made windowed first ([`leave_full_screen`]), as `vid_restart` does:
//! `r_fullscreen`'s integer 0, then the render thread's restart flag, the resize waiting for it.
//! The cvar's text stays, and that is what the game saves, so its config keeps the player's mode
//! however the run ends. The window's style and place, and full screen, go back at the stop.
//!
//! With an upscaler ([`arm_render`]: FSR with frame warping) the window is made the render
//! size instead: each eye drawn smaller, the upscaler bringing it to the eye size asked for. With
//! frame warping (one eye a frame) the window still holds two render-size eyes side by side: the
//! warped eye fills its left half, the menus' side-by-side pair the whole.
//!
//! The window is the one the render thread presents to (`SwapBuffers`' device context: [`seen`]).
//! Its owner is the game's main thread, which may be waiting on the render thread: each change is
//! made from a thread of its own, so a present never waits on the window's messages.

use crate::providers::engine::{RENDER_THREAD, RESTART_PENDING, Window};
use crate::{Facts, Wolf};
use monaka_core::sync::Locked;
use monaka_hook::mem;
use monaka_hook::module::Module;
use monaka_producer::game::{Component, Lack, Start};
use monaka_producer::log;
use monaka_stereo::eye_size::{self, Ask, Resizes};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering::*};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{HDC, WindowFromDC};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetWindowLongPtrW, GetWindowRect, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOOWNERZORDER, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, WS_CAPTION, WS_MAXIMIZEBOX,
    WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
};

/// The eye size ready-made, the window's facts kept at its install.
pub const EYE_SIZE: Component<Wolf> = Component { install, ..eye_size::eye_size::<Wolf>() };

/// The window presented to (0: not seen yet).
static WINDOW: AtomicIsize = AtomicIsize::new(0);
/// Both eyes side by side in the window (else the one eye fills it).
static SIDE_BY_SIDE: AtomicBool = AtomicBool::new(false);
/// The build's window facts, with its executable; none where not mapped.
static ENGINE: Mutex<Option<(Module, &'static Window)>> = Mutex::new(None);
/// `r_fullscreen` before the game was made windowed, for the stop; none when it was windowed.
static FULL_SCREEN_KEPT: Mutex<Option<i32>> = Mutex::new(None);
/// An upscaler's render size and the eye size it upscales to ([`arm_render`]).
static RENDER: Mutex<Option<((u32, u32), (u32, u32))>> = Mutex::new(None);
/// The window's style and place before it was resized, for the stop.
static BEFORE: Mutex<Option<Before>> = Mutex::new(None);

#[derive(Clone, Copy)]
struct Before {
    window: isize,
    style: isize,
    rect: RECT,
}

fn install(start: &mut Start<'_, Wolf>) -> Result<(), Lack> {
    let Facts { exe, build } = *start.build;
    *ENGINE.locked() = build.window.as_ref().map(|window| (exe, window));
    SIDE_BY_SIDE.store(crate::eyes::stereo_on(start.options), Release);
    eye_size::want(start)
}

/// The render thread's present to `dc` (`SwapBuffers`): the window it goes to.
pub fn seen(dc: HDC) {
    if WINDOW.load(Relaxed) == 0 {
        // SAFETY: a plain query on the game's own device context.
        let window = unsafe { WindowFromDC(dc) };
        WINDOW.store(window.0 as isize, Relaxed);
    }
}

/// Whether a frame of `width` x `height` may be published (the same-frame loop's `ready`): the
/// eyes in it at the size asked for (with an upscaler, at its render size), or the switch given up.
pub fn ready(width: u32, height: u32) -> bool {
    let eye = eye_of((width, height));
    let presented = match *RENDER.locked() {
        Some((render, display)) if eye == render => display,
        _ => eye,
    };
    let open = eye_size::ready::<Wolf>(presented);
    if open {
        keep_size(eye);
    }
    open
}

/// The eye size asked for ([`Resizes::ask`]), once asked.
static ASKED: Mutex<Option<(u32, u32)>> = Mutex::new(None);
/// When the window was last asked for its size again, and how often.
static REASKED: Mutex<(Option<Instant>, u64)> = Mutex::new((None, 0));
/// The window is asked for its size again at most this often.
const REASK_EVERY: Duration = Duration::from_secs(3);
/// Times logged at most.
const REASKS_LOGGED: u64 = 10;

/// After the switch: the game going back to a size of its own (its video settings applied again as
/// a level loads), the window asked for the eye size again.
fn keep_size(eye: (u32, u32)) {
    let Some(asked) = *ASKED.locked() else { return };
    if eye == drawn_eye(asked) {
        return;
    }
    let mut reasked = REASKED.locked();
    if reasked.0.is_some_and(|at| at.elapsed() < REASK_EVERY) {
        return;
    }
    reasked.0 = Some(Instant::now());
    if matches!(<Wolf as Resizes>::ask(asked, eye), Ask::Asked) {
        reasked.1 += 1;
        if reasked.1 <= REASKS_LOGGED {
            let want = drawn_eye(asked);
            log!("eye size: the game drew {}x{} eyes again; asked for {}x{} once more", eye.0, eye.1, want.0, want.1);
        }
    }
}

/// An upscaler drawing eyes of `render` for eyes of `display`: the window is made the render size.
pub fn arm_render(render: (u32, u32), display: (u32, u32)) {
    *RENDER.locked() = Some((render, display));
}

/// The size each eye is drawn at for eyes of `eye`: with an upscaler, its render size.
fn drawn_eye(eye: (u32, u32)) -> (u32, u32) {
    match *RENDER.locked() {
        Some((render, display)) if display == eye => render,
        _ => eye,
    }
}

/// An eye's size in a frame of `frame`.
fn eye_of(frame: (u32, u32)) -> (u32, u32) {
    if SIDE_BY_SIDE.load(Acquire) { (frame.0 / 2, frame.1) } else { frame }
}

/// The window's client size for eyes of `eye`.
pub fn window_for(eye: (u32, u32), side_by_side: bool) -> (u32, u32) {
    if side_by_side { (eye.0 * 2, eye.1) } else { eye }
}

impl Resizes for Wolf {
    fn ask(eye: (u32, u32), _presented: (u32, u32)) -> Ask {
        let Some((exe, engine)) = *ENGINE.locked() else { return Ask::Cannot("r_fullscreen is not mapped for this game".to_owned()) };
        let cvar = &engine.fullscreen;
        let Some(full_screen) = cvar.present(&exe).then(|| cvar.int(&exe)).flatten() else {
            return Ask::Cannot("r_fullscreen is not where this build keeps it".to_owned());
        };
        let window = WINDOW.load(Relaxed);
        if window == 0 {
            return Ask::NotYet;
        }
        let restart = match full_screen {
            0 => None,
            _ => match leave_full_screen(&exe, engine) {
                Some(flag) => Some(flag),
                None => return Ask::Cannot("the game is in full screen and its render thread is out of reach: set the game's display mode to windowed".to_owned()),
            },
        };
        *ASKED.locked() = Some(eye);
        let (width, height) = window_for(drawn_eye(eye), SIDE_BY_SIDE.load(Acquire));
        std::thread::spawn(move || {
            if restart.is_none_or(restarted) {
                resize(window, width, height);
            }
        });
        Ask::Asked
    }

    fn put_back() {
        *ASKED.locked() = None;
        let before = BEFORE.locked().take();
        let full_screen = FULL_SCREEN_KEPT.locked().take();
        if before.is_some() || full_screen.is_some() {
            std::thread::spawn(move || {
                if let Some(before) = before {
                    restore(before);
                }
                if let Some(value) = full_screen {
                    back_to_full_screen(value);
                }
            });
        }
    }
}

/// How often the wait for the render thread's restart looks at its flag.
const RESTART_LOOK_EVERY: Duration = Duration::from_millis(50);

/// The render thread's restart flag ([`RESTART_PENDING`]), where this build keeps it.
fn restart_flag(exe: &Module, engine: &Window) -> Option<usize> {
    let system = mem::read::<usize>(exe.at(engine.render_system)).filter(|p| *p != 0)?;
    mem::read::<usize>(system).filter(|v| *v == exe.at(engine.render_system_vtable))?;
    let thread = mem::read::<usize>(system + RENDER_THREAD).filter(|p| *p != 0)?;
    Some(thread + RESTART_PENDING)
}

/// The game windowed at its render thread's next frame, its `r_fullscreen` kept for the stop: the
/// restart flag raised, or none where it is out of reach (nothing changed then).
fn leave_full_screen(exe: &Module, engine: &Window) -> Option<usize> {
    let flag = restart_flag(exe, engine)?;
    let value = engine.fullscreen.int(exe)?;
    if !(engine.fullscreen.set_int(exe, 0) && mem::write(flag, 1u8)) {
        return None;
    }
    FULL_SCREEN_KEPT.locked().get_or_insert(value);
    log!("eye size: the game is in full screen; windowed while VR runs, its own setting kept");
    Some(flag)
}

/// Waits for the render thread to lower its restart `flag`, the window then its own to resize:
/// resized sooner, the restart would clamp it back. A video playing holds this until it ends, and
/// the eye size's gate waits with it. Whether the run still wants the size (the stop ends the wait).
fn restarted(flag: usize) -> bool {
    while mem::read::<u8>(flag).is_some_and(|pending| pending != 0) {
        if ASKED.locked().is_none() {
            return false;
        }
        std::thread::sleep(RESTART_LOOK_EVERY);
    }
    ASKED.locked().is_some()
}

/// The game's `r_fullscreen` back, its render thread applying it.
fn back_to_full_screen(value: i32) {
    let Some((exe, engine)) = *ENGINE.locked() else { return };
    let back = engine.fullscreen.set_int(&exe, value) && restart_flag(&exe, engine).is_some_and(|flag| mem::write(flag, 1u8));
    match back {
        true => log!("eye size: the game back in full screen"),
        false => log!("eye size: the game stays windowed: its render thread is out of reach"),
    }
}

/// The window borderless at the desktop's top-left, its client `width` x `height`.
fn resize(window: isize, width: u32, height: u32) {
    let hwnd = HWND(window as _);
    let mut rect = RECT::default();
    // SAFETY: plain queries and changes on the game's own window; its thread handles the messages.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return log!("eye size: the game's window is gone");
        }
        BEFORE.locked().get_or_insert(Before { window, style, rect });
        let frame = (WS_CAPTION | WS_THICKFRAME | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX).0 as isize;
        SetWindowLongPtrW(hwnd, GWL_STYLE, (style & !frame) | WS_POPUP.0 as isize);
        let flags = SWP_FRAMECHANGED | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOACTIVATE;
        match SetWindowPos(hwnd, None, 0, 0, width as i32, height as i32, flags) {
            Ok(()) => log!("eye size: the game's window made {width}x{height}, borderless"),
            Err(e) => log!("eye size: the game's window stays as it is: {e}"),
        }
    }
}

/// The window's style and place as they were.
fn restore(before: Before) {
    let hwnd = HWND(before.window as _);
    let RECT { left, top, right, bottom } = before.rect;
    // SAFETY: as in `resize`; a window already gone fails harmlessly.
    unsafe {
        SetWindowLongPtrW(hwnd, GWL_STYLE, before.style);
        let flags = SWP_FRAMECHANGED | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_NOACTIVATE;
        if SetWindowPos(hwnd, None, left, top, right - left, bottom - top, flags).is_ok() {
            log!("eye size: the game's window back at {}x{}", right - left, bottom - top);
        }
    }
}

#[cfg(test)]
mod tests;
