use super::*;

/// Looking down id +X (left +Y, up +Z) from (100, 200, 87).
const ORIGIN: [f32; 3] = [100.0, 200.0, 87.0];
const EAST: [f32; 9] = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];

#[test]
fn render_views_go_to_the_canonical_camera_and_back() {
    let convention = crate::camera_convention(50.0);
    let m = camera(&convention, &ORIGIN, &EAST).unwrap();
    // The world goes canonical with the camera (id -Y right, +Z up, -X back), so the camera
    // looks down -Z; its origin in metres.
    let close = |a: &[f32], b: &[f32]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
    assert!(close(&m, &[1.0, 0.0, 0.0, -4.0, 0.0, 1.0, 0.0, 1.74, 0.0, 0.0, 1.0, -2.0]), "{m:?}");
    assert_eq!(origin_axis(&convention, &m), Some((ORIGIN, EAST)));
    let mut broken = m;
    broken[3] = f32::NAN;
    assert_eq!(origin_axis(&convention, &broken), None);
}

#[test]
fn implausible_views_are_not_written() {
    let convention = crate::camera_convention(50.0);
    assert!(plausible(&convention, &ORIGIN, &EAST, [1.4, 0.9]));
    assert!(!plausible(&convention, &ORIGIN, &EAST, [0.0, 0.9]));
    let mut skewed = EAST;
    skewed[1] = 0.5;
    assert!(!plausible(&convention, &ORIGIN, &skewed, [1.4, 0.9]));
    assert!(!plausible(&convention, &[f32::NAN; 3], &EAST, [1.4, 0.9]));
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Fake(u32);

impl RenderView for Fake {
    fn write(&self, view: usize) -> bool {
        mem::write(view, self.0)
    }
}

#[test]
fn active_players_are_found_and_their_views_put_back() {
    // Two player views 16 bytes apart: the active byte first, the render view at 4.
    let mut memory = [0u32; 8];
    memory[1] = 7;
    memory[4] = 1;
    memory[5] = 9;
    let first = memory.as_mut_ptr() as usize;
    let players = Players { first, stride: 16, active: 0, render_view: 4 };
    let found: Vec<_> = players.active(2).collect();
    assert_eq!(found, [(1, first + 20)], "player 0's byte is 0");
    let mut saved = Saved::<Fake, 2>::new();
    assert!(!saved.any());
    assert!(saved.write(1, first + 20, &Fake(42), Fake(9)));
    assert!(!saved.write(2, first + 20, &Fake(43), Fake(9)), "past the builder's player views");
    assert!(saved.any());
    // SAFETY: reads the array above, which outlives this.
    assert_eq!(unsafe { std::ptr::read_volatile(&memory[5]) }, 42);
    saved.put_back();
    // SAFETY: as above.
    assert_eq!(unsafe { std::ptr::read_volatile(&memory[5]) }, 9);
}
