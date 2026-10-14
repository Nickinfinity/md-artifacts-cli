//! Port of `variables-crud.test.ts` (:89–:324) onto `mda_core::variables`: names, sub-set
//! addressing (index + heading), shape transitions, validate-before-mutate, error order.
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)] // reason: test helpers outside #[test] fns

use mda_core::error::FieldReason;
use mda_core::model::ArtifactModel;
use mda_core::parse::parse_from_content;
use mda_core::serialize::{SerializeError, serialize};
use mda_core::variables::{EditError as E, SubsetRef, *};
use mda_core::vks::{VksRecord, VksValue};

const FM: &str = "---\nartifactType: Variables\ntitle: Flat\n---\n";
const FLAT: &str = "---\nartifactType: Variables\ntitle: Flat\n---\n\n```vks\nVK-host: localhost\nVK-port: '3000'\n```\n";
const SETS: &str = "---\nartifactType: Variables\ntitle: Sets\ntags: [env, dev]\n---\n\n## Dev\nDev machine.\n\n```vks\nVK-host: localhost\n```\n\n## Prod\n\n```vks\nVK-host: example.com\n```\n";
const BLANK: &str = "---\nartifactType: Variables\ntitle: Blank\n---\n";
const ONE_SET: &str =
    "---\nartifactType: Variables\ntitle: One\n---\n\n## Only\n\n```vks\nVK-a: x\n```\n";
const LEGACY: &str = "---\nartifactType: Variables\ntitle: Legacy\n---\n\n```vks\nVK-host=localhost\nAPI_URL=http://a\n```\n";
const BADKEY: &str =
    "---\nartifactType: Variables\ntitle: Bad key\n---\n\n```vks\nVK-a:b=c\nVK-ok=1\n```\n";
const MIXED: &str = "---\nartifactType: Variables\ntitle: Mixed\n---\n\n```vks\nVK-top: t\n```\n\n## A\n\n```vks\nVK-a: a\n```\n";
const DUPS: &str = "---\nartifactType: Variables\ntitle: Dups\n---\n\n```vks\nA=1\nA=2\n```\n";
const FIRST_EMPTY: &str =
    "---\nartifactType: Variables\ntitle: F\n---\n\n## A\n\n## B\n\n```vks\nVK-b: 1\n```\n";

fn m(src: &str) -> ArtifactModel {
    model_of(&parse_from_content(src, "Variables/x.md")).unwrap()
}
fn s(v: &str) -> VksValue {
    VksValue::Str(v.into())
}
fn at(index: usize, heading: &str) -> SubsetRef {
    SubsetRef {
        index,
        heading: heading.into(),
    }
}
fn bytes(m: &ArtifactModel) -> String {
    serialize(m, 1 << 20).unwrap()
}
/// A refusal must leave the model untouched (validate before mutate).
fn refuse(m: &mut ArtifactModel, f: impl FnOnce(&mut ArtifactModel) -> Result<(), E>, want: E) {
    let before = m.clone();
    assert_eq!(f(m), Err(want));
    assert_eq!(*m, before);
}
fn names(m: &ArtifactModel, b: usize) -> Vec<&str> {
    m.blocks[b].vars.iter().map(|v| v.name.as_str()).collect()
}
fn headings(m: &ArtifactModel) -> Vec<&str> {
    m.blocks.iter().map(|b| b.heading.as_str()).collect()
}

#[test]
fn var_name_table() {
    for n in ["VK-a", "VK-a_1", "VK-Ab9"] {
        assert!(is_var_name(n), "{n}");
    }
    for n in [
        "VK-a.b",
        "VK-",
        "VK-1a",
        "VK-a-b",
        "VK-each:x",
        "VK-end:x",
        "/VK-a",
        "vk-a",
        "host",
        " VK-a",
        "VK-a>",
        "VK-é",
    ] {
        assert!(!is_var_name(n), "{n}");
    }
}

#[test]
fn default_names() {
    assert_eq!(unique_default_name(&[]), "Default");
    assert_eq!(unique_default_name(&["Default"]), "Default 2");
    assert_eq!(unique_default_name(&["Default", "Default 2"]), "Default 3");
}

#[test]
fn add_var_shapes() {
    let mut f = m(FLAT);
    add_var(&mut f, None, "VK-new", s("x")).unwrap();
    assert_eq!(f.blocks[0].vars.len(), 3);
    assert_eq!(names(&f, 0)[2], "VK-new");
    assert_eq!(f.blocks[0].vars[2].value, s("x"));

    let mut b = m(BLANK);
    add_var(&mut b, None, "VK-new", s("x")).unwrap();
    assert_eq!(
        bytes(&b),
        format!(
            "{}\n```vks\nVK-new: x\n```\n",
            "---\nartifactType: Variables\ntitle: Blank\n---\n"
        )
    );

    let mut sets = m(SETS);
    refuse(
        &mut sets,
        |mm| add_var(mm, None, "VK-n", s("")),
        E::BlockNotFound(None),
    );
    add_var(&mut sets, Some(&at(0, "Dev")), "VK-n", s("")).unwrap();
    assert_eq!(names(&sets, 0), ["VK-host", "VK-n"]);
    // same name in another sub-set is fine
    add_var(&mut sets, Some(&at(1, "Prod")), "VK-n", s("")).unwrap();
    refuse(
        &mut sets,
        |mm| add_var(mm, Some(&at(0, "Prod")), "VK-n", s("")),
        E::BlockNotFound(Some(0)),
    );
    refuse(
        &mut sets,
        |mm| add_var(mm, Some(&at(9, "Dev")), "VK-n", s("")),
        E::BlockNotFound(Some(9)),
    );
}

