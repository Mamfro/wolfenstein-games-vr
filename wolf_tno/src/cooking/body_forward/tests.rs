use super::*;

#[test]
fn a_stick_given_on_a_heading_turns_onto_the_one_wanted() {
    // Given a quarter turn left of the heading wanted: forward becomes the given heading's right.
    let (x, y) = turned_stick(0, 32767, std::f32::consts::FRAC_PI_2);
    assert!((x - 32767).abs() <= 1 && y.abs() <= 1, "{x} {y}");
    assert_eq!(turned_stick(1200, -3000, 0.0), (1200, -3000), "on it already: as pushed");
}

#[test]
fn the_run_goes_halfway_between_the_head_and_the_body() {
    // The head a quarter turn left: forward goes an eighth of a turn right of where it looks.
    let (x, y) = turned_stick(0, 32767, std::f32::consts::FRAC_PI_2 * HEAD_SHARE_OUT);
    let angle = f32::from(x).atan2(f32::from(y)).to_degrees();
    assert!((angle - 45.0).abs() < 0.1, "{angle}");
}
