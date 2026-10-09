//! The `system.*` ops. Stubs landed by H0.0; T0.2 implements them.

use serde::{Deserialize, Serialize};

use crate::{Ctx, OpError};

/// `system.version` takes no params.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VersionRequest {}

/// The engine and protocol versions.
#[derive(Serialize)]
pub struct VersionResponse {
    pub engine: String,
    pub protocol: String,
}

/// `system.ops` takes no params.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ListOpsRequest {}

/// Every registered op.
#[derive(Serialize)]
pub struct ListOpsResponse {
    pub ops: Vec<OpInfo>,
}

/// One op's name and summary.
#[derive(Serialize)]
pub struct OpInfo {
    pub name: String,
    pub summary: String,
}

/// `system.version`: the engine and protocol versions.
///
/// # Examples
///
/// ```
/// let v = mda_ops::system::version(&mda_ops::Ctx::new(None), mda_ops::system::VersionRequest {}).unwrap();
/// assert_eq!(v.protocol, mda_ops::PROTOCOL);
/// ```
pub fn version(_ctx: &Ctx, _req: VersionRequest) -> Result<VersionResponse, OpError> {
    Ok(VersionResponse {
        engine: env!("CARGO_PKG_VERSION").to_owned(),
        protocol: crate::PROTOCOL.to_owned(),
    })
}

/// `system.ops`: every registered op, in registry order.
///
/// # Examples
///
/// ```
/// let r = mda_ops::system::list_ops(&mda_ops::Ctx::new(None), mda_ops::system::ListOpsRequest {}).unwrap();
/// assert_eq!(r.ops.len(), mda_ops::ops().len());
/// ```
pub fn list_ops(_ctx: &Ctx, _req: ListOpsRequest) -> Result<ListOpsResponse, OpError> {
    let ops = crate::ops()
        .iter()
        .map(|o| OpInfo {
            name: o.name.to_owned(),
            summary: o.summary.to_owned(),
        })
        .collect();
    Ok(ListOpsResponse { ops })
}
