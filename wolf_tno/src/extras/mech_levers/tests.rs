use super::*;

/// A head looking `yaw` degrees left of -Z, level.
fn head(yaw: f32) -> HeadPose {
    HeadPose::synthetic(yaw, 0.0, HeadPose::SYNTHETIC_FOV, 0.064)
}

/// Both hands at `ahead` metres along the head's heading, 0.2 m apart, half a metre below it.
fn hands_ahead(head: &HeadPose, ahead: f32) -> [Vec3; 2] {
    let f = forward_axis(head.orientation);
    let right = [-f[2], 0.0, f[0]];
    let at = |side: f32| std::array::from_fn(|k| head.position[k] + ahead * f[k] + side * 0.1 * right[k] - if k == 1 { 0.5 } else { 0.0 });
    [at(-1.0), at(1.0)]
}

#[test]
fn the_hands_reach_is_along_the_heads_heading_whichever_way_it_faces() {
    for yaw in [0.0, 90.0, -135.0] {
        let head = head(yaw);
        let reach = reach(&head, hands_ahead(&head, 0.4)).unwrap();
        assert!((reach - 0.4).abs() < 1e-4, "yaw {yaw}: {reach}");
    }
}

#[test]
fn pushing_forward_aims_down_and_pulling_back_aims_up_to_the_mechs_limits() {
    let (min, max) = (-30.0, 20.0);
    assert_eq!(pitch_for(0.0, min, max), 0.0);
    assert!((pitch_for(THROW / 2.0, min, max) - 10.0).abs() < 1e-4, "half forward: half way down");
    assert!((pitch_for(-THROW / 2.0, min, max) + 15.0).abs() < 1e-4, "half back: half way up");
    assert_eq!(pitch_for(1.0, min, max), max);
    assert_eq!(pitch_for(-1.0, min, max), min);
}
