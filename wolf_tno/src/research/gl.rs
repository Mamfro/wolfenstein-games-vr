//! Probe (`probe_gl=1`): the viewport and scissor calls of a few whole frames, with the draw
//! framebuffer bound and the engine call site of each, to find where the HUD is drawn. The engine
//! sets both through its import table (one wrapper each), so these import hooks see every call.

use monaka_hook::module::Module;
use monaka_hook::{Hooks, InFlight, Original};
use monaka_producer::{Rejection, log};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::*};
use windows::Win32::Graphics::OpenGL::glGetIntegerv;
use windows::Win32::System::Diagnostics::Debug::RtlCaptureStackBackTrace;
use monaka_core::sync::Locked;

type RectFn = unsafe extern "system" fn(i32, i32, i32, i32);
static VIEWPORT_ORIGINAL: Original<RectFn> = Original::new();
static SCISSOR_ORIGINAL: Original<RectFn> = Original::new();

const GL_DRAW_FRAMEBUFFER_BINDING: u32 = 0x8CA6;
/// Frames to record, starting this many frames after attaching.
const FIRST_FRAME: u64 = 120;
const FRAMES: u64 = 2;

static RECORDING: AtomicBool = AtomicBool::new(false);
static EXE_BASE: AtomicUsize = AtomicUsize::new(0);
static LINES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// The import hooks on glViewport and glScissor, beside the adapter's.
pub fn install(hooks: &mut Hooks, exe: &Module) -> Result<(), Rejection> {
    if !super::options().gl {
        return Ok(());
    }
    EXE_BASE.store(exe.base(), Relaxed);
    // SAFETY: the import slots hold OpenGL's glViewport and glScissor, of the detours' type.
    unsafe {
        hooks.import(&VIEWPORT_ORIGINAL, "glViewport", exe, "OPENGL32.dll", "glViewport", viewport)?;
        hooks.import(&SCISSOR_ORIGINAL, "glScissor", exe, "OPENGL32.dll", "glScissor", scissor)?;
    }
    Ok(())
}

/// At each SwapBuffers (render thread): starts and ends the recorded frames.
pub fn frame(frame: u64) {
    if EXE_BASE.load(Relaxed) == 0 {
        return;
    }
    // Gameplay frames only: wait until views are being built, then record the next few.
    static START: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    if START.load(Relaxed) == 0 && frame >= FIRST_FRAME && crate::providers::eye_site::builds() > 0 {
        START.store(frame + 1, Relaxed);
    }
    let start = START.load(Relaxed);
    let recording = start != 0 && (start..start + FRAMES).contains(&frame);
    if RECORDING.swap(recording, AcqRel) {
        let lines = std::mem::take(&mut *LINES.locked());
        log!("probe_gl: frame {} had {} viewport/scissor calls", frame - 1, lines.len());
        for line in lines {
            log!("  {line}");
        }
    }
}

fn record(kind: &str, x: i32, y: i32, w: i32, h: i32) {
    let mut framebuffer = 0i32;
    // SAFETY: a state query on the render thread's current context.
    unsafe { glGetIntegerv(GL_DRAW_FRAMEBUFFER_BINDING, &mut framebuffer) };
    let base = EXE_BASE.load(Relaxed);
    let mut frames = [std::ptr::null_mut(); 24];
    // SAFETY: fills our buffer with return addresses.
    let captured = unsafe { RtlCaptureStackBackTrace(0, &mut frames, None) } as usize;
    // The first three return addresses inside the game (the viewport goes through a wrapper).
    let sites: Vec<String> = frames[..captured.min(frames.len())]
        .iter()
        .map(|&f| (f as usize).wrapping_sub(base))
        .filter(|&rva| rva < 0x0220_0000)
        .take(3)
        .map(|rva| format!("{rva:#x}"))
        .collect();
    LINES.locked().push(format!("{kind} {x} {y} {w} {h} fb {framebuffer} from {}", sites.join(" < ")));
}

unsafe extern "system" fn viewport(x: i32, y: i32, w: i32, h: i32) {
    let flight = InFlight::enter();
    if RECORDING.load(Acquire) {
        record("viewport", x, y, w, h);
    }
    // SAFETY: forwards the game's own call.
    unsafe { VIEWPORT_ORIGINAL.get(&flight)(x, y, w, h) }
}

unsafe extern "system" fn scissor(x: i32, y: i32, w: i32, h: i32) {
    let flight = InFlight::enter();
    if RECORDING.load(Acquire) {
        record("scissor", x, y, w, h);
    }
    // SAFETY: forwards the game's own call.
    unsafe { SCISSOR_ORIGINAL.get(&flight)(x, y, w, h) }
}
