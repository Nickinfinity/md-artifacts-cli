//! The command line: one subcommand per op, human output by default, `--json` for the exact
//! response. Stub landed by H0.0b; T0.4 implements it.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use mda_ops::{Ctx, OpError, Root};
use serde_json::{Value, json};

use crate::{debug, serve};

mod input;
pub mod print;

use input::{WriteFileArgs, bad_request, patch_edit, read_input};

/// Global flags and the subcommand.
#[derive(clap::Parser)]
#[command(name = "mda", version)]
pub struct Cli {
    /// The vault to use for this process (not persisted).
    #[arg(long, global = true)]
    pub vault: Option<PathBuf>,
    /// Print the exact op response as one JSON line.
    #[arg(long, global = true)]
    pub json: bool,
    /// Log to stderr: `--debug` or `--debug=trace`. `require_equals` keeps `--debug system ops`
    /// from reading `system` as the level.
    #[arg(long, global = true, num_args = 0..=1, require_equals = true, default_missing_value = "debug")]
    pub debug: Option<String>,
    /// Write debug output to this file instead of stderr (needs `--debug` or `MDA_LOG`).
    #[arg(long, global = true)]
    pub log_file: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Command,
}

/// Top-level subcommands: one per ops group, plus `serve`.
#[derive(clap::Subcommand)]
pub enum Command {
    /// Serve JSON-RPC 2.0 over stdio, one JSON object per line.
    Serve,
    /// Read, list and write artifacts.
    #[command(subcommand)]
    Artifact(ArtifactCmd),
    /// Engine information.
    #[command(subcommand)]
    System(SystemCmd),
    /// Run any operation by dotted name with raw JSON params (default `{}`).
    Call {
        /// Dotted op name, e.g. `system.ops`.
        op: String,
        /// JSON params.
        params: Option<String>,
    },
}

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

/// `mda system …`
#[derive(clap::Subcommand)]
pub enum SystemCmd {
    /// Engine and protocol versions.
    Version,
    /// List every registered operation.
    Ops,
}

/// Parse the command line, set up logging and the context, then run `serve` or one op.
///
/// # Examples
///
/// ```no_run
/// // Parses the process arguments, so it cannot run inside a doctest.
/// let code = mda::cli::run();
/// ```
pub fn run() -> ExitCode {
    let cli = Cli::parse();
    let env = std::env::var("MDA_LOG").ok();
    let level = debug::level_from(cli.debug.as_deref(), env.as_deref());
    if let Err(e) = debug::init(level, cli.log_file.as_deref()) {
        // Exhaustive on purpose: a new variant must fail to compile here, not skip the report.
        let debug::DebugError::Io(kind) = e;
        let e = OpError::new(mda_ops::error::IO_FAILED).with("kind", kind.to_string());
        return fail(&e, cli.json);
    }
    let root = match cli.vault.as_deref().map(Root::new).transpose() {
        Ok(root) => root,
        Err(e) => return fail(&OpError::from(e), cli.json),
    };
    let ctx = Ctx::new(root);
    let Some((op, params)) = request(cli.command) else {
        return match serve::run(&ctx, std::io::stdin().lock(), std::io::stdout().lock()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::from(3),
        };
    };
    // Every subcommand goes through dispatch only: one code path, one dispatch log line.
    match params.and_then(|p| mda_ops::dispatch(&ctx, &op, p)) {
        Ok(v) if cli.json => emit(&v.to_string()),
        Ok(v) => {
            if let Some(e) = print::refuse(&op, &v) {
                return fail(&e, false);
            }
            for w in print::warnings(&v) {
                let _ = writeln!(std::io::stderr().lock(), "{w}");
            }
            emit(&print::success(&op, &v))
        }
        Err(e) => fail(&e, cli.json),
    }
}

/// The op and params a subcommand stands for; `None` for `serve`.
fn request(cmd: Command) -> Option<(String, Result<Value, OpError>)> {
    let (op, params) = match cmd {
        Command::Serve => return None,
        Command::Artifact(cmd) => return Some(artifact_request(cmd)),
        Command::System(SystemCmd::Version) => ("system.version".to_owned(), Ok(json!({}))),
        Command::System(SystemCmd::Ops) => ("system.ops".to_owned(), Ok(json!({}))),
        Command::Call { op, params } => {
            let p = match params {
                None => Ok(json!({})),
                Some(s) => serde_json::from_str(&s).map_err(|e| {
                    OpError::new(mda_ops::error::OP_BAD_REQUEST).with("reason", e.to_string())
                }),
            };
            (op, p)
        }
    };
    Some((op, params))
}

/// `mda artifact …`: write inputs (model, code) are read here, client side; the op does the vault I/O.
fn artifact_request(cmd: ArtifactCmd) -> (String, Result<Value, OpError>) {
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

/// Write one block to stdout; a broken pipe is an I/O failure.
fn emit(text: &str) -> ExitCode {
    match writeln!(std::io::stdout().lock(), "{text}") {
        Ok(()) => ExitCode::SUCCESS,
        Err(_) => ExitCode::from(3),
    }
}

/// Report an op error (`--json`: the `OpError` JSON on stdout; human: stderr) and map the exit code.
fn fail(e: &OpError, json: bool) -> ExitCode {
    if json {
        let text = serde_json::to_string(e).unwrap_or_default();
        let _ = writeln!(std::io::stdout().lock(), "{text}");
    } else {
        let _ = writeln!(std::io::stderr().lock(), "{}", print::error_line(e));
    }
    ExitCode::from(exit_code(e))
}

/// The process exit code for an op error (P-5).
///
/// # Examples
///
/// ```
/// let e = mda_ops::OpError::new("io.failed");
/// assert_eq!(mda::cli::exit_code(&e), 3);
/// ```
pub fn exit_code(err: &OpError) -> u8 {
    if err.code == mda_ops::error::IO_FAILED {
        3
    } else {
        1
    }
}
