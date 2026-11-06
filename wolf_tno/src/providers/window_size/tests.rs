use super::*;

#[test]
fn the_window_holds_both_eyes_side_by_side() {
    assert_eq!(window_for((2688, 2688), true), (5376, 2688));
    assert_eq!(window_for((1440, 1600), false), (1440, 1600), "one eye fills it");
}

#[test]
fn the_resize_waits_for_the_restart_and_the_stop_ends_the_wait() {
    use std::sync::atomic::AtomicU8;
    static FLAG: AtomicU8 = AtomicU8::new(1);
    let flag = FLAG.as_ptr() as usize;

    *ASKED.locked() = Some((2996, 2928));
    let waiting = std::thread::spawn(move || restarted(flag));
    std::thread::sleep(RESTART_LOOK_EVERY * 2);
    assert!(!waiting.is_finished(), "held while the render thread has not restarted");
    FLAG.store(0, SeqCst);
    assert!(waiting.join().unwrap(), "restarted, the size still wanted");

    FLAG.store(1, SeqCst);
    let waiting = std::thread::spawn(move || restarted(flag));
    *ASKED.locked() = None;
    assert!(!waiting.join().unwrap(), "the stop ends the wait, the window left alone");
}
