//! THE registration list: one line per op, sorted by name. Orchestrator-owned.

use crate::registry::{OpSpec, typed};
use crate::{artifact, artifact_write, system};

pub(crate) static OPS: &[OpSpec] = &[
    OpSpec {
        name: "artifact.create",
        summary: "Create an artifact file from a model",
        handler: |c, v| typed(c, v, artifact_write::create),
    },
    OpSpec {
        name: "artifact.delete",
        summary: "Delete an artifact file",
        handler: |c, v| typed(c, v, artifact_write::delete),
    },
    OpSpec {
        name: "artifact.patch",
        summary: "Edit a title, description or block code in place",
        handler: |c, v| typed(c, v, artifact_write::patch),
    },
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
        name: "artifact.update",
        summary: "Rewrite an artifact file from a model",
        handler: |c, v| typed(c, v, artifact_write::update),
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
