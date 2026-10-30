//! The command line: one subcommand per op, human output by default, `--json` for the exact
//! response. Stub landed by H0.0b; T0.4 implements it.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use mda_ops::OpError;
use serde_json::{Value, json};

use crate::{debug, serve};

mod artifact;
mod english;
mod index;
mod input;
mod migrate;
mod naming;
pub mod print;
mod vars;
mod varsets;
mod vault;

/// Global flags and the subcommand.
#[derive(clap::Parser)]
#[command(name = "mda", version)]
pub struct Cli {
    /// The vault for this process (not persisted; overrides the default).
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
    Artifact(artifact::ArtifactCmd),
    /// Plan, run and create template indexes.
    #[command(subcommand)]
    Index(index::IndexCmd),
    /// Engine information.
    #[command(subcommand)]
    System(SystemCmd),
    /// Select, inspect and list vaults.
    #[command(subcommand)]
    Vault(vault::VaultCmd),
    /// Engine configuration.
    #[command(subcommand)]
    Config(vault::ConfigCmd),
    /// Artifact types in the current vault.
    #[command(subcommand)]
    Types(vault::TypesCmd),
    /// Variables files: create, edit sub-sets and variables.
    #[command(subcommand)]
    Vars(vars::VarsCmd),
    /// Variable sets: match an artifact, apply, save.
    #[command(subcommand)]
    Varsets(varsets::VarsetsCmd),
    /// Render text without an artifact.
    #[command(subcommand)]
    Render(RenderCmd),
    /// Migrate legacy vault files (plan writes nothing).
    #[command(subcommand)]
    Migrate(migrate::MigrateCmd),
    /// Check a vault file name, or derive the default one, before writing.
    #[command(subcommand)]
    Naming(naming::NamingCmd),
    /// Run any operation by dotted name with raw JSON params (default `{}`).
    Call {
        /// Dotted op name, e.g. `system.ops`.
        op: String,
        /// JSON params.
        params: Option<String>,
    },
}

/// `mda render …`
#[derive(clap::Subcommand)]
pub enum RenderCmd {
    /// Fill `<VK-…>` tokens in text (FILE or `-` = stdin) with `--values` (a JSON object file, `-` = stdin).
    Text {
        /// The text file, or `-` for stdin.
        #[arg(default_value = "-")]
        file: String,
        /// Values as a JSON object (file, or `-` for stdin).
        #[arg(long)]
        values: Option<String>,
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
    // `--vault` made absolute client side (V-17(3)); the resolver refuses relative paths.
    let flag = match cli
        .vault
        .as_deref()
        .map(|p| input::absolute(p, "vault"))
        .transpose()
    {
        Ok(f) => f,
        Err(e) => return fail(&e, cli.json),
    };
    let cfg = mda_ops::vault::config_dir(std::env::consts::OS, |k| std::env::var_os(k));
    let ctx = match mda_ops::vault::resolve(flag.as_deref().map(Path::new), cfg) {
        Ok(c) => c,
        Err(e) => return fail(&e, cli.json),
    };
    let typed = !matches!(cli.command, Command::Call { .. });
    let Some((op, params)) = request(cli.command, &ctx) else {
        return match serve::run(&ctx, std::io::stdin().lock(), std::io::stdout().lock()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::from(3),
        };
    };
    // Every subcommand goes through dispatch only: one code path, one dispatch log line.
    match params.and_then(|p| mda_ops::dispatch(&ctx, &op, p)) {
        Ok(v) if cli.json => emit_rc(&v.to_string(), rc_of(typed, &op, &v)),
        Ok(v) => {
            if let Some(e) = print::refuse(&op, &v) {
                return fail(&e, false);
            }
            for w in print::warnings(&v) {
                let _ = writeln!(std::io::stderr().lock(), "{w}");
            }
            emit_rc(&print::success(&op, &v), rc_of(typed, &op, &v))
        }
        Err(e) => fail(&e, cli.json),
    }
}

/// The op and params a subcommand stands for; `None` for `serve`. `ctx` lets a typed command run
/// a preparatory op (human `migrate … apply` plans first, M-6).
fn request(cmd: Command, ctx: &mda_ops::Ctx) -> Option<(String, Result<Value, OpError>)> {
    let (op, params) = match cmd {
        Command::Serve => return None,
        Command::Artifact(cmd) => return Some(artifact::artifact_request(cmd)),
        Command::Index(cmd) => return Some(index::index_request(cmd)),
        Command::Vault(cmd) => return Some(vault::vault_request(cmd)),
        Command::Config(cmd) => return Some(vault::config_request(cmd)),
        Command::Types(cmd) => return Some(vault::types_request(cmd)),
        Command::Vars(cmd) => return Some(vars::vars_request(cmd)),
        Command::Varsets(cmd) => return Some(varsets::varsets_request(cmd)),
        Command::Migrate(cmd) => return Some(migrate::migrate_request(cmd, ctx)),
        Command::Naming(cmd) => return Some(naming::naming_request(cmd)),
        Command::Render(RenderCmd::Text { file, values }) => (
            "render.text".to_owned(),
            input::render_text_params(&file, values),
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

/// The exit code for a successful response: [`report_exit`] for typed commands, 0 for `mda call`.
fn rc_of(typed: bool, op: &str, v: &Value) -> u8 {
    if typed { report_exit(op, v) } else { 0 }
}

/// Write one block to stdout and exit with `rc`; a broken pipe is an I/O failure.
fn emit_rc(text: &str, rc: u8) -> ExitCode {
    match writeln!(std::io::stdout().lock(), "{text}") {
        Ok(()) => ExitCode::from(rc),
        Err(_) => ExitCode::from(3),
    }
}

/// Exit code of a successful typed command: 1 for `vault.validate` with `errors > 0` (M-5), else 0.
///
/// # Examples
///
/// ```
/// assert_eq!(mda::cli::report_exit("system.ops", &serde_json::json!({})), 0);
/// ```
pub fn report_exit(op: &str, v: &Value) -> u8 {
    u8::from(op == "vault.validate" && v["errors"].as_u64() > Some(0))
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
