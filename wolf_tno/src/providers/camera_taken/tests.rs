use super::*;

/// A view at `origin` looking along `forward` (its other rows left alone).
fn view(origin: [f32; 3], forward: [f32; 3]) -> [f32; 12] {
    [origin[0], origin[1], origin[2], forward[0], forward[1], forward[2], 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]
}

#[test]
fn views_apart_measures_the_origins_and_the_forward_axes() {
    let (units, degrees) = views_apart(&view([0.0; 3], [1.0, 0.0, 0.0]), &view([3.0, 4.0, 0.0], [0.0, 1.0, 0.0]));
    assert!((units - 5.0).abs() < 1e-4 && (degrees - 90.0).abs() < 1e-3, "{units} {degrees}");
}

#[test]
fn the_camera_goes_off_past_the_far_pair_and_back_within_the_near_one() {
    assert!(!away_now(false, 5.0, 5.0), "a little apart: still on (walking, a bob)");
    assert!(away_now(false, 16.0, 2.0), "a grab's camera ahead");
    assert!(away_now(false, 0.0, 31.0), "turned away");
    assert!(away_now(true, 5.0, 5.0), "once off, it stays off until close again");
    assert!(!away_now(true, 1.0, 2.0));
}
