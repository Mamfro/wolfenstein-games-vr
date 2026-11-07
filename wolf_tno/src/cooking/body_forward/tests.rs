use super::*;

#[test]
fn a_menu_takes_the_stick_as_a_steady_step() {
    assert_eq!(menu_step(0, -30000, false), (0, -32767), "down, full");
    assert_eq!(menu_step(0, -29999, true), (0, -32767), "the dither's flip is the same step");
    assert_eq!(menu_step(20000, 9000, false), (32767, 0), "its main axis alone");
    assert_eq!(menu_step(0, -13000, false), (0, -32767), "a light push steps");
    assert_eq!(menu_step(5000, -5000, false), (0, 0), "under a third: at rest");
    assert_eq!(menu_step(0, -8000, true), (0, -32767), "held until back under a fifth");
    assert_eq!(menu_step(0, -6000, true), (0, 0), "then let go");
}