#[test]
fn add_var_refusals_leave_model_untouched() {
    for src in [FLAT, SETS] {
        let mut x = m(src);
        let r = at(0, &x.blocks[0].heading.clone());
        let r = (src == SETS).then_some(r);
        refuse(
            &mut x,
            |mm| add_var(mm, r.as_ref(), "VK-host", s("")),
            E::VarExists("VK-host".into()),
        );
        for bad in ["host", "API_URL", "VK-a.b"] {
            refuse(
                &mut x,
                |mm| add_var(mm, r.as_ref(), bad, s("")),
                E::BadName(bad.into()),
            );
        }
    }
    // blank + bad name: no block pushed (A2 #5)
    let mut b = m(BLANK);
    refuse(
        &mut b,
        |mm| add_var(mm, None, "host", s("")),
        E::BadName("host".into()),
    );
    assert!(b.blocks.is_empty());
}

#[test]
fn rename_var_cases() {
    let mut f = m(FLAT);
    rename_var(&mut f, None, "VK-host", "VK-h").unwrap();
    assert_eq!(names(&f, 0), ["VK-h", "VK-port"]);
    assert_eq!(f.blocks[0].vars[0].value, s("localhost"));
    let before = f.clone();
    refuse(
        &mut f,
        |mm| rename_var(mm, None, "VK-zzz", "VK-q"),
        E::VarNotFound("VK-zzz".into()),
    );
    refuse(
        &mut f,
        |mm| rename_var(mm, None, "VK-h", "nope"),
        E::BadName("nope".into()),
    );
    refuse(
        &mut f,
        |mm| rename_var(mm, None, "VK-h", "VK-port"),
        E::VarExists("VK-port".into()),
    );
    rename_var(&mut f, None, "VK-h", "VK-h").unwrap();
    assert_eq!(f, before);

    let mut l = m(LEGACY);
    rename_var(&mut l, None, "API_URL", "VK-api_url").unwrap();
    assert_eq!(names(&l, 0), ["VK-host", "VK-api_url"]);

    let mut k = m(BADKEY);
    assert!(matches!(
        serialize(&k, 1 << 20),
        Err(SerializeError::Vars(_))
    ));
    rename_var(&mut k, None, "VK-a:b", "VK-a").unwrap();
    assert!(serialize(&k, 1 << 20).is_ok());
}

#[test]
fn set_var_cases() {
    let mut f = m(FLAT);
    set_var(&mut f, None, "VK-host", s("h")).unwrap();
    assert_eq!(f.blocks[0].vars[0].value, s("h"));
    let rec = VksValue::Record(VksRecord(vec![("a".into(), s("1"))]));
    set_var(&mut f, None, "VK-port", rec.clone()).unwrap();
    assert_eq!(f.blocks[0].vars[1].value, rec);
    refuse(
        &mut f,
        |mm| set_var(mm, None, "VK-no", s("")),
        E::VarNotFound("VK-no".into()),
    );
    let mut l = m(LEGACY);
    set_var(&mut l, None, "API_URL", s("u")).unwrap();
    assert_eq!(l.blocks[0].vars[1].value, s("u"));
}

#[test]
fn delete_var_cases() {
    let mut f = m(FLAT);
    delete_var(&mut f, None, "VK-host").unwrap();
    assert_eq!(names(&f, 0), ["VK-port"]);
    delete_var(&mut f, None, "VK-port").unwrap();
    assert_eq!(bytes(&f), FM);
    refuse(
        &mut f,
        |mm| delete_var(mm, None, "VK-port"),
        E::VarNotFound("VK-port".into()),
    );

    let mut d = m(DUPS);
    delete_var(&mut d, None, "A").unwrap();
    assert!(d.blocks[0].vars.is_empty());

    let mut o = m(ONE_SET);
    delete_var(&mut o, Some(&at(0, "Only")), "VK-a").unwrap();
    assert!(bytes(&o).ends_with("## Only\n\n```vks\n```\n"));
}

