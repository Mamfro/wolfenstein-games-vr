use super::*;

#[test]
fn model_space_round_trips() {
    let (s, c) = 0.4f32.sin_cos();
    let m = Model { origin: [100.0, -50.0, 20.0], axis: [c, s, 0.0, -s, c, 0.0, 0.0, 0.0, 1.0], scale: [1.0; 3] };
    let p = [3.0, -4.0, 5.0];
    assert!(length(sub(m.to_model(m.to_world(p)), p)) < 1e-4);
    let rows = [0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
    let back = m.rows_to_world(&m.rows_to_model(&rows));
    assert!(back.iter().zip(&rows).all(|(a, b)| (a - b).abs() < 1e-5));
}

/// The model's canonical frame takes a model point where `to_world` and the engine's world
/// convention do, scale and units included.
#[test]
fn the_model_in_the_canonical_world_matches_its_world_placement() {
    let (s, c) = 0.4f32.sin_cos();
    let m = Model { origin: [100.0, -50.0, 20.0], axis: [c, s, 0.0, -s, c, 0.0, 0.0, 0.0, 1.0], scale: [1.1, 1.1, 0.9] };
    let units_per_metre = 39.37;
    let frame = m.to_canonical(units_per_metre).unwrap();
    let p = [3.0, -4.0, 5.0];
    let through_frame = monaka_core::camera::position(&compose(&frame, &monaka_core::camera::translation(p)));
    let world = m.to_world(p);
    let expected = eng_idt::camera_convention(units_per_metre).world.canonical(world).map(|v| v / units_per_metre);
    assert!(length(sub(through_frame, expected)) < 1e-5, "{through_frame:?} {expected:?}");
}

#[test]
fn a_joint_reads_its_basis_both_ways() {
    let r = [0.0, -1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0];
    let j = from_rotation(&r, [1.0, 2.0, 3.0]);
    assert_eq!(rotation_from_rows(&basis_rows(&j, Basis::Columns), Basis::Columns), r);
    assert_eq!(rotation_from_rows(&basis_rows(&j, Basis::Rows), Basis::Rows), r);
    assert_eq!([j[3], j[7], j[11]], [1.0, 2.0, 3.0]);
    assert!(monaka_core::camera::is_rigid(&LEFT_GRIP, 0.01), "the measured grip is a rotation");
    assert!(monaka_core::camera::is_rigid(&RIGHT_GRIP, 0.01), "its mirror is one too");
}
