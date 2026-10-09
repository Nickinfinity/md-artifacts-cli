//! THE registration list: one line per op, sorted by name. Orchestrator-owned.

use crate::registry::{OpSpec, typed};
use crate::system;

pub(crate) static OPS: &[OpSpec] = &[
    OpSpec {
        name: "system.ops",
        summary: "List every registered operation",
        handler: |c, v| typed(c, v, system::list_ops),
    },
    OpSpec {
        name: "system.version",
        summary: "Engine and protocol versions",
        handler: |c, v| typed(c, v, system::version),
    },
];
