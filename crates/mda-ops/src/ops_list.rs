//! THE registration list: one line per op, sorted by name. Orchestrator-owned.

use crate::registry::{OpSpec, typed};
use crate::{artifact, system};

pub(crate) static OPS: &[OpSpec] = &[
    OpSpec {
        name: "artifact.read",
        summary: "Parse one artifact file",
        handler: |c, v| typed(c, v, artifact::read),
    },
    OpSpec {
        name: "artifact.tree",
        summary: "List one level of a type directory",
        handler: |c, v| typed(c, v, artifact::tree),
    },
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
