use super::*;

/// 120 Hz: a refresh every 8,333,333 ns.
const P: u64 = 8_333_333;

fn grid(woke: u64) -> Grid {
    Grid { period_ns: P, woke_ns: woke }
}

#[test]
fn rates_that_divide_the_refresh_take_whole_refreshes() {
    assert_eq!(refreshes_per_frame(P, 120), Some(1));
    assert_eq!(refreshes_per_frame(P, 60), Some(2));
    assert_eq!(refreshes_per_frame(P, 96), None, "1.25 refreshes");
    assert_eq!(refreshes_per_frame(0, 60), None);
}

#[test]
fn frames_go_every_two_refreshes_on_the_grid() {
    let timing = grid(1_000_000_000);
    let (first, late) = next_slot(&timing, 0, None, 2, 1_000_000_000 + 3_000_000);
    assert_eq!((first, late), (1_000_000_000 + P, false), "the next grid point");
    let (second, late) = next_slot(&timing, 0, Some(first), 2, first + 10_000_000);
    assert_eq!((second, late), (first + 2 * P, false));
}

#[test]
fn a_frame_that_ran_over_starts_on_the_next_point() {
    let last = 1_000_000_000;
    let timing = grid(last);
    let (slot, late) = next_slot(&timing, 0, Some(last), 1, last + P + P / 2 + 1_000);
    assert!(late);
    assert_eq!(slot, last + 2 * P);
}

#[test]
fn a_pair_landing_early_moves_the_phase_later() {
    let timing = grid(1_000_000_000);
    // Published a quarter refresh after the wake: a quarter before halfway.
    let phase = Phase::after(&timing, 0, 1_000_000_000 + P / 4);
    assert!(phase > 0 && phase < P / 4, "{phase}");
}

#[test]
fn frames_that_fit_a_refresh_keep_one() {
    let mut r = Refreshes::new();
    for _ in 0..Refreshes::WINDOW * 5 {
        assert_eq!(r.note(6_000_000, P), None);
    }
    assert_eq!(r.now(), 1);
}

#[test]
fn a_window_over_a_refresh_takes_two_at_once_and_settles_back_slowly() {
    let mut r = Refreshes::new();
    let changes: Vec<u64> = (0..Refreshes::WINDOW).filter_map(|i| r.note(if i == 5 { 9_000_000 } else { 6_000_000 }, P)).collect();
    assert_eq!(changes, [2]);
    // Fitting one refresh again: back to one only after the settling windows.
    let back: Vec<(u32, u64)> = (0..Refreshes::WINDOW * Refreshes::SETTLE).filter_map(|i| r.note(6_000_000, P).map(|n| (i, n))).collect();
    assert_eq!(back, [(Refreshes::WINDOW * Refreshes::SETTLE - 1, 1)]);
}
