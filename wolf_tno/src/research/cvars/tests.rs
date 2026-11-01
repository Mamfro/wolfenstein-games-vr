use super::parse;

#[test]
fn an_entry_names_the_cvar_its_place_and_value() {
    assert_eq!(parse("r_useLightScissors@0x1d17b20=0"), Some(("r_useLightScissors", 0x1d17b20, 0)));
    assert!(parse("r_useLightScissors=0").is_none(), "no place");
    assert!(parse("x@zz=1").is_none(), "no hex");
}
