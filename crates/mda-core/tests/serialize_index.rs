//! W3b serializer additions (spec §1.4, §8.7): `index: true` emitted last and carried by `from_parsed`,
//! and an empty whole-file body written as frontmatter only. Split from `serialize.rs` (size).
#![allow(clippy::unwrap_used, clippy::panic)] // reason: assertion helpers outside #[test] fns; a panic is the failure signal

use mda_core::error::FieldReason;
use mda_core::model::ArtifactModel;
use mda_core::parse::parse_from_content;
use mda_core::serialize::{SerializeError, from_parsed, serialize};
use serde_json::{Value, json};

const MAX: usize = 1 << 20;

fn model(v: Value) -> ArtifactModel {
    serde_json::from_value(v).unwrap()
}

fn ser(v: Value) -> String {
    serialize(&model(v), MAX).unwrap()
}

fn refused(v: Value) -> (String, FieldReason) {
    match serialize(&model(v), MAX) {
        Err(SerializeError::Field { field, reason }) => (field, reason),
        other => panic!("expected Field, got {other:?}"),
    }
}

// W7c: `index` (then `paths`) just before `tags`, which is last; tags as a block list.
const INDEX_BYTES: &str = "---\nartifactType: AIAgentsConfig\ntitle: Index\ntarget: CLAUDE.md\nindex: true\ntags:\n  - a\n---\n\n```\n1. [[a]]\n```\n";

fn index_model(index: bool) -> Value {
    json!({"artifactType":"AIAgentsConfig","title":"Index","tags":["a"],"target":"CLAUDE.md",
           "index":index,"blocks":[{"code":"1. [[a]]"}]})
}

#[test]
fn index_true_is_emitted_before_tags() {
    assert_eq!(ser(index_model(true)), INDEX_BYTES);
}

#[test]
fn index_false_is_not_emitted_nor_serialized() {
    let out = ser(index_model(false));
    assert!(!out.contains("index:"), "{out}");
    assert!(
        serde_json::to_value(model(index_model(false)))
            .unwrap()
            .get("index")
            .is_none()
    );
}

#[test]
fn index_survives_from_parsed_round_trip() {
    let p = parse_from_content(INDEX_BYTES, "AIAgentsConf/i.md");
    let m = from_parsed(&p);
    assert!(m.index);
    assert_eq!(serialize(&m, MAX).unwrap(), INDEX_BYTES);
}

#[test]
fn empty_whole_file_body_is_frontmatter_only() {
    let want = "---\nartifactType: Template\ntitle: a\nextension: ts\n---\n";
    for blocks in [json!([{"code":""}]), json!([]), json!([{"code":"  \n"}])] {
        let got =
            ser(json!({"artifactType":"Template","title":"a","extension":"ts","blocks":blocks}));
        assert_eq!(got, want);
    }
    let got =
        ser(json!({"artifactType":"AIAgentsConfig","target":"CLAUDE.md","blocks":[{"code":""}]}));
    assert_eq!(
        got,
        "---\nartifactType: AIAgentsConfig\ntarget: CLAUDE.md\n---\n"
    );
}

#[test]
fn empty_whole_file_with_vars_stays_refused() {
    let r = refused(json!({"artifactType":"Template","title":"a","blocks":[
        {"code":"","vars":[{"name":"VK-a","value":"x"}]}]}));
    assert_eq!(r, ("blocks/0/language".to_owned(), FieldReason::RoundTrip));
}

#[test]
fn empty_snippet_is_still_fenced() {
    let out = ser(json!({"artifactType":"Snippet","title":"s","blocks":[{"code":""}]}));
    assert!(out.contains("```"), "{out}");
}
