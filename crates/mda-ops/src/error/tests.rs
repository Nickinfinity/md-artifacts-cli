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

#[test]
fn w4_codes_exact_json() {
    let cases = [
        (VAULT_NOT_OBSIDIAN, "vault.not_obsidian"),
        (PATH_NOT_DIR, "path.not_dir"),
        (VAULT_UNKNOWN, "vault.unknown"),
        (VAULT_UNAVAILABLE, "vault.unavailable"),
        (CONFIG_INVALID, "config.invalid"),
    ];
    for (code, want) in cases {
        assert!(ALL_CODES.contains(&code), "{code}");
        assert_eq!(
            serde_json::to_value(OpError::new(code).with("path", "/v")).unwrap(),
            serde_json::json!({"code": want, "params": {"path": "/v"}})
        );
    }
}

#[test]
fn vault_error_maps_w4() {
    let root = Root::new(&std::env::temp_dir()).unwrap();
    let e = vault_error(
        VaultError::NotDir {
            path: root.path().join("Snippets"),
        },
        &root,
    );
    assert_eq!(
        (e.code, e.params["path"].as_str()),
        (PATH_NOT_DIR, "Snippets")
    );
    let e = vault_error(
        VaultError::ConfigInvalid {
            path: root.path().join(".mda/config.toml"),
        },
        &root,
    );
    assert_eq!(
        (e.code, e.params["path"].as_str()),
        (CONFIG_INVALID, ".mda/config.toml")
    );
    let e = OpError::from(VaultError::NotObsidian { path: "/v".into() });
    assert_eq!(
        (e.code, e.params["path"].as_str()),
        (VAULT_NOT_OBSIDIAN, "/v")
    );
}

#[test]
fn w5_codes_exact_json() {
    let with = [
        (VARS_BAD_NAME, "vars.bad_name", "name"),
        (VARS_VAR_EXISTS, "vars.var_exists", "name"),
        (VARS_VAR_NOT_FOUND, "vars.var_not_found", "name"),
        (VARS_SUBSET_EXISTS, "vars.subset_exists", "heading"),
        (VARS_BAD_HEADING, "vars.bad_heading", "reason"),
    ];
    for (code, want, key) in with {
        assert!(ALL_CODES.contains(&code), "{code}");
        assert_eq!(
            serde_json::to_value(OpError::new(code).with(key, "x")).unwrap(),
            serde_json::json!({"code": want, "params": {key: "x"}})
        );
    }
    for (code, want) in [
        (VARS_UNTITLED_DESCRIPTION, "vars.untitled_description"),
        (VARS_BAD_TITLE, "vars.bad_title"),
    ] {
        assert!(ALL_CODES.contains(&code), "{code}");
        assert_eq!(
            serde_json::to_value(OpError::new(code)).unwrap(),
            serde_json::json!({"code": want, "params": {}})
        );
    }
}

#[test]
fn edit_error_maps() {
    use mda_core::variables::EditError;
    let j = |e: EditError| serde_json::to_value(OpError::from(e)).unwrap();
    let cases = [
        (
            EditError::BadName("h".into()),
            serde_json::json!({"code": "vars.bad_name", "params": {"name": "h"}}),
        ),
        (
            EditError::VarExists("VK-a".into()),
            serde_json::json!({"code": "vars.var_exists", "params": {"name": "VK-a"}}),
        ),
        (
            EditError::VarNotFound("VK-a".into()),
            serde_json::json!({"code": "vars.var_not_found", "params": {"name": "VK-a"}}),
        ),
        (
            EditError::SubsetExists("Dev".into()),
            serde_json::json!({"code": "vars.subset_exists", "params": {"heading": "Dev"}}),
        ),
        (
            EditError::EmptyHeading,
            serde_json::json!({"code": "vars.bad_heading", "params": {"reason": "empty"}}),
        ),
        (
            EditError::UntitledDescription,
            serde_json::json!({"code": "vars.untitled_description", "params": {}}),
        ),
        (
            EditError::BlockNotFound(None),
            serde_json::json!({"code": "artifact.block_not_found", "params": {"block": ""}}),
        ),
        (
            EditError::BlockNotFound(Some(3)),
            serde_json::json!({"code": "artifact.block_not_found", "params": {"block": "3"}}),
        ),
        (
            EditError::Mixed,
            serde_json::json!({"code": "artifact.not_serializable", "params": {"reason": "variables"}}),
        ),
    ];
    for (e, want) in cases {
        assert_eq!(j(e), want);
    }
}

#[test]
fn w5b_code_exact_json() {
    assert!(ALL_CODES.contains(&VARS_LIMIT));
    assert_eq!(
        serde_json::to_value(
            OpError::new(VARS_LIMIT)
                .with("limit", "files")
                .with("max", "10000")
        )
        .unwrap(),
        serde_json::json!({"code": "vars.limit", "params": {"limit": "files", "max": "10000"}})
    );
}

#[test]
fn vault_limit_maps() {
    use mda_vault::WalkLimit;
    let root = Root::new(&std::env::temp_dir()).unwrap();
    for (l, s, max) in [
        (WalkLimit::Depth, "depth", 32),
        (WalkLimit::Files, "files", 10_000),
        (WalkLimit::Bytes, "bytes", 67_108_864),
    ] {
        let want = serde_json::json!({"code": "vars.limit", "params": {"limit": s, "max": max.to_string()}});
        let e = vault_error(VaultError::Limit { limit: l, max }, &root);
        assert_eq!(serde_json::to_value(e).unwrap(), want);
        let e = OpError::from(VaultError::Limit { limit: l, max });
        assert_eq!(serde_json::to_value(e).unwrap(), want);
    }
}
