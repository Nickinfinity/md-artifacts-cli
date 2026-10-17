//! The op registry machinery. Stubs landed by H0.0; T0.2 implements them. The registration list
//! itself is `ops_list.rs` (orchestrator-owned).

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::time::Instant;

use serde_json::{Value, json};

use crate::{OpError, Root, error};

/// Where the session vault came from (`config.get`).
///
/// # Examples
///
/// ```
/// assert_eq!(serde_json::to_value(mda_ops::Source::Flag).unwrap(), "flag");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// `--vault` (or a client's explicit root): one process, never persisted.
    Flag,
    /// The machine file's default, or a `vault.select`/`vault.use` in this session.
    Default,
    /// No vault.
    None,
}

/// What every op receives from its front-end: the session vault (switched by `vault.select` /
/// `vault.use`), the machine config dir, where the vault came from, and the deferred error of a
/// stale or corrupt default (V-3). `serve` is single-threaded, so `RefCell`/`Cell` suffice. Never
/// hold a `borrow()` across a call that may `borrow_mut()`.
///
/// # Examples
///
/// ```
/// let ctx = mda_ops::Ctx::new(None);
/// assert!(ctx.root.borrow().is_none());
/// assert_eq!(ctx.source.get(), mda_ops::Source::None);
/// ```
pub struct Ctx {
    pub root: RefCell<Option<Root>>,
    pub config_dir: Option<PathBuf>,
    pub source: Cell<Source>,
    pub default_error: RefCell<Option<OpError>>,
}

impl Ctx {
    /// A context for one process or session: `source` is `Flag` when `root` is given, else `None`;
    /// no machine config.
    pub fn new(root: Option<Root>) -> Self {
        let source = if root.is_some() {
            Source::Flag
        } else {
            Source::None
        };
        Self {
            root: RefCell::new(root),
            config_dir: None,
            source: Cell::new(source),
            default_error: RefCell::new(None),
        }
    }

    /// The same context with a machine config dir (the resolver; in-process tests pass a temp dir).
    ///
    /// # Examples
    ///
    /// ```
    /// let ctx = mda_ops::Ctx::new(None).with_config_dir(std::env::temp_dir().join("mda-doc-cfg"));
    /// assert!(ctx.config_dir.is_some());
    /// ```
    pub fn with_config_dir(mut self, dir: PathBuf) -> Self {
        self.config_dir = Some(dir);
        self
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
/// mean `{}`; any other non-object is `op.bad_request`.
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
    if !params.is_object() {
        // Fixed literal: never format the value (could be 16 MiB, or a secret) into the reply.
        return Err(OpError::new(error::OP_BAD_REQUEST).with("reason", "params must be an object"));
    }
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
