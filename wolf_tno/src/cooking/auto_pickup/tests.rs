use super::*;

const PLAYER: [f32; 6] = [-16.0, -16.0, 0.0, 16.0, 16.0, 74.0];

/// A 4-unit box at `(x, y, z)`.
fn at(x: f32, y: f32, z: f32) -> [f32; 6] {
    [x - 2.0, y - 2.0, z - 2.0, x + 2.0, y + 2.0, z + 2.0]
}

#[test]
fn an_item_within_reach_of_the_players_bounds_is_in_reach() {
    assert!(in_reach(&PLAYER, &at(0.0, 0.0, 1.0), REACH), "at the feet");
    assert!(in_reach(&PLAYER, &at(16.0 + REACH, 0.0, 40.0), REACH), "on a table, an arm's length ahead");
    assert!(in_reach(&PLAYER, &at(-30.0, 30.0, 74.0 + REACH - 4.0), REACH), "on a shelf above, to one side");
}

#[test]
fn an_item_past_reach_on_any_axis_is_not() {
    assert!(!in_reach(&PLAYER, &at(16.0 + REACH + 3.0, 0.0, 40.0), REACH), "too far ahead");
    assert!(!in_reach(&PLAYER, &at(0.0, -16.0 - REACH - 3.0, 40.0), REACH), "too far behind");
    assert!(!in_reach(&PLAYER, &at(0.0, 0.0, -REACH - 3.0), REACH), "a floor below");
}

#[test]
fn a_refused_item_waits_its_thinks_then_is_tried_again() {
    let mut refused = Refused::new();
    refused.note(0x1000, 100);
    assert!(refused.waits(0x1000, 100));
    assert!(refused.waits(0x1000, 100 + RETRY_THINKS - 1));
    assert!(!refused.waits(0x1000, 100 + RETRY_THINKS), "tried again");
    assert!(!refused.waits(0x2000, 101), "another item");
}

#[test]
fn the_oldest_refusal_gives_way_when_more_are_noted() {
    let mut refused = Refused::new();
    for i in 0..=REMEMBERED {
        refused.note(0x1000 + i, 5);
    }
    assert!(!refused.waits(0x1000, 5), "written over");
    assert!(refused.waits(0x1000 + REMEMBERED, 5));
}
