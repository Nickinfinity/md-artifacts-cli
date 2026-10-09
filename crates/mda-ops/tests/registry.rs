#![allow(clippy::unwrap_used, clippy::expect_used)] // tests: failure is the signal

use mda_ops::{Ctx, OpError, OpSpec, check_names, dispatch_in, ops, system, typed};
use mda_vault::VaultError;
use serde::Deserialize;
use serde_json::{Value, json};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EchoReq {
    x: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}

fn echo(_: &Ctx, r: EchoReq) -> Result<u32, OpError> {
    Ok(r.x)
}
fn empty(_: &Ctx, _: Empty) -> Result<u32, OpError> {
    Ok(7)
}

static LIST: &[OpSpec] = &[
    OpSpec {
        name: "t.echo",
        summary: "",
        handler: |c, v| typed(c, v, echo),
    },
    OpSpec {
        name: "t.empty",
        summary: "",
        handler: |c, v| typed(c, v, empty),
    },
];

fn ctx() -> Ctx {
    Ctx::new(None)
}

#[test]
fn unknown_name() {
    let e = dispatch_in(LIST, &ctx(), "nope", json!({})).unwrap_err();
    assert_eq!(e.code, "op.unknown");
    assert_eq!(e.params["name"], "nope");
}

#[test]
fn typed_ok_and_bad_request() {
    assert_eq!(
        dispatch_in(LIST, &ctx(), "t.echo", json!({"x": 1})).unwrap(),
        json!(1)
    );
    let e = dispatch_in(LIST, &ctx(), "t.echo", json!({"x": 1, "y": 2})).unwrap_err();
    assert_eq!(e.code, "op.bad_request");
    assert!(e.params.contains_key("reason"));
    assert_eq!(
        dispatch_in(LIST, &ctx(), "t.empty", Value::Null).unwrap(),
        json!(7)
    );
}

#[test]
fn names_checked() {
    let one = |name| OpSpec {
        name,
        summary: "",
        handler: |_, _| Ok(Value::Null),
    };
    assert!(
        check_names(&[one("a"), one("a")])
            .unwrap_err()
            .contains('a')
    );
    assert!(check_names(&[one("b"), one("a")]).is_err());
    assert!(check_names(ops()).is_ok());
}

#[test]
fn error_wire_shape_and_vault_mapping() {
    let e = OpError::new("path.not_found").with("path", "a");
    assert_eq!(
        serde_json::to_value(&e).unwrap(),
        json!({"code":"path.not_found","params":{"path":"a"}})
    );
    let p = || std::path::PathBuf::from("p");
    let code = |v: VaultError| OpError::from(v).code;
    assert_eq!(
        code(VaultError::OutsideRoot { path: p() }),
        "path.outside_root"
    );
    assert_eq!(code(VaultError::NotFound { path: p() }), "path.not_found");
    assert_eq!(
        code(VaultError::TooLarge {
            path: p(),
            size: 2,
            max: 1
        }),
        "file.too_large"
    );
    assert_eq!(
        code(VaultError::NotRegular { path: p() }),
        "file.not_regular"
    );
    assert_eq!(
        code(VaultError::Io {
            path: p(),
            kind: std::io::ErrorKind::Other
        }),
        "io.failed"
    );
}

#[test]
fn system_ops() {
    let v = system::version(&ctx(), system::VersionRequest {}).unwrap();
    assert_eq!(v.protocol, mda_ops::PROTOCOL);
    assert_eq!(v.engine, env!("CARGO_PKG_VERSION"));
    let names: Vec<String> = system::list_ops(&ctx(), system::ListOpsRequest {})
        .unwrap()
        .ops
        .into_iter()
        .map(|o| o.name)
        .collect();
    let want: Vec<&str> = ops().iter().map(|o| o.name).collect();
    assert_eq!(names, want);
}

fn bad_resp(_: &Ctx, _: Empty) -> Result<std::collections::BTreeMap<(u8, u8), u8>, OpError> {
    Ok([((1, 2), 3)].into())
}

#[test]
fn unserializable_response_is_internal_error() {
    let list = [OpSpec {
        name: "t.bad",
        summary: "",
        handler: |c, v| typed(c, v, bad_resp),
    }];
    let e = dispatch_in(&list, &ctx(), "t.bad", json!({})).unwrap_err();
    assert_eq!(e.code, "op.internal");
    assert!(e.params.is_empty());
}
