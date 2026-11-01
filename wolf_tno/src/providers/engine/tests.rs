use super::*;

#[test]
fn each_build_has_its_own_executable_and_fingerprint() {
    for (i, build) in ROWS.iter().enumerate() {
        assert_eq!(build.sha256.len(), 64);
        assert_eq!(build.layout_left_right.1, "leftRightStereo");
        assert!(ROWS[..i].iter().all(|earlier| !earlier.exe.eq_ignore_ascii_case(build.exe)), "{} twice", build.exe);
    }
}
