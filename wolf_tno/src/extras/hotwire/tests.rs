use super::*;

/// A head looking `yaw` degrees left of -Z, level.
fn head(yaw: f32) -> HeadPose {
    HeadPose::synthetic(yaw, 0.0, HeadPose::SYNTHETIC_FOV, 0.064)
}

/// `rest` moved `side` metres along `right` and `up` metres up.
fn moved(rest: Vec3, right: Vec3, side: f32, up: f32) -> Vec3 {
    std::array::from_fn(|k| rest[k] + side * right[k] + if k == 1 { up } else { 0.0 })
}

#[test]
fn the_level_right_is_square_to_the_heading_whichever_way_the_head_faces() {
    for yaw in [0.0, 90.0, -135.0] {
        let head = head(yaw);
        let right = level_right(&head).unwrap();
        let forward = forward_axis(head.orientation);
        assert!(right[1].abs() < 1e-6, "yaw {yaw}: level");
        assert!((right[0] * forward[0] + right[2] * forward[2]).abs() < 1e-4, "yaw {yaw}: square to the heading");
        assert!((right[0].hypot(right[2]) - 1.0).abs() < 1e-4, "yaw {yaw}: unit length");
    }
    let ahead = level_right(&head(0.0)).unwrap();
    assert!((ahead[0] - 1.0).abs() < 1e-4, "facing -Z, right is +X: {ahead:?}");
}

#[test]
fn hands_at_rest_centre_both_wires() {
    let rest = [[-0.15, 1.2, -0.4], [0.15, 1.2, -0.4]];
    assert_eq!(sticks(rest, [1.0, 0.0, 0.0], rest), [0; 4]);
}

#[test]
fn each_controller_moves_only_its_own_hand_right_and_up_positive() {
    for yaw in [0.0, 90.0, -135.0] {
        let right = level_right(&head(yaw)).unwrap();
        let rest = [[-0.15, 1.2, -0.4], [0.15, 1.2, -0.4]];
        let left_moved = [moved(rest[0], right, THROW / 2.0, -THROW), rest[1]];
        assert_eq!(sticks(rest, right, left_moved), [64, -127, 0, 0], "yaw {yaw}: left half right, all the way down");
        let right_moved = [rest[0], moved(rest[1], right, -THROW, THROW / 2.0)];
        assert_eq!(sticks(rest, right, right_moved), [0, 0, -127, 64], "yaw {yaw}: right all the way left, half up");
    }
}

#[test]
fn past_the_throw_holds_the_end_of_the_range() {
    let rest = [[0.0; 3], [0.0; 3]];
    let far = [[1.0, 1.0, 0.0], [-1.0, -1.0, 0.0]];
    assert_eq!(sticks(rest, [1.0, 0.0, 0.0], far), [127, 127, -127, -127]);
}

#[test]
fn moving_toward_or_away_from_the_head_moves_no_wire() {
    let right = level_right(&head(0.0)).unwrap();
    let rest = [[-0.15, 1.2, -0.4], [0.15, 1.2, -0.4]];
    let pushed = [[-0.15, 1.2, -0.6], [0.15, 1.2, -0.2]];
    assert_eq!(sticks(rest, right, pushed), [0; 4]);
}