#[test]
fn add_subset_cases() {
    let mut f = m(FLAT);
    add_subset(&mut f, "Prod").unwrap();
    assert_eq!(headings(&f), ["Default", "Prod"]);
    assert_eq!(names(&f, 0), ["VK-host", "VK-port"]);
    assert_eq!(
        bytes(&f),
        format!(
            "{FM}\n## Default\n\n```vks\nVK-host: localhost\nVK-port: '3000'\n```\n\n## Prod\n\n```vks\n```\n"
        )
    );
    let mut f = m(FLAT);
    add_subset(&mut f, "Default").unwrap();
    assert_eq!(headings(&f), ["Default 2", "Default"]);

    let mut b = m(BLANK);
    add_subset(&mut b, "X").unwrap();
    assert_eq!(headings(&b), ["X"]);

    let mut s2 = m(SETS);
    refuse(
        &mut s2,
        |mm| add_subset(mm, "Dev"),
        E::SubsetExists("Dev".into()),
    );
    refuse(&mut s2, |mm| add_subset(mm, "  "), E::EmptyHeading);
    add_subset(&mut s2, " Stage ").unwrap();
    assert_eq!(headings(&s2), ["Dev", "Prod", "Stage"]);

    let mut g = m(SETS);
    add_subset(&mut g, "A\n## B").unwrap();
    assert!(matches!(
        serialize(&g, 1 << 20),
        Err(SerializeError::Field {
            reason: FieldReason::RoundTrip,
            ..
        })
    ));
}

#[test]
fn rename_subset_cases() {
    let mut x = m(SETS);
    rename_subset(&mut x, &at(1, "Prod"), "Live").unwrap();
    assert_eq!(headings(&x), ["Dev", "Live"]);
    refuse(
        &mut x,
        |mm| rename_subset(mm, &at(1, "Live"), "Dev"),
        E::SubsetExists("Dev".into()),
    );
    rename_subset(&mut x, &at(1, "Live"), "Live").unwrap();
    refuse(
        &mut x,
        |mm| rename_subset(mm, &at(1, "Live"), ""),
        E::EmptyHeading,
    );
    refuse(
        &mut x,
        |mm| rename_subset(mm, &at(1, "Prod"), "Z"),
        E::BlockNotFound(Some(1)),
    );
    let mut f = m(FLAT);
    rename_subset(&mut f, &at(0, ""), "X").unwrap();
    assert!(bytes(&f).contains("\n## X\n"));
}

#[test]
fn describe_subset_cases() {
    let mut x = m(SETS);
    describe_subset(&mut x, &at(1, "Prod"), "  hi  ").unwrap();
    assert_eq!(x.blocks[1].description, "hi");
    assert!(bytes(&x).contains("## Prod\nhi\n"));
    describe_subset(&mut x, &at(0, "Dev"), "").unwrap();
    assert!(!bytes(&x).contains("Dev machine."));
    let mut f = m(FLAT);
    refuse(
        &mut f,
        |mm| describe_subset(mm, &at(0, ""), "d"),
        E::UntitledDescription,
    );
    refuse(
        &mut x,
        |mm| describe_subset(mm, &at(5, "Q"), "d"),
        E::BlockNotFound(Some(5)),
    );
}

#[test]
fn delete_subset_cases() {
    let mut x = m(SETS);
    delete_subset(&mut x, &at(1, "Prod")).unwrap();
    assert!(bytes(&x).contains("## Dev\n"));
    let mut o = m(ONE_SET);
    delete_subset(&mut o, &at(0, "Only")).unwrap();
    assert_eq!(bytes(&o), "---\nartifactType: Variables\ntitle: One\n---\n");
    let mut f = m(FLAT);
    delete_subset(&mut f, &at(0, "")).unwrap();
    assert_eq!(bytes(&f), FM);
    refuse(
        &mut f,
        |mm| delete_subset(mm, &at(3, "Z")),
        E::BlockNotFound(Some(3)),
    );
}

#[test]
fn model_of_shapes() {
    assert_eq!(m(FLAT).blocks.len(), 1);
    assert_eq!(m(SETS).blocks.len(), 2);
    assert_eq!(m(BLANK).blocks.len(), 0);
    assert_eq!(
        model_of(&parse_from_content(MIXED, "Variables/x.md")),
        Err(E::Mixed)
    );
    // A2 #1: a fenceless first section is not a mixed file; the parser drops it (accepted limit, spec §13.1).
    assert_eq!(
        model_of(&parse_from_content(FIRST_EMPTY, "Variables/x.md"))
            .unwrap()
            .blocks
            .len(),
        1
    );
}

#[test]
fn error_order_name_subset_var_dup() {
    let mut x = m(SETS);
    refuse(
        &mut x,
        |mm| rename_var(mm, Some(&at(9, "X")), "VK-host", "nope"),
        E::BadName("nope".into()),
    );
    refuse(
        &mut x,
        |mm| rename_var(mm, Some(&at(9, "X")), "VK-host", "VK-ok"),
        E::BlockNotFound(Some(9)),
    );
    refuse(
        &mut x,
        |mm| rename_var(mm, Some(&at(0, "Dev")), "VK-missing", "VK-host"),
        E::VarNotFound("VK-missing".into()),
    );
    let mut l = m(LEGACY);
    refuse(
        &mut l,
        |mm| rename_var(mm, None, "API_URL", "API_URL"),
        E::BadName("API_URL".into()),
    );
}
