use super::*;

#[test]
fn a_camera_turned_by_the_head_turn_is_the_head() {
    // The game camera facing id +Y (left); the head facing the same way, tilted a quarter turn
    // about its forward axis (left now down, up now left).
    let camera = [0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
    let head = [0.0, 1.0, 0.0, 0.0, 0.0, -1.0, -1.0, 0.0, 0.0];
    let turn = turn_between(&camera, &head);
    let turned = turned_rows(&camera, &turn);
    assert!(turned.iter().zip(head).all(|(a, b)| (a - b).abs() < 1e-6), "{turned:?}");
    // The same tilt on a view turned elsewhere (the game's view a little off its camera).
    let identity = turn_between(&camera, &camera);
    assert!(turned_rows(&head, &identity).iter().zip(head).all(|(a, b)| (a - b).abs() < 1e-6), "no turn leaves it");
}

#[test]
fn a_view_turned_as_the_model_was_placed_gives_the_models_rows() {
    // The view pitched 30 degrees down, the model kept level: the turn between them takes the view's
    // rows onto the model's, and measures 30 degrees.
    let (s, c) = 30f32.to_radians().sin_cos();
    let view = [c, 0.0, -s, 0.0, 1.0, 0.0, s, 0.0, c];
    let level = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
    let turn = turn_between(&view, &level);
    assert!((turn_degrees(&turn) - 30.0).abs() < 1e-3);
    let rows = turned_rows(&view, &turn);
    assert!(rows.iter().zip(&level).all(|(a, b)| (a - b).abs() < 1e-5), "{rows:?}");
}
