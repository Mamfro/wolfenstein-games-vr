use super::*;
use monaka_contract::protocol::LEFT_HAND;

#[test]
fn synthetic_controllers_sit_ahead_and_point_outward() {
    let head = HeadPose::synthetic(0.0, 0.0, HeadPose::SYNTHETIC_FOV, 0.064);
    let right = synthetic_palm(RIGHT_HAND, &head);
    let left = synthetic_palm(LEFT_HAND, &head);
    assert!(right.position[0] > head.position[0] && left.position[0] < head.position[0]);
    assert!(right.position[2] < head.position[2], "ahead is -z");
    // Forward is -z of the orientation: the right one points right (+x), the left one left.
    let forward = |q: [f32; 4]| {
        let r = monaka_core::camera::rotation(q);
        [-r[2], -r[5], -r[8]]
    };
    assert!(forward(right.orientation)[0] > 0.2 && forward(left.orientation)[0] < -0.2);
    assert!(forward(right.orientation)[1] > 0.0, "a little up");
}
