use super::*;

#[test]
fn side_by_side_the_frame_is_both_eyes_wide() {
    assert_eq!(frame_for((2064, 2208), true), (4128, 2208));
    assert_eq!(eye_of((4128, 2208), true), (2064, 2208));
    assert_eq!(frame_for((1999, 1600), true), (3998, 1600));
    assert_eq!(eye_of(frame_for((1999, 1600), true), true), (1999, 1600), "an odd eye width comes back");
}

#[test]
fn without_stereo_the_frame_is_the_eye() {
    assert_eq!(frame_for((2064, 2208), false), (2064, 2208));
    assert_eq!(eye_of((1920, 1080), false), (1920, 1080));
}

#[test]
fn the_size_wanted_packs_and_unpacks() {
    WANTED.store(pack((4128, 2208)), Release);
    assert_eq!(wanted(), Some((4128, 2208)));
    WANTED.store(0, Release);
    assert_eq!(wanted(), None);
}

#[test]
fn the_new_order_starts_at_the_games_own_size() {
    let monaka_contract::manifest::Kind::Choice { default, .. } = EYE_SIZE.options[0].kind else { panic!("render_size is a choice") };
    assert_eq!((EYE_SIZE.options[0].id, default), ("render_size", "game"));
    assert_eq!(EYE_SIZE.games, ["wolf_tno"]);
}
