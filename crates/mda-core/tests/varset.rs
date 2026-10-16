//! `varset` shape, listed sub-sets and search (spec §13.2; TS `varset.service.ts:205–238`,
//! `variablesView.provider.ts:160–256`).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)] // reason: test helpers outside #[test] fns

use mda_core::model::{ParsedArtifact, ParsedVar};
use mda_core::parse::parse_from_content;
use mda_core::serialize::from_parsed;
use mda_core::varset::{Query, Shape, Subset, file_text, filter_subsets, shape, subsets};

const BLANK: &str = "---\nartifactType: Variables\ntitle: Blank\n---\n";
const FLAT: &str = "---\nartifactType: Variables\ntitle: Flat\n---\n\n```vks\nVK-host: localhost\nVK-port: '3000'\n```\n";
const SETS: &str = "---\nartifactType: Variables\ntitle: Sets\ntags: [env, dev]\n---\n\n## Dev\nDev machine.\n\n```vks\nVK-host: localhost\n```\n\n## Prod\n\n```vks\nVK-host: example.com\n```\n";
const ONE_SET: &str =
    "---\nartifactType: Variables\ntitle: One\n---\n\n## Only\n\n```vks\nVK-a: x\n```\n";
const MIXED: &str = "---\nartifactType: Variables\ntitle: Mixed\n---\n\n```vks\nVK-top: t\n```\n\n## A\n\n```vks\nVK-a: a\n```\n";
const BAD: &str = "---\nartifactType: Variables\ntitle: Bad\n---\n\n```vks\nVK-a: x # note\n```\n";
const FIRST_EMPTY: &str =
    "---\nartifactType: Variables\ntitle: F\n---\n\n## A\n\n## B\n\n```vks\nVK-b: 1\n```\n";
const EMPTY_SET: &str = "---\nartifactType: Variables\ntitle: E\n---\n\n## X\n\n```vks\n```\n";
const ENVS: &str = "---\nartifactType: Variables\ntitle: Envs\ndescription: Deploy targets\n---\n\n## Stage\nStaging box.\n\n```vks\nVK-host: stage.example.com\n```\n\n## Prod\n\n```vks\nVK-host: example.com\nVK-db:\n  user: admin\n  zone: eu-west\n```\n";
const LIST_OF_RECORDS: &str = "---\nartifactType: Variables\ntitle: L\n---\n\n## S\n\n```vks\nVK-hosts:\n  - name: alpha\n    ip: 10.0.0.9\n```\n";

fn p(src: &str) -> ParsedArtifact {
    parse_from_content(src, "Variables/x.md")
}
fn q(s: &str) -> Query {
    Query::new(s).unwrap()
}
fn headings(v: &[Subset]) -> Vec<&str> {
    v.iter().map(|s| s.heading.as_str()).collect()
}

#[test]
fn shape_table() {
    assert_eq!(shape(&p(BLANK)), Shape::Blank);
    assert_eq!(shape(&p(FLAT)), Shape::Flat);
    for s in [SETS, ONE_SET, MIXED] {
        assert_eq!(shape(&p(s)), Shape::Sets);
    }
    let bad = p(BAD);
    assert!(bad.vars_error.is_some());
    assert_eq!(shape(&bad), Shape::Blank);
}

#[test]
fn subsets_flat_is_untitled_block_zero() {
    let s = subsets(&p(FLAT));
    assert_eq!(
        s,
        vec![Subset {
            index: 0,
            heading: String::new(),
            description: String::new(),
            vars: vec![
                ParsedVar::text("VK-host", "localhost"),
                ParsedVar::text("VK-port", "3000")
            ],
            vars_error: None,
        }]
    );
    let s = subsets(&p(SETS));
    assert_eq!(s.len(), 2);
    assert_eq!(s[0].description, "Dev machine.");
    assert!(subsets(&p(BLANK)).is_empty());
    assert_eq!(headings(&subsets(&p(EMPTY_SET))), ["X"]);
    let f = subsets(&p(FIRST_EMPTY));
    // The parser drops a fenceless `##` section (#54), so only B remains, at index 0.
    assert_eq!(f.len(), 1);
    assert_eq!((f[0].index, f[0].heading.as_str()), (0, "B"));
    assert_eq!(f[0].vars, [ParsedVar::text("VK-b", "1")]);
}

#[test]
fn subsets_address_matches_model_blocks() {
    for src in [
        BLANK,
        FLAT,
        SETS,
        ONE_SET,
        MIXED,
        BAD,
        FIRST_EMPTY,
        EMPTY_SET,
        ENVS,
    ] {
        let a = p(src);
        let got: Vec<_> = subsets(&a)
            .into_iter()
            .map(|s| (s.index, s.heading))
            .collect();
        let want: Vec<_> = from_parsed(&a)
            .blocks
            .iter()
            .enumerate()
            .map(|(i, b)| (i, b.heading.clone()))
            .collect();
        assert_eq!(got, want, "{src}");
    }
}

#[test]
fn query_trims_and_lowercases() {
    let k = q("  HoSt ");
    assert!(k.hit("vk-host"));
    assert!(k.hit("VK-HOST"));
    for e in ["", "   ", "\u{feff}"] {
        assert!(Query::new(e).is_none(), "{e:?}");
    }
}

#[test]
fn file_text_matches_ts() {
    let t = file_text("Flat", "", &["env".into(), "dev".into()]);
    assert_eq!(t, "Flat  #env #dev");
    assert!(q("#env").hit(&t));
    assert!(q("env").hit(&t));
}

#[test]
fn filter_ports_ts_search() {
    let envs = subsets(&p(ENVS));
    let one = |query: &str| filter_subsets(envs.clone(), &q(query));
    // var name hit: only that var, and only its sub-set
    let r = one("vk-db");
    assert_eq!(headings(&r), ["Prod"]);
    assert_eq!(r[0].index, 1);
    assert_eq!(r[0].vars.len(), 1);
    // value hit
    let r = one("stage.example.com");
    assert_eq!(headings(&r), ["Stage"]);
    // heading hit keeps whole (Prod has 2 vars)
    let r = one("PROD");
    assert_eq!(r[0].vars.len(), 2);
    // heading hit keeps Stage whole
    assert_eq!(one("stage")[0].vars.len(), 1);
    // description hit keeps whole
    assert_eq!(headings(&one("staging box")), ["Stage"]);
    assert!(one("nomatch").is_empty());
    // a value hit in both sub-sets keeps only the matching var of each
    let r = one("example.com");
    assert_eq!(headings(&r), ["Stage", "Prod"]);
    assert_eq!(r[1].vars.len(), 1);
    assert_eq!(r[1].vars[0].name, "VK-host");
    // "admin" only hits Prod's record leaf; Stage is dropped
    let r = one("admin");
    assert_eq!((r.len(), r[0].index), (1, 1));
}

#[test]
fn structured_leaves_not_keys() {
    let envs = subsets(&p(ENVS));
    let r = filter_subsets(envs.clone(), &q("eu-west"));
    assert_eq!(headings(&r), ["Prod"]);
    assert!(filter_subsets(envs, &q("zone")).is_empty());
    let l = subsets(&p(LIST_OF_RECORDS));
    assert_eq!(filter_subsets(l.clone(), &q("10.0.0.9")).len(), 1);
    assert!(filter_subsets(l, &q("ip")).is_empty());
}
