use super::*;

#[test]
fn a_trigger_held_and_let_go_is_one_throw() {
    let mut trigger = Trigger::default();
    assert!(!trigger.step(40), "not ready yet");
    assert!(!trigger.step(200), "ready");
    assert!(!trigger.step(100), "between: still held");
    assert!(trigger.step(10), "let go");
    assert!(!trigger.step(0), "once");
}

#[test]
fn a_light_touch_is_no_throw() {
    let mut trigger = Trigger::default();
    assert!(!trigger.step(100));
    assert!(!trigger.step(0));
}

/// The knife moving along +x at `speed` units a second, a sample every 11 ms up to `until`.
fn moving(speed: f32, until: u64) -> Vec<Sample> {
    (0..40).map(|i| until.saturating_sub(440) + i * 11).map(|at| Sample { at, position: [speed * at as f32 / 1000.0, 0.0, 30.0] }).collect()
}

#[test]
fn a_fast_swing_is_read_with_its_way_and_speed() {
    // 4 m/s at 50 units a metre.
    let (speed, way) = swing(&moving(200.0, 1000), 1000, 50.0).unwrap();
    assert!((speed - 200.0).abs() < 1.0, "{speed}");
    assert!((way[0] - 1.0).abs() < 1e-4, "{way:?}");
}

#[test]
fn a_slow_hand_is_no_swing() {
    assert!(swing(&moving(40.0, 1000), 1000, 50.0).is_none(), "under 1.5 m/s");
}

#[test]
fn only_the_moments_before_the_let_go_count() {
    // A swing that ended long before: nothing within the last 300 ms moved.
    let mut samples = moving(200.0, 500);
    let last = samples.last().unwrap().position;
    samples.extend((0..30).map(|i| Sample { at: 600 + i * 11, position: last }));
    assert!(swing(&samples, 930, 50.0).is_none());
}

#[test]
fn a_fire_axis_runs_along_the_swing_level() {
    let axis = fire_axis([0.0, 1.0, 0.0]).unwrap();
    assert_eq!(&axis[..3], &[0.0, 1.0, 0.0]);
    // Left of +y (id: x forward, y left) is -x; up stays up.
    assert!((axis[3] + 1.0).abs() < 1e-6 && (axis[8] - 1.0).abs() < 1e-6, "{axis:?}");
    assert!(fire_axis([0.0, 0.0, 1.0]).is_some(), "straight up still has axes");
}
