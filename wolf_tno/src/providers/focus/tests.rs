use super::*;

#[test]
fn the_focus_view_turns_and_moves_with_the_head() {
    // The camera looking along +x; the head 4 units left and turned a quarter left: the trace starts
    // at the head and looks along +y.
    let camera = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    let head = [0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
    let turn = follow_head::turn_between(&camera, &head);
    let (origin, rows) = followed([10.0, 20.0, 30.0], &camera, [0.0, 4.0, 0.0], &turn);
    assert_eq!(origin, [10.0, 24.0, 30.0]);
    assert!(rows.iter().zip(&head).all(|(a, b)| (a - b).abs() < 1e-6), "{rows:?}");
}
