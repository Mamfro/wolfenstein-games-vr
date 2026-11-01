//! The player's render view at the view builder, as id Tech 5 and the Void engine keep it: an
//! origin (world units) and axis rows (forward, left, up), converted to and from the canonical
//! camera ([`camera`], [`origin_axis`]) and checked before it is written ([`plausible`]); and the
//! builder's player views, each active one's render view found ([`Players::active`]) and the
//! game's own put back after the call ([`Saved`]). Each game supplies where its build keeps them.

use monaka_core::camera::{Mat34, all_finite, plausible_camera, plausible_fov};
use monaka_core::convention::CameraConvention;
use monaka_hook::mem;

/// The canonical camera of a render view's `origin` and `axis` rows (forward, left, up).
pub fn camera(convention: &CameraConvention, origin: &[f32; 3], axis: &[f32; 9]) -> Option<Mat34> {
    let mut packed = [0.0f32; 16];
    for k in 0..3 {
        packed[4 * k..4 * k + 3].copy_from_slice(&axis[3 * k..3 * k + 3]);
    }
    packed[12..15].copy_from_slice(origin);
    packed[15] = 1.0;
    convention.to_canonical(&packed)
}

/// A canonical camera as a render view's origin and axis rows (forward, left, up); none when they
/// are not finite.
pub fn origin_axis(convention: &CameraConvention, camera: &Mat34) -> Option<([f32; 3], [f32; 9])> {
    let mut packed = [0.0f32; 16];
    if !convention.from_canonical(camera, &mut packed) {
        return None;
    }
    let origin = [packed[12], packed[13], packed[14]];
    let mut axis = [0.0f32; 9];
    for k in 0..3 {
        axis[3 * k..3 * k + 3].copy_from_slice(&packed[4 * k..4 * k + 3]);
    }
    (all_finite(&origin) && all_finite(&axis)).then_some((origin, axis))
}

/// Whether a render view about to be written is one to draw from: its camera
/// ([`plausible_camera`]) and its full field-of-view angles in radians ([`plausible_fov`]).
pub fn plausible(convention: &CameraConvention, origin: &[f32; 3], axis: &[f32; 9], fov: [f32; 2]) -> bool {
    camera(convention, origin, axis).is_some_and(|m| plausible_camera(&m)) && plausible_fov(fov)
}

/// Where the view builder's player views are: the first at `first`, the next `stride` bytes on,
/// each with a byte that is not 0 while the player is active (`active` bytes in) and its render
/// view (`render_view` bytes in).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Players {
    pub first: usize,
    pub stride: usize,
    pub active: usize,
    pub render_view: usize,
}

impl Players {
    /// The active players of the first `count`: each one's number and the address of its render view.
    pub fn active(self, count: usize) -> impl Iterator<Item = (usize, usize)> {
        (0..count).filter_map(move |player| {
            let base = self.first + player * self.stride;
            let active = mem::read::<u8>(base + self.active).is_some_and(|active| active != 0);
            active.then_some((player, base + self.render_view))
        })
    }
}

/// A render view in a game's own form, written at the address of one.
pub trait RenderView: Copy {
    fn write(&self, view: usize) -> bool;
}

/// The render views written for this call of the builder, each with the game's own as read, put
/// back after the call ([`Saved::put_back`]). `N`: the builder's player views.
pub struct Saved<V, const N: usize> {
    views: [Option<(usize, V)>; N],
}

impl<V: RenderView, const N: usize> Saved<V, N> {
    pub fn new() -> Self {
        Self { views: [None; N] }
    }

    /// Writes `drawn` at `view`, player `player`'s render view, keeping `own` (the game's, as read)
    /// to put back once it is written. Whether it was.
    pub fn write(&mut self, player: usize, view: usize, drawn: &V, own: V) -> bool {
        let Some(slot) = self.views.get_mut(player) else { return false };
        let written = drawn.write(view);
        if written {
            *slot = Some((view, own));
        }
        written
    }

    /// Whether any render view was written.
    pub fn any(&self) -> bool {
        self.views.iter().any(Option::is_some)
    }

    /// The game's own render views back where they were written.
    pub fn put_back(&self) {
        for (view, own) in self.views.iter().flatten() {
            own.write(*view);
        }
    }
}

impl<V: RenderView, const N: usize> Default for Saved<V, N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
