//! THE registration list: one line per op, sorted by name. Orchestrator-owned.

use crate::registry::{OpSpec, typed};
use crate::{
    artifact, artifact_write, index, index_create, prefill, render, system, types, vault,
    write_file,
};

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
        name: "artifact.prefill",
        summary: "Build an artifact model from editor, file or terminal text",
        handler: |c, v| typed(c, v, prefill::prefill),
    },
    OpSpec {
        name: "artifact.read",
        summary: "Parse one artifact file",
        handler: |c, v| typed(c, v, artifact::read),
    },
    OpSpec {
        name: "artifact.render",
        summary: "Render a block with variable values",
        handler: |c, v| typed(c, v, render::render),
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
        name: "artifact.write_file",
        summary: "Render a whole-file artifact into a workspace file",
        handler: |c, v| typed(c, v, write_file::write_file),
    },
    OpSpec {
        name: "config.get",
        summary: "The session vault, its source and the config dir",
        handler: |c, v| typed(c, v, vault::get),
    },
    OpSpec {
        name: "index.create",
        summary: "Scaffold sibling artifacts and an index linking them",
        handler: |c, v| typed(c, v, index_create::create),
    },
    OpSpec {
        name: "index.plan",
        summary: "Resolve a template index into steps",
        handler: |c, v| typed(c, v, index::plan),
    },
    OpSpec {
        name: "index.run",
        summary: "Write every step of a template index into a workspace",
        handler: |c, v| typed(c, v, index::run),
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
    OpSpec {
        name: "types.list",
        summary: "Each artifact type: dir, enabled, dir exists",
        handler: |c, v| typed(c, v, types::list),
    },
    OpSpec {
        name: "types.set_enabled",
        summary: "Enable or disable a type in the current vault",
        handler: |c, v| typed(c, v, types::set_enabled),
    },
    OpSpec {
        name: "vault.inspect",
        summary: "Inspect a folder as a vault (reads only)",
        handler: |c, v| typed(c, v, vault::inspect),
    },
    OpSpec {
        name: "vault.list",
        summary: "Known vaults and the default",
        handler: |c, v| typed(c, v, vault::list),
    },
    OpSpec {
        name: "vault.select",
        summary: "Validate, prepare, remember and use a vault",
        handler: |c, v| typed(c, v, vault::select),
    },
    OpSpec {
        name: "vault.use",
        summary: "Make a known vault the default and use it",
        handler: |c, v| typed(c, v, vault::use_vault),
    },
];
