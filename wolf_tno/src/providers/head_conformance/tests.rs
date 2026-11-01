use super::*;

/// A space turned `degrees` about the up axis and moved `x` metres.
fn space(degrees: f32, x: f32) -> Mat34 {
    let (s, c) = degrees.to_radians().sin_cos();
    [c, 0.0, s, x, 0.0, 1.0, 0.0, 0.0, -s, 0.0, c, 0.0]
}

fn sample(at: f32, degrees: f32, x: f32) -> Sample {
    Sample { at, space: space(degrees, x), camera: space(degrees, x), tilt: 0.0, commands: 0 }
}

#[test]
fn a_steady_turn_and_walk_is_no_jump_however_the_frames_are_spaced() {
    // 90 degrees and 2 m a second; frames 8 ms then 12 ms apart.
    let (d, cm) = jump(&sample(0.0, 0.0, 0.0), &sample(0.008, 0.72, 0.016), &sample(0.020, 1.8, 0.04));
    assert!(d < 0.01 && cm < 0.01, "{d} {cm}");
}

#[test]
fn a_frame_drawn_from_another_pose_is_a_jump() {
    // The middle frame half a degree and a centimetre off the line.
    let (d, cm) = jump(&sample(0.0, 0.0, 0.0), &sample(0.01, 1.0 + 0.5, 0.01), &sample(0.02, 2.0, 0.0));
    assert!((d - 0.5).abs() < 0.02, "{d}");
    assert!((cm - 1.0).abs() < 0.02, "{cm}");
}

#[test]
fn a_head_tilted_right_ear_down_tilts_positive() {
    // Turned about the forward axis (OpenXR: -z), right ear down: the right axis drops.
    let half = (-20f32).to_radians() / 2.0;
    let q = [0.0, 0.0, half.sin(), half.cos()];
    assert!((tilt(q) - 20.0).abs() < 0.01, "{}", tilt(q));
}
