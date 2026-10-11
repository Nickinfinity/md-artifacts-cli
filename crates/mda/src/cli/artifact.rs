//! `mda artifact …`: the subcommands and how each becomes an op request. Write inputs (model,
//! code, values) are read here, client side; the op does the vault I/O.

use std::path::PathBuf;

use mda_ops::OpError;
use serde_json::{Value, json};

use super::input::{self, WriteFileArgs, bad_request, patch_edit, read_input};

/// `mda artifact …`
#[derive(clap::Subcommand)]
pub enum ArtifactCmd {
    /// Parse one artifact file.
    #[command(name = "show", alias = "read")]
    Read {
        /// Vault-relative path, `<TypeDir>/<rel>.md`.
        path: String,
    },
    /// List one level of a type directory.
    #[command(name = "ls", alias = "tree")]
    Tree {
        /// The artifact type, exact spelling (`Snippet`, `AIPrompt`, …).
        #[arg(value_name = "TYPE")]
        artifact_type: String,
        /// A sub-directory of the type directory.
        dir: Option<String>,
    },
    /// Create an artifact file from a model (JSON file, `-` = stdin); never overwrites.
    #[command(name = "new", alias = "create")]
    Create {
        /// Vault-relative path, `<TypeDir>/<rel>.md`.
        path: String,
        /// The `ArtifactModel` JSON file, or `-` for stdin.
        #[arg(long)]
        model: String,
    },
    /// Rewrite an artifact file from a model, when its hash is `--hash`.
    Update {
        /// Vault-relative path.
        path: String,
        /// The `ArtifactModel` JSON file, or `-` for stdin.
        #[arg(long)]
        model: String,
        /// The content hash last read (`artifact show --json`).
        #[arg(long)]
        hash: String,
    },
    /// Edit a title, description or one block's code in place, when the hash is `--hash`.
    #[command(group(clap::ArgGroup::new("edit").required(true)))]
    Patch {
        /// Vault-relative path.
        path: String,
        /// The content hash last read.
        #[arg(long)]
        hash: String,
        /// New title (`""` removes it).
        #[arg(long, group = "edit")]
        title: Option<String>,
        /// New description (`""` removes it).
        #[arg(long, group = "edit")]
        description: Option<String>,
        /// Block index (as `artifact show` lists them); needs `--heading` and `--code-file`.
        #[arg(long, requires_all = ["heading", "code_file"])]
        block: Option<usize>,
        /// The heading of `--block`.
        #[arg(long, requires = "block")]
        heading: Option<String>,
        /// The new code (file, or `-` for stdin); without `--block`, the single body.
        #[arg(long = "code-file", group = "edit")]
        code_file: Option<String>,
    },
    /// Render an artifact (or one block) with variable values.
    Render {
        /// Vault-relative path.
        path: String,
        /// Block index (as `artifact show` lists them).
        #[arg(long)]
        block: Option<usize>,
        /// Unsaved code to render instead of the stored block (file, or `-` for stdin).
        #[arg(long = "code-file")]
        code_file: Option<String>,
        /// Values as a JSON object (file, or `-` for stdin).
        #[arg(long)]
        values: Option<String>,
    },
    /// Render a whole-file artifact (Template, AIAgentsConfig) into a workspace folder.
    #[command(name = "write-file")]
    WriteFile {
        /// Vault-relative path.
        path: String,
        /// The workspace root (made absolute); every write stays inside it.
        #[arg(long)]
        workspace: PathBuf,
        /// Destination folder, workspace-relative (default: the root).
        #[arg(long, default_value = "")]
        dest: String,
        /// The output file name (default: from the artifact).
        #[arg(long)]
        name: Option<String>,
        /// Block index.
        #[arg(long)]
        block: Option<usize>,
        /// Values as a JSON object (file, or `-` for stdin).
        #[arg(long)]
        values: Option<String>,
        /// Overwrite: the hash of the existing file (from `file.exists`).
        #[arg(long)]
        hash: Option<String>,
    },
    /// Build an artifact model from editor, file or terminal text.
    Prefill {
        /// `selection`, `file` or `terminal`.
        #[arg(long)]
        source: String,
        /// The artifact type, exact spelling.
        #[arg(long = "type")]
        artifact_type: String,
        /// The source text (file, or `-` for stdin).
        #[arg(long = "text-file")]
        text_file: String,
        /// The editor language id.
        #[arg(long = "language-id")]
        language_id: Option<String>,
        /// The source file name (`file` source).
        #[arg(long = "file-name")]
        file_name: Option<String>,
    },
    /// Delete an artifact file, when its hash is `--hash`.
    #[command(name = "rm", alias = "delete")]
    Delete {
        /// Vault-relative path.
        path: String,
        /// The content hash last read.
        #[arg(long)]
        hash: String,
    },
}

/// `mda artifact …`: write inputs (model, code) are read here, client side; the op does the vault I/O.
pub(super) fn artifact_request(cmd: ArtifactCmd) -> (String, Result<Value, OpError>) {
    let model = |m: &str| {
        read_input(m).and_then(|t| {
            serde_json::from_str::<Value>(&t).map_err(|e| bad_request(&e.to_string()))
        })
    };
    match cmd {
        ArtifactCmd::Create { path, model: m } => (
            "artifact.create".to_owned(),
            model(&m).map(|m| json!({ "path": path, "model": m })),
        ),
        ArtifactCmd::Update {
            path,
            model: m,
            hash,
        } => (
            "artifact.update".to_owned(),
            model(&m).map(|m| json!({ "path": path, "expectedHash": hash, "model": m })),
        ),
        ArtifactCmd::Delete { path, hash } => (
            "artifact.delete".to_owned(),
            Ok(json!({ "path": path, "expectedHash": hash })),
        ),
        ArtifactCmd::Patch {
            path,
            hash,
            title,
            description,
            block,
            heading,
            code_file,
        } => {
            let edit = patch_edit(title, description, block, heading, code_file);
            let p = edit.map(|e| json!({ "path": path, "expectedHash": hash, "edit": e }));
            ("artifact.patch".to_owned(), p)
        }
        ArtifactCmd::Read { path } => ("artifact.read".to_owned(), Ok(json!({ "path": path }))),
        ArtifactCmd::Tree { artifact_type, dir } => (
            "artifact.tree".to_owned(),
            Ok(json!({ "type": artifact_type, "dir": dir })),
        ),
        cmd => w3_request(cmd),
    }
}

/// The W3 subcommands (render, write-file, prefill); kept apart so `artifact_request` stays short.
fn w3_request(cmd: ArtifactCmd) -> (String, Result<Value, OpError>) {
    match cmd {
        ArtifactCmd::Render {
            path,
            block,
            code_file,
            values,
        } => (
            "artifact.render".to_owned(),
            input::render_params(path, block, code_file, values),
        ),
        ArtifactCmd::WriteFile {
            path,
            workspace,
            dest,
            name,
            block,
            values,
            hash,
        } => {
            let args = WriteFileArgs {
                path,
                workspace,
                dest,
                name,
                block,
                values,
                hash,
            };
            (
                "artifact.write_file".to_owned(),
                input::write_file_params(args),
            )
        }
        ArtifactCmd::Prefill {
            source,
            artifact_type,
            text_file,
            language_id,
            file_name,
        } => (
            "artifact.prefill".to_owned(),
            input::prefill_params(source, artifact_type, &text_file, language_id, file_name),
        ),
        // Every other subcommand is handled by `artifact_request` before reaching here.
        _ => (
            "artifact.read".to_owned(),
            Err(bad_request("unreachable subcommand")),
        ),
    }
}
