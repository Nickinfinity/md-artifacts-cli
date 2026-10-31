//! THE registration list: one line per op, sorted by name. Orchestrator-owned.

use crate::registry::{OpSpec, typed};
use crate::{
    artifact, artifact_rename, artifact_write, index, index_create, migrate, naming, prefill,
    render, system, types, validate, vars, varsets, vault, write_file,
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
        name: "artifact.rename",
        summary: "Move an artifact file to a new name in its type directory",
        handler: |c, v| typed(c, v, artifact_rename::rename),
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
        name: "migrate.frontmatter.apply",
        summary: "Rewrite planned files: type: → artifactType:",
        handler: |c, v| typed(c, v, migrate::frontmatter_apply),
    },
    OpSpec {
        name: "migrate.frontmatter.plan",
        summary: "List files whose frontmatter type: needs migrating",
        handler: |c, v| typed(c, v, migrate::frontmatter_plan),
    },
    OpSpec {
        name: "naming.check",
        summary: "Check a vault file name, or derive the default one",
        handler: |c, v| typed(c, v, naming::check),
    },
    OpSpec {
        name: "naming.output",
        summary: "The output file name a whole-file artifact would write",
        handler: |c, v| typed(c, v, naming::output),
    },
    OpSpec {
        name: "render.text",
        summary: "Fill tokens in text with values (no artifact)",
        handler: |c, v| typed(c, v, render::text),
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
        name: "vars.file.new",
        summary: "Create a blank Variables file",
        handler: |c, v| typed(c, v, vars::file_new),
    },
    OpSpec {
        name: "vars.list",
        summary: "List Variables files with their sub-sets and variables",
        handler: |c, v| typed(c, v, vars::list),
    },
    OpSpec {
        name: "vars.meta.tags",
        summary: "Replace a Variables file's tags",
        handler: |c, v| typed(c, v, vars::meta_tags),
    },
    OpSpec {
        name: "vars.search",
        summary: "Search Variables files, sub-sets and variables",
        handler: |c, v| typed(c, v, vars::search),
    },
    OpSpec {
        name: "vars.subset.add",
        summary: "Add an empty sub-set",
        handler: |c, v| typed(c, v, vars::subset_add),
    },
    OpSpec {
        name: "vars.subset.delete",
        summary: "Delete a sub-set",
        handler: |c, v| typed(c, v, vars::subset_delete),
    },
    OpSpec {
        name: "vars.subset.describe",
        summary: "Set a sub-set's description",
        handler: |c, v| typed(c, v, vars::subset_describe),
    },
    OpSpec {
        name: "vars.subset.rename",
        summary: "Rename a sub-set",
        handler: |c, v| typed(c, v, vars::subset_rename),
    },
    OpSpec {
        name: "vars.var.add",
        summary: "Add a variable",
        handler: |c, v| typed(c, v, vars::var_add),
    },
    OpSpec {
        name: "vars.var.delete",
        summary: "Delete a variable",
        handler: |c, v| typed(c, v, vars::var_delete),
    },
    OpSpec {
        name: "vars.var.rename",
        summary: "Rename a variable",
        handler: |c, v| typed(c, v, vars::var_rename),
    },
    OpSpec {
        name: "vars.var.set",
        summary: "Set a variable's value",
        handler: |c, v| typed(c, v, vars::var_set),
    },
    OpSpec {
        name: "varsets.apply",
        summary: "Merge a sub-set's values over the client's",
        handler: |c, v| typed(c, v, varsets::apply),
    },
    OpSpec {
        name: "varsets.match",
        summary: "Rank variable sub-sets against an artifact",
        handler: |c, v| typed(c, v, varsets::match_),
    },
    OpSpec {
        name: "varsets.save",
        summary: "Save values as a new Variables file",
        handler: |c, v| typed(c, v, varsets::save),
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
    OpSpec {
        name: "vault.validate",
        summary: "Check every artifact file; report errors and pending migrations",
        handler: |c, v| typed(c, v, validate::validate),
    },
];
