//! Language tables (T3.2): lookups, guards, prototype-key traps.
#![allow(clippy::unwrap_used)] // reason: test file

use mda_core::language::{
    LANG_ALIAS, LANG_EXT, LANG_FENCE, ext_for_lang, map_language_id, normalize_lang_id,
};

#[test]
fn map_language_id_uses_fence_table() {
    assert_eq!(map_language_id("typescriptreact"), "tsx");
    for (id, fence) in LANG_FENCE {
        assert_eq!(map_language_id(id), *fence);
    }
    for pass in ["javascript", "", "zig"] {
        assert_eq!(map_language_id(pass), pass);
    }
}

#[test]
fn normalize_lowercases_then_aliases() {
    assert_eq!(normalize_lang_id("JS"), "javascript");
    assert_eq!(normalize_lang_id("Python"), "python");
    assert_eq!(normalize_lang_id("c#"), "csharp");
    assert_eq!(normalize_lang_id("zig"), "zig");
    assert_eq!(normalize_lang_id("İ"), "i\u{307}"); // Unicode lowercase, like JS
}

#[test]
fn ext_for_lang_table_then_safe_id_then_txt() {
    assert_eq!(ext_for_lang("javascript"), "js");
    assert_eq!(ext_for_lang("plaintext"), "txt");
    assert_eq!(ext_for_lang("zig"), "zig");
    assert_eq!(ext_for_lang("weird-id!"), "txt");
    assert_eq!(ext_for_lang(""), "txt");
}

#[test]
fn table_sizes_pinned() {
    assert_eq!(LANG_ALIAS.len(), 26);
    assert_eq!(LANG_FENCE.len(), 6);
    assert_eq!(LANG_EXT.len(), 29);
}

#[test]
fn guards() {
    for (id, fence) in LANG_FENCE {
        assert_eq!(normalize_lang_id(fence), *id, "fence {fence} round-trips");
    }
    for (id, _) in LANG_EXT {
        for (fid, fence) in LANG_FENCE {
            assert!(
                !(id == fence && id != fid),
                "ext key {id} is a foreign fence"
            );
        }
    }
    for (k, v) in LANG_ALIAS {
        assert!(!v.is_empty() && *v == v.to_lowercase(), "alias value {v}");
        assert_ne!(k, v, "self alias {k}");
    }
    for (_, e) in LANG_EXT {
        assert!(!e.starts_with('.'), "ext {e}");
    }
}

#[test]
fn prototype_keys_miss() {
    assert_eq!(map_language_id("constructor"), "constructor");
    assert_eq!(normalize_lang_id("toString"), "tostring");
    assert_eq!(ext_for_lang("constructor"), "constructor");
    assert_eq!(ext_for_lang("__proto__"), "txt");
}
