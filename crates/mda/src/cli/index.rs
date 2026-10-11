//! `mda index …`: the template-index subcommands and the requests they stand for.

use std::path::PathBuf;

use mda_ops::OpError;
use serde_json::{Map, Value, json};

use super::input::{absolute, read_array, read_object};

/// `mda index …`
#[derive(clap::Subcommand)]
pub enum IndexCmd {
    /// Resolve a template index's links into steps with destination candidates (reads only).
    Plan {
        /// Vault-relative path of the index, `<TypeDir>/<rel>.md`.
        path: String,
        /// The workspace folder the run starts from (mirrored destinations); default: the root.
        #[arg(long = "mirror-from", default_value = "")]
        mirror_from: String,
    },
    /// Write every step's file into the workspace; existing files are skipped.
    Run {
        /// Vault-relative path of the index.
        path: String,
        /// The workspace root (made absolute); every write stays inside it.
        #[arg(long)]
        workspace: PathBuf,
        /// The steps as a JSON array of `{relPath, destDir, fileName?}` (file, or `-` for stdin).
        #[arg(long)]
        steps: String,
        /// Values as a JSON object (file, or `-` for stdin), applied to every step.
        #[arg(long)]
        values: Option<String>,
    },
    /// Scaffold one sibling artifact per path plus an index linking them.
    Create {
        /// The whole-file type, exact spelling (`Template`, `AIAgentsConfig`).
        #[arg(long = "type")]
        artifact_type: String,
        /// The vault folder to create in, under the type directory.
        #[arg(long)]
        dir: String,
        /// The paths the siblings stand for (e.g. `src/Button.tsx`).
        #[arg(required = true)]
        paths: Vec<String>,
    },
}

/// The op and params an `mda index` subcommand stands for.
pub(super) fn index_request(cmd: IndexCmd) -> (String, Result<Value, OpError>) {
    match cmd {
        IndexCmd::Plan { path, mirror_from } => (
            "index.plan".to_owned(),
            Ok(json!({ "path": path, "mirrorFrom": mirror_from })),
        ),
        IndexCmd::Run {
            path,
            workspace,
            steps,
            values,
        } => (
            "index.run".to_owned(),
            run_params(path, &workspace, &steps, values),
        ),
        IndexCmd::Create {
            artifact_type,
            dir,
            paths,
        } => (
            "index.create".to_owned(),
            Ok(json!({ "type": artifact_type, "dir": dir, "paths": paths })),
        ),
    }
}

fn run_params(
    path: String,
    workspace: &std::path::Path,
    steps: &str,
    values: Option<String>,
) -> Result<Value, OpError> {
    let mut p = Map::new();
    p.insert("path".into(), json!(path));
    p.insert("workspaceRoot".into(), json!(absolute(workspace)?));
    p.insert("steps".into(), read_array(steps)?);
    if let Some(f) = values {
        p.insert("values".into(), read_object(&f)?);
    }
    Ok(Value::Object(p))
}
