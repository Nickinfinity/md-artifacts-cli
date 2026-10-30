use mda_core::naming::{NameError as E, default_stem, validate_stem};

#[test]
fn stem_rule_in_order() {
    assert_eq!(validate_stem(".hidden"), Err(E::EdgeDot));
    assert_eq!(validate_stem("trail."), Err(E::EdgeDot));
    assert_eq!(validate_stem("CON"), Err(E::Reserved));
    assert_eq!(validate_stem("con.txt"), Err(E::Reserved));
    assert_eq!(validate_stem("a:b"), Err(E::IllegalChar));
    assert_eq!(validate_stem("a*b"), Err(E::IllegalChar));
    assert_eq!(validate_stem("a\"b"), Err(E::IllegalChar));
    assert_eq!(validate_stem(" lead"), Err(E::EdgeSpace));
    assert_eq!(validate_stem("x.md"), Err(E::MdExtension));
    assert_eq!(validate_stem("X.MD"), Err(E::MdExtension));
    assert_eq!(validate_stem("a\u{85}b"), Err(E::ControlChar));
    assert_eq!(validate_stem(""), Err(E::Empty));
    assert_eq!(validate_stem("  "), Err(E::Empty));
    assert_eq!(validate_stem(&"a".repeat(252)), Ok(()));
    assert_eq!(validate_stem(&"a".repeat(253)), Err(E::TooLong));
    assert_eq!(validate_stem("café"), Ok(()));
    assert_eq!(validate_stem("日本語"), Ok(()));
    assert_eq!(validate_stem("a.b"), Ok(()));
}

#[test]
fn first_failure_wins() {
    assert_eq!(validate_stem(".con"), Err(E::EdgeDot));
    assert_eq!(validate_stem("CON.md"), Err(E::Reserved));
    assert_eq!(validate_stem("a:\u{85}"), Err(E::IllegalChar));
}

#[test]
fn default_stem_rule() {
    assert_eq!(default_stem("Button Group"), "button-group");
    assert_eq!(default_stem(""), "untitled");
    assert_eq!(default_stem("日本"), "untitled");
    assert_eq!(default_stem(&"a".repeat(300)), "a".repeat(252));
    assert_eq!(
        default_stem(&format!("{}-b", "a".repeat(251))),
        "a".repeat(251)
    );
}
