use mda_core::naming::{
    ACCEPTED_STEMS, NameError as E, default_stem, slugify, validate_dir_segment, validate_name,
    validate_stem,
};

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
    assert_eq!(default_stem("日本"), "日本");
    assert_eq!(default_stem("!!!"), "untitled");
    let t = default_stem(&"\u{e9}".repeat(300));
    assert!(t.len() <= 252 && t.chars().all(|c| c == '\u{e9}'));
    assert_eq!(default_stem(&"a".repeat(300)), "a".repeat(252));
    assert_eq!(
        default_stem(&format!("{}-b", "a".repeat(251))),
        "a".repeat(251)
    );
}

#[test]
fn slug_unicode() {
    assert_eq!(slugify("Caf\u{e9} notes"), "caf\u{e9}-notes");
    assert_eq!(slugify("Cafe\u{301}"), "caf\u{e9}");
    assert_eq!(slugify("日本語 メモ"), "日本語-メモ");
    assert_eq!(slugify("\u{130}stanbul"), "i\u{307}stanbul");
    assert_eq!(slugify("--A--"), "a");
    assert_eq!(slugify("!!!"), "");
    assert_eq!(slugify("\u{663}"), "\u{663}");
    assert_eq!(slugify("\u{212a}elvin"), "kelvin");
    let long = slugify(&"a".repeat(10_000));
    assert_eq!(long.len(), 1024);
}

#[test]
fn format_chars_and_nfc() {
    for c in [
        "\u{200b}", "\u{200e}", "\u{202e}", "\u{2066}", "\u{feff}", "\u{2028}",
    ] {
        assert_eq!(
            validate_stem(&format!("a{c}b")),
            Err(E::ControlChar),
            "{c:?}"
        );
        assert_eq!(validate_dir_segment(&format!("a{c}b")), Err(E::ControlChar));
        assert_eq!(validate_name(&format!("a{c}b")), Ok(()));
    }
    assert_eq!(validate_stem("a\u{200c}b"), Ok(()));
    assert_eq!(validate_stem("a\u{200d}b"), Ok(()));
    assert_eq!(validate_stem("cafe\u{301}"), Err(E::NotNfc));
    assert_eq!(validate_dir_segment("cafe\u{301}"), Err(E::NotNfc));
    assert_eq!(validate_name("cafe\u{301}.txt"), Ok(()));
    assert_eq!(validate_stem("a\u{200b}:"), Err(E::IllegalChar));
    assert_eq!(validate_stem("a\u{200b}e\u{301}"), Err(E::ControlChar));
}

#[test]
fn reserved_complete() {
    for r in [
        "COM0",
        "lpt9.txt",
        "COM\u{b9}",
        "LPT\u{b3}",
        "CONIN$",
        "conout$.md",
        "CON .txt",
        "nul .md",
    ] {
        assert_eq!(validate_stem(r), Err(E::Reserved), "{r}");
        assert_eq!(validate_name(r), Err(E::Reserved), "{r}");
        assert_eq!(validate_dir_segment(r), Err(E::Reserved), "{r}");
    }
    for r in ["COM10", "CONX", "con-x", "CON-x"] {
        assert_eq!(validate_stem(r), Ok(()), "{r}");
    }
    assert_eq!(validate_stem("CON "), Err(E::EdgeSpace));
    assert_eq!(validate_stem("CON."), Err(E::EdgeDot));
}

#[test]
fn dir_segment_rules() {
    assert_eq!(validate_dir_segment(".hidden"), Ok(()));
    assert_eq!(validate_dir_segment("x.md"), Ok(()));
    assert_eq!(validate_dir_segment("trail."), Err(E::EdgeDot));
    assert_eq!(validate_dir_segment(" a"), Err(E::EdgeSpace));
    assert_eq!(validate_dir_segment(""), Err(E::Empty));
    assert_eq!(validate_dir_segment("a|b"), Err(E::IllegalChar));
    assert_eq!(validate_dir_segment(&"a".repeat(255)), Ok(()));
    assert_eq!(validate_dir_segment(&"a".repeat(256)), Err(E::TooLong));
    assert_eq!(validate_dir_segment("sub"), Ok(()));
}

#[test]
fn accepted_stems_ok() {
    for s in ACCEPTED_STEMS {
        assert_eq!(validate_stem(s), Ok(()), "{s:?}");
    }
    assert_eq!(ACCEPTED_STEMS.last().map(|s| s.len()), Some(252));
}
