//! The op registry machinery. Stubs landed by H0.0; T0.2 implements them. The registration list
//! itself is `ops_list.rs` (orchestrator-owned).

use std::time::Instant;

use serde_json::{Value, json};

use crate::{OpError, Root, error};

/// What every op receives from its front-end: the active vault root, if any.
///
/// # Examples
///
/// ```
/// let ctx = mda_ops::Ctx::new(None);
/// assert!(ctx.root.is_none());
/// ```
pub struct Ctx {
    pub root: Option<Root>,
}

impl Ctx {
    /// A context for one process or session.
    pub fn new(root: Option<Root>) -> Self {
        Self { root }
    }
}

/// One registered operation: its dotted name, a one-line summary, and its JSON handler.
pub struct OpSpec {
    pub name: &'static str,
    pub summary: &'static str,
    pub handler: fn(&Ctx, Value) -> Result<Value, OpError>,
}

/// The single list of registered operations.
///
/// # Examples
///
/// ```
/// assert!(mda_ops::ops().iter().any(|o| o.name == "system.ops"));
/// ```
pub fn ops() -> &'static [OpSpec] {
    crate::ops_list::OPS
}

/// Run the op registered as `name` with JSON `params`.
///
/// # Examples
///
/// ```
/// let ctx = mda_ops::Ctx::new(None);
/// let e = mda_ops::dispatch(&ctx, "nope", serde_json::json!({})).unwrap_err();
/// assert_eq!(e.code, "op.unknown");
/// ```
pub fn dispatch(ctx: &Ctx, name: &str, params: Value) -> Result<Value, OpError> {
    dispatch_in(ops(), ctx, name, params)
}

/// [`dispatch`] over an explicit list (tests use their own). The one place the op layer logs:
/// `debug` = name, duration (`<n>µs`), `ok` or the error code only (never params: `reason` can echo
/// values); `trace` adds params and response (the sink warns that values are logged).
///
/// # Examples
///
/// ```
/// let ctx = mda_ops::Ctx::new(None);
/// let r = mda_ops::dispatch_in(mda_ops::ops(), &ctx, "system.version", serde_json::json!(null));
/// assert!(r.is_ok());
/// ```
pub fn dispatch_in(
    list: &[OpSpec],
    ctx: &Ctx,
    name: &str,
    params: Value,
) -> Result<Value, OpError> {
    log::trace!("op {name:?} params {params}");
    let start = Instant::now();
    let res = match list.iter().find(|o| o.name == name) {
        Some(op) => (op.handler)(ctx, params),
        None => Err(OpError::new(error::OP_UNKNOWN).with("name", name)),
    };
    let us = start.elapsed().as_micros();
    match &res {
        Ok(v) => {
            log::debug!("op {name:?} {us}µs ok");
            log::trace!("op {name:?} response {v}");
        }
        Err(e) => log::debug!("op {name:?} {us}µs {}", e.code),
    }
    res
}

/// Adapt a typed op `fn(&Ctx, Req) -> Result<Resp, OpError>` into a JSON handler. `null` params
/// mean `{}`.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, typed, system};
/// let v = typed(&Ctx::new(None), serde_json::Value::Null, system::version).unwrap();
/// assert_eq!(v["protocol"], mda_ops::PROTOCOL);
/// ```
pub fn typed<Req: serde::de::DeserializeOwned, Resp: serde::Serialize>(
    ctx: &Ctx,
    params: Value,
    f: fn(&Ctx, Req) -> Result<Resp, OpError>,
) -> Result<Value, OpError> {
    let params = if params.is_null() { json!({}) } else { params };
    let req = serde_json::from_value(params)
        .map_err(|e| OpError::new(error::OP_BAD_REQUEST).with("reason", e.to_string()))?;
    let resp = f(ctx, req)?;
    serde_json::to_value(resp).map_err(|_| OpError::new(error::OP_INTERNAL))
}

/// Check that op names are unique and sorted (so the list is binary-searchable and diffable).
///
/// # Examples
///
/// ```
/// assert!(mda_ops::check_names(mda_ops::ops()).is_ok());
/// ```
pub fn check_names(list: &[OpSpec]) -> Result<(), String> {
    for (a, b) in list
        .iter()
        .zip(list.iter().skip(1))
        .map(|(a, b)| (a.name, b.name))
    {
        if a == b {
            return Err(format!("duplicate op name: {a}"));
        }
        if a > b {
            return Err(format!("ops not sorted: {a} before {b}"));
        }
    }
    Ok(())
}
