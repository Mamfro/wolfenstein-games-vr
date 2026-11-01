use super::*;
use monaka_core::hold::HOLD;
use std::time::Duration;

fn read(held: &mut Held, buttons: u16, at: Instant) -> u16 {
    let mut pad = Gamepad { buttons, ..Gamepad::default() };
    step(held, &mut pad, at);
    pad.buttons
}

#[test]
fn y_held_opens_the_journal_and_tapped_is_the_games() {
    let start = Instant::now();
    let ms = |n: u64| start + Duration::from_millis(n);
    let mut held = Held { y: LongPress::new(button::Y), back_until: None };
    assert_eq!(read(&mut held, button::Y, ms(0)), 0, "held back while it may be a hold");
    assert_eq!(read(&mut held, 0, ms(100)), button::Y, "a tap of Y when let go");
    assert_eq!(read(&mut held, 0, ms(300)), 0);
    assert_eq!(read(&mut held, button::Y, ms(400)), 0);
    let held_long = ms(400) + HOLD;
    assert_eq!(read(&mut held, button::Y, held_long), button::BACK, "held: Back");
    assert_eq!(read(&mut held, button::Y, held_long + TAP), 0, "Back once");
    assert_eq!(read(&mut held, 0, held_long + TAP * 2), 0, "no Y after the hold");
}
