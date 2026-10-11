//! The error table's exact-JSON and listing tests (split from `error.rs` to keep it under ~400 lines).

use super::*;

#[test]
fn vault_error_relativizes_paths() {
    let root = Root::new(&std::env::temp_dir()).unwrap();
    let abs = root.path().join("Snippets/x.md");
    let e = vault_error(VaultError::NotFound { path: abs }, &root);
    assert_eq!(e.params["path"], "Snippets/x.md");
}

#[test]
fn vault_error_echoes_outside_candidate() {
    let root = Root::new(&std::env::temp_dir()).unwrap();
    let e = vault_error(
        VaultError::OutsideRoot {
            path: "../etc/passwd".into(),
        },
        &root,
    );
    assert_eq!(e.code, PATH_OUTSIDE_ROOT);
    assert_eq!(e.params["path"], "../etc/passwd");
}

#[test]
fn conflict_maps_relative_with_hashes() {
    let root = Root::new(&std::env::temp_dir()).unwrap();
    let e = vault_error(
        VaultError::Conflict {
            path: root.path().join("Templates/a.md"),
            expected: "a".into(),
            actual: "b".into(),
        },
        &root,
    );
    assert_eq!(
        serde_json::to_value(&e).unwrap(),
        serde_json::json!({"code": "file.conflict",
            "params": {"path": "Templates/a.md", "expected": "a", "actual": "b"}})
    );
}

#[test]
fn exists_maps_relative() {
    let root = Root::new(&std::env::temp_dir()).unwrap();
    let e = vault_error(
        VaultError::Exists {
            path: root.path().join("Templates/a.md"),
        },
        &root,
    );
    assert_eq!(
        serde_json::to_value(&e).unwrap(),
        serde_json::json!({"code": "file.exists", "params": {"path": "Templates/a.md"}})
    );
}

#[test]
fn write_codes_exact_json() {
    let cases = [
        (ARTIFACT_VARS_INVALID, "path", "artifact.vars_invalid"),
        (
            ARTIFACT_BLOCK_NOT_FOUND,
            "block",
            "artifact.block_not_found",
        ),
        (
            ARTIFACT_NOT_SERIALIZABLE,
            "reason",
            "artifact.not_serializable",
        ),
        (
            ARTIFACT_UNREPRESENTABLE,
            "field",
            "artifact.unrepresentable",
        ),
        (VKS_UNREPRESENTABLE, "var", "vks.unrepresentable"),
    ];
    for (code, key, want) in cases {
        let e = OpError::new(code).with(key, "x");
        assert_eq!(
            serde_json::to_value(&e).unwrap(),
            serde_json::json!({"code": want, "params": {key: "x"}})
        );
    }
}

#[test]
fn naming_codes_listed() {
    for c in mda_core::error::NAMING_CODES {
        assert!(ALL_CODES.contains(c), "{c}");
    }
}

#[test]
fn warning_codes_are_not_error_codes() {
    for c in RENDER_WARNING_CODES {
        assert!(!ALL_CODES.contains(c), "{c}");
    }
}

#[test]
fn w3_codes_exact_json() {
    let bare = [
        (RENDER_CONTAINS_ESCAPE, "render.contains_escape"),
        (NAMING_EMPTY, "naming.empty"),
        (NAMING_EDGE_SPACE, "naming.edge_space"),
        (NAMING_EDGE_DOT, "naming.edge_dot"),
        (NAMING_ILLEGAL_CHAR, "naming.illegal_char"),
        (NAMING_CONTROL_CHAR, "naming.control_char"),
        (NAMING_RESERVED, "naming.reserved"),
        (ARTIFACT_IS_INDEX, "artifact.is_index"),
    ];
    for (code, want) in bare {
        assert_eq!(
            serde_json::to_value(OpError::new(code)).unwrap(),
            serde_json::json!({"code": want, "params": {}})
        );
    }
    let with = [
        (NAMING_PATH_INJECTION, "field", "naming.path_injection"),
        (NAMING_TOO_LONG, "max", "naming.too_long"),
        (ARTIFACT_NOT_WHOLE_FILE, "type", "artifact.not_whole_file"),
        (ARTIFACT_MULTI_BLOCK, "count", "artifact.multi_block"),
    ];
    for (code, key, want) in with {
        assert_eq!(
            serde_json::to_value(OpError::new(code).with(key, "x")).unwrap(),
            serde_json::json!({"code": want, "params": {key: "x"}})
        );
    }
    let limit = OpError::new(RENDER_LIMIT)
        .with("limit", "steps")
        .with("max", "1000000");
    assert_eq!(
        serde_json::to_value(limit).unwrap(),
        serde_json::json!({"code": "render.limit", "params": {"limit": "steps", "max": "1000000"}})
    );
}

#[test]
fn name_error_maps_field() {
    use mda_core::naming::{NameError, NameField};
    let e = OpError::from(NameError::PathInjection(NameField::Title));
    assert_eq!(
        serde_json::to_value(e).unwrap(),
        serde_json::json!({"code": "naming.path_injection", "params": {"field": "title"}})
    );
    let e = OpError::from(NameError::TooLong);
    assert_eq!(e.params["max"], "255");
    assert!(OpError::from(NameError::Reserved).params.is_empty());
}

#[test]
fn vars_error_maps_unchanged() {
    let v = mda_core::error::VarsError::new(VKS_CONTROL_CHAR, 0).with("var", "VK-a");
    let e = OpError::from(v);
    assert_eq!(e.code, "vks.control_char");
    assert_eq!(e.params["var"], "VK-a");
}

#[test]
fn path_codes_listed() {
    for c in mda_core::error::PATH_CODES {
        assert!(ALL_CODES.contains(c), "{c}");
    }
}

#[test]
fn w3b_codes_exact_json() {
    for c in PATH_CODES {
        assert_eq!(
            serde_json::to_value(OpError::new(c).with("path", "x")).unwrap(),
            serde_json::json!({"code": *c, "params": {"path": "x"}})
        );
    }
    let cases = [
        (INDEX_NOT_INDEX, "path", "index.not_index"),
        (INDEX_UNSLUGGABLE, "segment", "index.unsluggable"),
    ];
    for (code, key, want) in cases {
        assert_eq!(
            serde_json::to_value(OpError::new(code).with(key, "x")).unwrap(),
            serde_json::json!({"code": want, "params": {key: "x"}})
        );
    }
    let l = OpError::new(INDEX_LIMIT)
        .with("limit", "links")
        .with("max", "1000");
    assert_eq!(
        serde_json::to_value(l).unwrap(),
        serde_json::json!({"code": "index.limit", "params": {"limit": "links", "max": "1000"}})
    );
}

#[test]
fn vks_codes_listed() {
    for c in mda_core::error::VKS_CODES {
        assert!(ALL_CODES.contains(c), "{c}");
    }
}
