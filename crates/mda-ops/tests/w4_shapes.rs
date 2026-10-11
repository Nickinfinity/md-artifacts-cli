//! W4 request/response shapes (H4.0): camelCase, `deny_unknown_fields`, defaults, optional fields.

use mda_ops::types::{ListTypesRequest, SetEnabledRequest};
use mda_ops::vault::{
    ConfigGetRequest, ConfigGetResponse, InspectRequest, ListRequest, SelectRequest, UseRequest,
};
use mda_ops::{OpError, Source};
use serde_json::json;

fn rejects<T: serde::de::DeserializeOwned>(v: serde_json::Value) {
    assert!(serde_json::from_value::<T>(v.clone()).is_err(), "{v}");
}

#[test]
fn select_request_defaults_create() {
    let r: SelectRequest = serde_json::from_value(json!({"path": "/v"})).unwrap();
    assert!(r.create_defaults);
    let r: SelectRequest =
        serde_json::from_value(json!({"path": "/v", "createDefaults": false})).unwrap();
    assert!(!r.create_defaults);
}

#[test]
fn requests_reject_unknown_fields() {
    rejects::<InspectRequest>(json!({"path": "/v", "x": 1}));
    rejects::<SelectRequest>(json!({"path": "/v", "x": 1}));
    rejects::<ListRequest>(json!({"x": 1}));
    rejects::<UseRequest>(json!({"path": "/v", "x": 1}));
    rejects::<ConfigGetRequest>(json!({"x": 1}));
    rejects::<ListTypesRequest>(json!({"x": 1}));
    rejects::<SetEnabledRequest>(json!({"type": "Command", "enabled": true, "x": 1}));
}

#[test]
fn set_enabled_type_is_exact() {
    rejects::<SetEnabledRequest>(json!({"type": "snippet", "enabled": true}));
    let r: SetEnabledRequest =
        serde_json::from_value(json!({"type": "AIPrompt", "enabled": false})).unwrap();
    assert!(!r.enabled);
}

#[test]
fn config_get_json_shape() {
    let none = ConfigGetResponse {
        vault: None,
        source: Source::None,
        config_dir: None,
        default_error: None,
    };
    assert_eq!(
        serde_json::to_value(none).unwrap(),
        json!({"source": "none"})
    );
    let full = ConfigGetResponse {
        vault: Some("/v".into()),
        source: Source::Default,
        config_dir: Some("/c".into()),
        default_error: Some(OpError::new("vault.unavailable").with("path", "/d")),
    };
    assert_eq!(
        serde_json::to_value(full).unwrap(),
        json!({"vault": "/v", "source": "default", "configDir": "/c",
            "defaultError": {"code": "vault.unavailable", "params": {"path": "/d"}}})
    );
}

#[test]
fn source_serializes_lowercase() {
    for (s, want) in [
        (Source::Flag, "flag"),
        (Source::Default, "default"),
        (Source::None, "none"),
    ] {
        assert_eq!(serde_json::to_value(s).unwrap(), want);
    }
}
