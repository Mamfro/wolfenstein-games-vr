//! Frames in step with the headset, for an id Tech game's frame pacing: where on the headset's
//! refresh grid the next frame starts ([`next_slot`]), where its pairs land in a refresh
//! ([`Phase`]), and how many refreshes a frame needs, from the game's own rate
//! ([`refreshes_per_frame`]) or from how long its frames take ([`Refreshes`]).

/// The headset's refresh grid as the viewer sends it: how long a refresh is, and when the viewer
/// last woke for one (performance-counter nanoseconds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Grid {
    pub period_ns: u64,
    pub woke_ns: u64,
}

/// The most refreshes a frame is held for.
pub const MOST_REFRESHES: u64 = 4;
/// A rate is a fraction of the refresh when within this share of one.
const FITS: f64 = 0.005;
/// How far the landing point moves toward halfway each pair (a share of how far off it is).
const PHASE_GAIN: f64 = 0.05;

/// How many refreshes of `period` nanoseconds a frame at `hz` takes, when that is whole.
pub fn refreshes_per_frame(period: u64, hz: i32) -> Option<u64> {
    if period == 0 || hz <= 0 {
        return None;
    }
    let refreshes = 1e9 / period as f64 / f64::from(hz);
    let whole = refreshes.round();
    (whole >= 1.0 && whole <= MOST_REFRESHES as f64 && (refreshes - whole).abs() <= FITS * whole).then_some(whole as u64)
}

/// `value` brought into `[-half, half)` of a cycle of `period`.
fn wrapped(value: i64, period: u64) -> i64 {
    let p = period as i64;
    (value + p / 2).rem_euclid(p) - p / 2
}

/// The slot the next frame starts at on `grid` (its points moved by `phase`): the grid point
/// nearest `refreshes` refreshes after the `last` slot; or, when that is past `now` by half a
/// refresh or more (or there is no last slot), the first point from `now` on. Whether it came late
/// (the frame before ran over).
pub fn next_slot(grid: &Grid, phase: u64, last: Option<u64>, refreshes: u64, now: u64) -> (u64, bool) {
    let period = grid.period_ns;
    let origin = grid.woke_ns + phase % period;
    let point_at_or_after = |t: u64| {
        let n = t.saturating_sub(origin).div_ceil(period);
        origin + n * period
    };
    let Some(last) = last else { return (point_at_or_after(now), false) };
    let wanted = last + refreshes * period;
    // The grid point nearest the slot wanted (the grid may have slid a little against it).
    let nearest = point_at_or_after(wanted.saturating_sub(period / 2));
    if nearest + period / 2 <= now {
        return (point_at_or_after(now), true);
    }
    (nearest, false)
}

/// Where the paced frames' pairs land: an offset of the grid's points, moved each pair so its
/// publish lands halfway between two of the viewer's wakes.
pub struct Phase;

impl Phase {
    /// The offset after a pair published at `published`, from `phase`, on `grid`.
    pub fn after(grid: &Grid, phase: u64, published: u64) -> u64 {
        let period = grid.period_ns;
        let moved = phase as i64 - (Self::off(grid, published) as f64 * PHASE_GAIN) as i64;
        moved.rem_euclid(period as i64) as u64
    }

    /// How far a pair published at `published` lands from halfway (nanoseconds, signed).
    pub fn off(grid: &Grid, published: u64) -> i64 {
        wrapped(published as i64 - grid.woke_ns as i64 - grid.period_ns as i64 / 2, grid.period_ns)
    }
}

/// How many refreshes a frame gets, for a game without a rate of its own: the fewest its frames'
/// work fits in, measured over windows of [`Refreshes::WINDOW`] frames (the longest frame of each).
/// More at once when a window needs more; fewer only after [`Refreshes::SETTLE`] windows in a row
/// all fit in fewer, so a passing spike does not halve the rate back and forth.
#[derive(Clone, Copy, Debug)]
pub struct Refreshes {
    now: u64,
    longest: u64,
    frames: u32,
    fewer: u32,
}

impl Refreshes {
    /// Frames a window measures.
    pub const WINDOW: u32 = 120;
    /// Windows in a row that must fit in fewer refreshes before a frame gets fewer.
    pub const SETTLE: u32 = 3;
    /// The share of a refresh left spare when a frame's work is fitted into refreshes.
    pub const MARGIN: f64 = 0.08;

    pub const fn new() -> Self {
        Self { now: 1, longest: 0, frames: 0, fewer: 0 }
    }

    /// Refreshes a frame gets now.
    pub fn now(&self) -> u64 {
        self.now
    }

    /// A frame's work took `work_ns` on a grid of `period_ns` refreshes: Some(new count) when the
    /// count changes.
    pub fn note(&mut self, work_ns: u64, period_ns: u64) -> Option<u64> {
        self.longest = self.longest.max(work_ns);
        self.frames += 1;
        if self.frames < Self::WINDOW || period_ns == 0 {
            return None;
        }
        let usable = period_ns as f64 * (1.0 - Self::MARGIN);
        let needed = ((self.longest as f64 / usable).ceil() as u64).clamp(1, MOST_REFRESHES);
        self.longest = 0;
        self.frames = 0;
        if needed > self.now {
            self.fewer = 0;
            self.now = needed;
            return Some(needed);
        }
        if needed < self.now {
            self.fewer += 1;
            if self.fewer >= Self::SETTLE {
                self.fewer = 0;
                self.now -= 1;
                return Some(self.now);
            }
        } else {
            self.fewer = 0;
        }
        None
    }
}

impl Default for Refreshes {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
