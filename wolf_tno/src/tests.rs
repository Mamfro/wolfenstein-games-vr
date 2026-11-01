#[test]
fn the_manifests_are_the_builds() {
    let manifests: Vec<&str> = super::manifest::MANIFESTS.iter().flat_map(|m| m.executables.iter().copied()).collect();
    let builds: Vec<&str> = super::engine::ROWS.iter().map(|b| b.exe).collect();
    assert_eq!(manifests, builds);
}
