//! The emitted-body bound (X-15): a `vks` body over `MAX_BODY_BYTES` is refused as `vks.limit`, per
//! fence, exactly where the reader applies it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test file

use mda_core::model::ArtifactModel;
use mda_core::serialize::{SerializeError, serialize};

const MAX: usize = 1 << 20;

fn model(json: serde_json::Value) -> ArtifactModel {
    serde_json::from_value(json).unwrap()
}

fn assert_body_limit(m: &ArtifactModel) {
    let Err(SerializeError::Vars(e)) = serialize(m, MAX) else {
        panic!("expected Vars(vks.limit), got {:?}", serialize(m, MAX));
    };
    assert_eq!(e.code, "vks.limit");
    let p = |k: &str| e.params.get(k).map(String::as_str);
    assert_eq!(p("limit"), Some("body_bytes"));
    assert_eq!(p("max"), Some("65536"));
    assert_eq!(p("line"), Some("0"));
    assert_eq!(e.params.len(), 3);
}

fn flat(len: usize) -> ArtifactModel {
    model(serde_json::json!({"artifactType":"Variables","title":"V",
        "blocks":[{"vars":[{"name":"VK-a","value":"a".repeat(len)}]}]}))
}

#[test]
fn variables_body_over_64k_is_vks_limit() {
    assert_body_limit(&flat(70_000));
}

#[test]
fn code_block_vars_body_over_64k_is_vks_limit() {
    assert_body_limit(&model(
        serde_json::json!({"artifactType":"Snippet","title":"S",
        "blocks":[{"code":"<VK-a>","vars":[{"name":"VK-a","value":"a".repeat(70_000)}]}]}),
    ));
}

#[test]
fn body_at_limit_is_ok() {
    // `VK-a: ` is 6 bytes + value + `\n`: 65_529 + 7 = 65_536 exactly.
    assert!(serialize(&flat(65_529), MAX).is_ok());
    assert_body_limit(&flat(65_530));
}
