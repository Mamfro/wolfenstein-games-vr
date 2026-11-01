//! `probe_panels`: the hands' ammo panels as published, written as BMPs (alpha shown over grey)
//! beside the run's log a few times, to see what the panels hold without a headset.

use monaka_channel::d3d;
use monaka_producer::{Session, log};
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

/// The left eye's layers after which the panels are written (the first ones may come before the
/// boxes settle).
const WRITTEN_AT: [u64; 3] = [120, 600, 1800];

static LAYERS: AtomicU64 = AtomicU64::new(0);

/// After eye `eye`'s 2D layer (its panels just published after the left eye's).
pub fn layer_ended(eye: usize) {
    if !super::options().panels || eye != 0 {
        return;
    }
    let count = LAYERS.fetch_add(1, Relaxed) + 1;
    if !WRITTEN_AT.contains(&count) {
        return;
    }
    let Some(session) = Session::get() else { return };
    monaka_stereo::hud_panels::gl::each_panel(|panel| {
        let path = session.path(&format!("hud-panel-{}-{count}.bmp", panel.piece()));
        // SAFETY: a COM call on the panel's live device.
        let Ok(context) = (unsafe { panel.device().GetImmediateContext() }) else { return };
        match d3d::read_image(panel.device(), &context, panel.out()) {
            Ok((width, height, rgba)) => {
                let bgra: Vec<u8> = rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .flat_map(|p| {
                        let back = (128 * (255 - p[3] as u32) / 255) as u8;
                        [p[2].saturating_add(back), p[1].saturating_add(back), p[0].saturating_add(back), 255]
                    })
                    .collect();
                match std::fs::write(&path, monaka_core::bmp::bgra(width, height, &bgra)) {
                    Ok(()) => log!("HUD panel written to {}", path.display()),
                    Err(e) => log!("HUD panel not written: {e}"),
                }
            }
            Err(e) => log!("HUD panel read-back failed: {e}"),
        }
    });
}
