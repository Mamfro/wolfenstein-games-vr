use super::*;
use crate::providers::eye_site::tests::EAST;

#[test]
fn the_dominant_eye_sits_at_the_game_camera() {
    let mut view = EAST;
    put_dominant_eye(&mut view, DominantEye::Right, 1.6);
    // The engine puts the right eye at origin - left * separation: the game camera.
    let right_eye: Vec<f32> = (0..3).map(|k| view[k] - 1.6 * view[6 + k]).collect();
    assert_eq!(right_eye, EAST[..3].to_vec());
    let mut view = EAST;
    put_dominant_eye(&mut view, DominantEye::Left, 1.6);
    let left_eye: Vec<f32> = (0..3).map(|k| view[k] + 1.6 * view[6 + k]).collect();
    assert_eq!(left_eye, EAST[..3].to_vec());
    let mut view = EAST;
    put_dominant_eye(&mut view, DominantEye::None, 1.6);
    assert_eq!(view, EAST);
}

#[test]
fn aiming_down_sights_is_read_from_the_zoom() {
    assert_eq!(sighting(80.0, 80.0), 0.0);
    assert_eq!(sighting(79.999985, 80.0), 0.0, "rounding at rest is not sighting");
    assert_eq!(sighting(95.0, 80.0), 0.0, "wider (sprinting) is not sighting");
    assert!((sighting(77.6, 80.0) - 0.4222).abs() < 1e-3, "the first zoom frame is part way");
    assert_eq!(sighting(65.0, 80.0), 1.0);
    assert_eq!(sighting(f32::NAN, 80.0), 0.0);
    assert_eq!(sighting(65.0, 0.0), 0.0);
}

#[test]
fn the_left_trigger_zooms_only_on_foot_with_one_gun() {
    assert!(zooms(false, false));
    assert!(!zooms(true, false), "two guns: it fires the left one");
    assert!(!zooms(false, true), "a mounted gun, the mech or a car: theirs");
}
