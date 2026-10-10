//! The command line: one subcommand per op, human output by default, `--json` for the exact
//! response. Stub landed by H0.0b; T0.4 implements it.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use mda_ops::{Ctx, OpError, Root};
use serde_json::{Value, json};

use crate::{debug, serve};

pub mod print;

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
    /// Read and list artifacts.
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
        Ok(v) => {
            let text = if cli.json {
                v.to_string()
            } else {
                print::success(&op, &v)
            };
            emit(&text)
        }
        Err(e) => fail(&e, cli.json),
    }
}

/// The op and params a subcommand stands for; `None` for `serve`.
fn request(cmd: Command) -> Option<(String, Result<Value, OpError>)> {
    let (op, params) = match cmd {
        Command::Serve => return None,
        Command::Artifact(ArtifactCmd::Read { path }) => {
            ("artifact.read".to_owned(), Ok(json!({ "path": path })))
        }
        Command::Artifact(ArtifactCmd::Tree { artifact_type, dir }) => (
            "artifact.tree".to_owned(),
            Ok(json!({ "type": artifact_type, "dir": dir })),
        ),
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
