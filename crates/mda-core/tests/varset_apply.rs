//! T5b.2: var-set apply (spec §13.3, X-17) — split from `varset_score.rs` at H5b.1 (size).
#![allow(clippy::indexing_slicing, clippy::unwrap_used)] // reason: test assertions index fixed shapes

use mda_core::model::ParsedVar;
use mda_core::render::Values;
use mda_core::varset::{Action, apply};
use mda_core::vks::VksValue;

fn t(n: &str, v: &str) -> ParsedVar {
    ParsedVar::text(n, v)
}

fn cur(p: &[(&str, &str)]) -> Values {
    p.iter()
        .map(|(k, v)| ((*k).into(), VksValue::Str((*v).into())))
        .collect()
}

fn set(p: &[(&str, &str)]) -> Vec<ParsedVar> {
    p.iter().map(|(k, v)| t(k, v)).collect()
}

fn find<'a>(a: &'a mda_core::varset::Applied, n: &str) -> &'a mda_core::varset::Change {
    a.changes.iter().find(|c| c.name == n).unwrap()
}

fn st(x: &str) -> VksValue {
    VksValue::Str(x.into())
}

#[test]
#[allow(clippy::cognitive_complexity)] // reason: one flat sequence of ported TS cases, no branching
fn apply_ports_ts() {
    // empty current + set -> all filled
    let a = apply(
        &Values::new(),
        &set(&[("VK-host", "localhost"), ("VK-port", "3000")]),
    );
    assert_eq!(a.changes.len(), 2);
    assert!(a.changes.iter().all(|c| c.action == Action::Filled));
    assert_eq!(
        a.values,
        cur(&[("VK-host", "localhost"), ("VK-port", "3000")])
    );
    // :70 filled row carries oldValue "" and the set's newValue
    let a = apply(&Values::new(), &set(&[("VK-x", "val")]));
    let c = find(&a, "VK-x");
    assert_eq!(
        (&c.old_value, &c.new_value, c.action),
        (&st(""), &st("val"), Action::Filled)
    );
    // overridden, new value wins
    let a = apply(
        &cur(&[("VK-host", "old.example.com"), ("VK-port", "8080")]),
        &set(&[("VK-host", "new.example.com"), ("VK-port", "3000")]),
    );
    let c = find(&a, "VK-host");
    assert_eq!(
        (&c.old_value, &c.new_value, c.action),
        (
            &st("old.example.com"),
            &st("new.example.com"),
            Action::Overridden
        )
    );
    let c = find(&a, "VK-port");
    assert_eq!(
        (&c.old_value, &c.new_value, c.action),
        (&st("8080"), &st("3000"), Action::Overridden)
    );
    assert_eq!(
        a.values,
        cur(&[("VK-host", "new.example.com"), ("VK-port", "3000")])
    );
    // kept
    let a = apply(
        &cur(&[("VK-keep", "preserved")]),
        &set(&[("VK-other", "unrelated")]),
    );
    let c = find(&a, "VK-keep");
    assert_eq!(
        (&c.old_value, &c.new_value, c.action),
        (&st("preserved"), &st("preserved"), Action::Kept)
    );
    assert_eq!(a.values["VK-keep"], st("preserved"));
    // mix of filled, overridden, kept
    let a = apply(
        &cur(&[("VK-keep", "stay"), ("VK-override", "old")]),
        &set(&[("VK-override", "new"), ("VK-fill", "fresh")]),
    );
    assert_eq!(find(&a, "VK-keep").action, Action::Kept);
    assert_eq!(find(&a, "VK-override").action, Action::Overridden);
    assert_eq!(find(&a, "VK-fill").action, Action::Filled);
    assert_eq!(
        a.values,
        cur(&[
            ("VK-keep", "stay"),
            ("VK-override", "new"),
            ("VK-fill", "fresh")
        ])
    );
    assert_eq!(a.changes.len(), 3);
    // :139 chained apply
    let first = apply(
        &Values::new(),
        &set(&[
            ("VK-host", "first-host"),
            ("VK-port", "8080"),
            ("VK-only-in-first", "one"),
        ]),
    );
    assert_eq!(
        first.values,
        cur(&[
            ("VK-host", "first-host"),
            ("VK-port", "8080"),
            ("VK-only-in-first", "one")
        ])
    );
    let second = apply(
        &first.values,
        &set(&[("VK-host", "second-host"), ("VK-new", "fresh")]),
    );
    assert_eq!(
        second.values,
        cur(&[
            ("VK-host", "second-host"),
            ("VK-port", "8080"),
            ("VK-only-in-first", "one"),
            ("VK-new", "fresh")
        ])
    );
    assert_eq!(find(&second, "VK-host").action, Action::Overridden);
    assert_eq!(find(&second, "VK-port").action, Action::Kept);
    assert_eq!(find(&second, "VK-only-in-first").action, Action::Kept);
    assert_eq!(find(&second, "VK-new").action, Action::Filled);
    // empty set value fills, still "filled"
    let a = apply(&Values::new(), &set(&[("VK-x", "")]));
    let c = find(&a, "VK-x");
    assert_eq!(
        (&c.old_value, &c.new_value, c.action),
        (&st(""), &st(""), Action::Filled)
    );
    assert_eq!(a.values["VK-x"], st(""));
    // empty set value over existing -> overridden with empty newValue
    let a = apply(&cur(&[("VK-x", "previous")]), &set(&[("VK-x", "")]));
    let c = find(&a, "VK-x");
    assert_eq!(
        (&c.old_value, &c.new_value, c.action),
        (&st("previous"), &st(""), Action::Overridden)
    );
    assert_eq!(a.values["VK-x"], st(""));
    // merged final state of the union
    let a = apply(
        &cur(&[("VK-a", "A"), ("VK-b", "B")]),
        &set(&[("VK-b", "B-NEW"), ("VK-c", "C")]),
    );
    assert_eq!(
        a.values,
        cur(&[("VK-a", "A"), ("VK-b", "B-NEW"), ("VK-c", "C")])
    );
    // one change per union name, in name order, no duplicates
    let a = apply(
        &cur(&[("VK-a", "1"), ("VK-b", "2")]),
        &set(&[("VK-b", "two"), ("VK-c", "3")]),
    );
    let names: Vec<_> = a.changes.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["VK-a", "VK-b", "VK-c"]);
    let a = apply(&cur(&[("VK-x", "old")]), &set(&[("VK-x", "new")]));
    assert_eq!(a.changes.len(), 1);
    assert_eq!(a.changes[0].action, Action::Overridden);
    // both empty
    let a = apply(&Values::new(), &[]);
    assert!(a.values.is_empty() && a.changes.is_empty());
    // purity: inputs unchanged
    let c = cur(&[("VK-x", "orig")]);
    let sv0 = set(&[("VK-x", "changed"), ("VK-new", "fresh")]);
    let snapshot = sv0.clone();
    let _ = apply(&c, &sv0);
    assert_eq!(c, cur(&[("VK-x", "orig")]));
    assert_eq!(sv0, snapshot);
}

#[test]
fn apply_structured_and_old_empty() {
    let rec = mda_core::vks::VksValue::Record(mda_core::vks::VksRecord(vec![(
        "k".into(),
        VksValue::Str("v".into()),
    )]));
    let set = [ParsedVar {
        name: "VK-a".into(),
        value: rec.clone(),
    }];
    let a = apply(&cur(&[("VK-a", "s")]), &set);
    assert_eq!(a.values["VK-a"], rec);
    assert_eq!(a.changes[0].new_value, rec);
    // no current value: oldValue is Str(""), never omitted
    let a = apply(&Values::new(), &[t("VK-z", "q")]);
    assert_eq!(a.changes[0].old_value, VksValue::Str(String::new()));
    assert_eq!(a.changes[0].action, Action::Filled);
}

#[test]
fn legacy_dup_last_wins() {
    let a = apply(&Values::new(), &[t("VK-a", "1"), t("VK-a", "2")]);
    assert_eq!(a.values, cur(&[("VK-a", "2")]));
    assert_eq!(a.changes.len(), 1);
}
