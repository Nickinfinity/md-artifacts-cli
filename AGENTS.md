# AGENTS.md

Guidance for AI agents working in this repository. **Trust the tree over this file** where they
disagree, and fix this file in the same change.

> **Status (2026-10-09): W0 landed.** The workspace exists: `mda-core` (empty), `mda-vault` (containment +
> bounded reads), `mda-ops` (registry, `system.version`, `system.ops`), `mda` (CLI incl. `mda call`, `serve`,
> debug mode), the conformance harness (0 cases) and the op-case harness. Sections marked *(W0)* describe
> the tree; everything else (parse, write, render, vault config, variables, migration) is the **target**,
> built by the engine-port plan's W1–W7 (`docs/plans/engine-port/`).

---

## What this project is

**`mda`** is the engine for **MD Artifacts**: an Obsidian vault of reusable snippets, commands,
templates, AI agent configs, AI prompts and variable sets, stored as `.md` files. It is the **only**
component that reads or writes the vault. One binary serves every client:

| Client | How it talks to `mda` | Status |
|---|---|---|
| **Command line** (humans, scripts, CI) | `mda <command> …` — human output by default, `--json` for the exact response | **first-class from day one** |
| **VS Code extension** (`md-artifacts-snippets_and_tools-vscode`) | spawns `mda serve`: JSON-RPC 2.0, newline-delimited, over stdio | engine-port plan |
| **TUI** | `mda tui` (ratatui) — everything the extension does **except inserting into a file** | reserved, plan TBD |
| **AI agents** | `mda mcp` — Model Context Protocol over stdio | reserved, plan TBD |

**Everything is a command.** Every operation any client can perform exists once, as a typed
operation in `mda-ops`, and is reachable from the CLI. A feature that exists only in the TUI, only
over RPC or only through MCP is a defect.

---

## Branching and pull requests

**Work happens on `feature/<slug>`. Pull requests target `develop`, never `main`.** `main` is the
release branch and receives changes only by promoting `develop`. Remote:
`github.com/Nickinfinity/md-artifacts-cli`. `main` and `develop` are protected (PR required, one
code-owner approval, conversations resolved, no force-push or deletion); the sole owner's own PRs
merge with `gh pr merge --admin`.

```
feature/<slug>  ──PR──▶  develop  ──PR──▶  main
```

Commits: one per plan wave, Jira key(s) (`MAC-…`) leading the subject. **No AI attribution
anywhere**: no `Co-Authored-By` naming an AI, no "Generated with" footer, no AI wording in commits
or PR descriptions.

---

## Commands

```bash
cargo build                                   # debug build of every crate
cargo run -p mda -- <command> [--json]        # run the CLI
cargo run -p mda -- serve --vault <path>      # the JSON-RPC server (the extension's transport)

# The gate — run before every report, every commit:
cargo fmt --all --check \
  && cargo clippy --workspace --all-targets --all-features -- -D warnings \
  && cargo test --workspace --all-features
```

Toolchain: Rust **1.97.1** stable (rustc/cargo/clippy/rustfmt). **Not installed:** `cargo-deny`,
`cargo-nextest`, `cargo-insta`. Do not add them to the gate without a plan row that installs them.

---

## Workspace layout *(W0)*

```
md-artifacts-cli/
├── Cargo.toml                 # [workspace] members + [workspace.lints] + shared dependency versions
├── crates/
│   ├── mda-core/              # PURE domain — no I/O, no clock, no env, no stdout
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── model/         # ParsedArtifact, Block, Var, Frontmatter, ArtifactType … (serde)
│   │       ├── registry.rs    # THE artifact-type table (dir, contexts, writes_file, multi_block …)
│   │       ├── language.rs    # language alias / fence / extension tables
│   │       ├── parse/         # frontmatter · code fence · ## blocks · vars · tokens · flags
│   │       ├── serialize.rs   # THE .md emitter
│   │       ├── patch.rs       # surgical in-place edits
│   │       ├── render.rs      # token resolution (plain tokens; vks template engine later)
│   │       ├── naming.rs      # slugify, file names, whole-file output names
│   │       ├── multi_index.rs # index links, safe_rel_path (rejection authority)
│   │       ├── varset.rs      # scoring, sub-sets, apply, build
│   │       ├── variables.rs   # Variables-file mutations + validators
│   │       ├── migrate/       # frontmatter rewrite (vks rewrite later)
│   │       └── error.rs       # CoreError → stable codes
│   ├── mda-vault/             # ALL filesystem access
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── error.rs       # VaultError (mapped to OpError codes in mda-ops)
│   │       ├── contain.rs     # THE containment rule (canonicalize + component compare)
│   │       ├── write.rs       # atomic write (same-dir temp → fsync → rename) + content hash check
│   │       ├── read.rs        # bounded reads (size limit before reading)
│   │       ├── config.rs      # machine config (known vaults, default) + <vault>/.mda/config.toml
│   │       ├── vault.rs       # inspect / select / create default dirs (create only, never delete)
│   │       ├── listing.rs     # per-type tree, Variables files, the pane search (today's needs only)
│   │       └── migrate.rs     # plan/apply walkers (symlink-aware)
│   ├── mda-ops/               # THE command list — one typed request → response per operation
│   │   └── src/
│   │       ├── lib.rs         # PROTOCOL constant, re-exports (Ctx, OpSpec, OpError, dispatch, Root)
│   │       ├── registry.rs    # Ctx, OpSpec, dispatch, the typed adapter
│   │       ├── ops_list.rs    # THE registration list: one line per op
│   │       ├── error.rs       # OpError { code, params } — the ONLY error shape clients see
│   │       └── <group>.rs     # vault · types · artifact · render · index · vars · varsets · migrate · validate
│   ├── mda/                   # lib + bin: `mda`
│   │   └── src/
│   │       ├── main.rs        # 3 lines: `mda::cli::run()`
│   │       ├── lib.rs         # pub mod cli · serve · debug
│   │       ├── cli/           # clap derive, one enum per ops group, `mda call`; human printer + --json; exit codes
│   │       ├── serve/         # NDJSON JSON-RPC server: framing, handshake, dispatch via the op registry
│   │       └── debug.rs       # the one logger (stderr or --log-file)
│   ├── mda-tui/               # RESERVED — created by the TUI plan. ratatui/crossterm live only here
│   └── mda-mcp/               # RESERVED — created by the MCP plan
├── conformance/               # language-neutral parity fixtures (see below) — orchestrator-owned
├── docs/                      # gitignored working area (plans, references) — never committed
├── AGENTS.md
└── CREATING_A_PLAN.md         # how plans are written here
```

**Dependency direction is one-way:** `mda-core` ← `mda-vault` ← `mda-ops` ← {`mda`, `mda-tui`,
`mda-mcp`}. `mda-core` depends on no workspace crate and performs no I/O. A front-end never
imports `mda-vault` or `mda-core` for behaviour, only for types re-exported through `mda-ops`, so
every client provably runs the same code path.

**Mapping from the extension** (for porting): `src/services/*` (the vscode-free ones) →
`mda-core`; their filesystem parts and `vault.service`, the writers and the scanners →
`mda-vault`; `src/commands/*` and the panels' message handlers → `mda-ops` operations; webviews,
panes, QuickPicks → the TUI (later). `src/types/constants.ts` → `registry.rs` + `language.rs`.

---

## Architecture rules

### Ops are the API

- Each op is `fn(ctx: &Ctx, req: XRequest) -> Result<XResponse, OpError>`, registered once under a
  dotted name (`artifact.render`, `vars.var.add`). The registry is the single list consumed by the
  CLI, `serve`, and later the TUI and MCP.
- Request types use `#[serde(deny_unknown_fields)]`. Responses are plain data, never preformatted
  text.
- **`serve` exposes the registry automatically**: a new op is reachable over JSON-RPC the moment
  it is registered. The CLI gets a subcommand per op (a `--json` flag prints the response
  verbatim, which is exactly what `serve` would return).

### Errors are codes

`OpError { code: &'static str, params: BTreeMap<String, String> }`, e.g.
`{ "code": "vault.not_obsidian", "params": { "path": "…" } }`. **No prose in errors**: clients
localise (the VS Code extension in `en`/`es`; the CLI prints an English rendering from one table
in `mda/src/cli/`). Every code is listed in `mda-ops/src/error.rs` and tested for its exact JSON
shape. Codes are part of the protocol; renaming one is a breaking change.

### The protocol (`serve`, later `mcp`)

- **stdout carries protocol lines only**: one JSON object per line, flushed after each. Logs go to
  stderr. The workspace lints deny `print_stdout` and `dbg_macro`; `serve` writes through one
  writer.
- `initialize` first: the client sends its protocol version, the server answers with its own plus
  the type registry (the registry is added in W1; W0 answers `{ protocol, engine }`). A major
  mismatch is refused.
- Every write request carries the **content hash** the client last read; a mismatch returns
  `conflict` (optimistic concurrency), never an overwrite.

### Configuration *(E-8…E-10)*

Two engine-owned scopes, written only through ops: the **machine file** in the OS config dir
(known vaults and the default) and **`<vault>/.mda/config.toml`** (enabled types; travels with the
vault). Clients pick folders (the engine has no GUI); the engine validates (`.obsidian/` present)
and persists. `--vault <path>` overrides for one process without persisting. Artifact directory
names are fixed (`Snippets/`, `Commands/`, `Templates/`, `AIAgentsConf/`, `AIPrompts/`,
`Variables/`).

### Listing *(E-11)*

Only what today's features need: the per-type tree, the Variables files, var-set scoring and the
existing Variables search (titles, names, values, descriptions, tags). No general full-text search,
no persistent index. `// ponytail:` add caching only if a real vault proves slow.

---

## Single sources of truth *(W0 onward)*

Each fact lives in exactly one place. Re-implementing one is the regression this table prevents.

| Fact | Sole owner |
|---|---|
| Artifact types and their properties | `mda-core/src/registry.rs` |
| Language tables | `mda-core/src/language.rs` |
| `.md` parsing | `mda-core/src/parse/` |
| `.md` emission | `mda-core/src/serialize.rs` |
| `<VK-…>` token grammar | `mda-core/src/parse/` (one regex constant) |
| Slugs and file names | `mda-core/src/naming.rs` |
| Vault-authored relative paths (index links, `paths:`) | `mda-core/src/multi_index.rs` — `safe_rel_path` |
| Path containment | `mda-vault/src/contain.rs` |
| Atomic writes + content hash | `mda-vault/src/write.rs` |
| Configuration files | `mda-vault/src/config.rs` |
| The operation list | `mda-ops/src/ops_list.rs` (registered through `registry.rs`) |
| Error codes | `mda-ops/src/error.rs` |
| NDJSON framing | `mda/src/serve/` (shared with `mda-mcp` later) |

The on-disk format is specified by **`spec/ARTIFACT_FILE_FORMAT.md`** (ported from the extension;
§9–§10, the YAML variables and directives, were ruled at the engine-port W1 Role A pass 1). **If the spec and the
parser disagree, that is a bug to reconcile, never a judgement call.**

---

## Conformance — parity with the TypeScript extension

`conformance/` (tracked) holds language-neutral cases: `parse/` (input `.md` → expected JSON),
`serialize/` (model JSON → expected bytes), `render/` (file + values → expected text) and
`manifest.toml`. TS-derived cases are generated **once** by an exporter in the extension repo from
its current services, its goldens and the real vault. `tests/conformance.rs` walks the manifest.

- **Workers never edit `conformance/`.** The orchestrator adds or regenerates cases; the reviewer
  reads the diff as a finding surface.
- A disagreement is a **recorded decision**: fix Rust, or mark the case
  `deviates_from_ts = "<reason>"`.
- Features the extension never had (vks YAML, structured values, loops) get Rust-only cases.

---

## Testing commands and client requests — op cases *(W0)*

Every op is tested **once as data, three ways**. A case is a file,
`crates/mda/tests/op_cases/<op>/<case>.toml`: the request, the fixture vault, and either the expected
response JSON (`expect`) or the expected error code (`expect_error`). One harness (`tests/op_cases.rs`)
runs every case through `mda_ops::dispatch`, through `mda call <op> '<json>' --json` (exit code included), and
through a `serve` NDJSON session (asserting every stdout line is JSON).

- **Coverage guard:** every registered op needs at least one `expect` case and one `expect_error`
  case, or the suite fails. A new op without cases cannot land.
- **Human CLI output** (the non-`--json` form) is asserted per command in `tests/cli_human.rs`.
- **Fixture vault:** `crates/mda/tests/fixtures/vault/` (with `.obsidian/`), tracked, extended per wave.
- **Shared test helpers:** `crates/mda/tests/common/mod.rs` (spawn the binary, NDJSON session, the
  every-stdout-line-is-JSON check), orchestrator-owned; test files import it with `mod common;`.
- The case files double as the protocol's examples in `spec/PROTOCOL.md`.

## Debug mode *(W0)*

`--debug` (op names, durations, result codes, paths touched) or `--debug=trace` (also full requests and
responses, and protocol frames in `serve`) on every command, `serve` included; `MDA_LOG=debug|trace`
does the same; `--log-file <path>` writes there instead of stderr. **Never stdout.** The library crates
log through the `log` facade; `mda/src/debug.rs` is the one logger. 🔒 `debug` never logs variable
values or file contents (they can be secrets); `trace` does, and its first line says so. The VS Code
extension enables it with a setting and shows the engine's stderr in an Output channel.

## Security — the standing threat model

Untrusted: **vault file contents and names**, every request from any client (including AI agents
through MCP), CLI arguments, TUI text inputs. Held by construction, because clippy does no taint
analysis:

- **Reject, never sanitise.** Limits (file size, nesting depth, node counts, expansion size) are
  checked **before** the work they bound.
- **One containment rule** (`contain.rs`): canonicalize (symlinks resolved) and compare path
  **components**. Checked immediately before the I/O it protects. Vault writes are contained to the
  vault root; workspace writes (whole-file artifacts, scaffolds) to an allowed root the client
  passes, never one derived from vault content.
- **Atomic writes** only; a crash leaves the old file or the new one.
- **No panics on input:** `#![forbid(unsafe_code)]`; `unwrap`/`expect`/`panic!` denied outside tests.
- **stdout discipline** in `serve`/`mcp` (above): a stray byte breaks every client.
- **Terminal safety for rendered output:** clients that send rendered text to a terminal refuse
  content containing `ESC` (`\x1b`); the engine reports it in the render response, so every client
  can apply the same rule.
- **MCP write tools are off unless `--allow-write`** (when MCP exists).

A task touching any of these surfaces is marked 🔒 in its plan; its hostile-input test asserts the
**sink** (the file outside the vault is unchanged; stdout carried only protocol lines; nothing was
written), never only the checker's return value.

---

## Dependencies

The ladder: **std → an existing workspace crate → a crate already in `Cargo.lock` → a new crate**,
with the reason recorded in the plan that adds it. Every version is pinned in the root
`[workspace.dependencies]`.

Today's `Cargo.toml` lists `ratatui` and `crossterm` for the hello-world; **W0 removes them from the
binary**, and they return only inside `mda-tui`.

---

## Code style

- `.rs` file ≤ ~400 lines (plan a split at ~500, past 700 split before adding); function ≤ ~50 lines.
- Every public item has a `///` comment with a compiling `# Examples` block (doctests run in the gate).
- Comments explain **why**. Deliberate simplifications carry `// ponytail: <ceiling> — <upgrade path>`.
- Newtypes for validated values (`VaultRoot`, `RelPath`, `VarName`); enums over strings; `TryFrom` at
  the boundary. A value past the boundary is already valid.
- One `thiserror` enum per crate boundary; no `anyhow` (`main.rs` is three lines calling `mda::cli::run()`).
- Workspace lints (root `Cargo.toml`): `unsafe_code = "forbid"`; clippy `unwrap_used`, `expect_used`,
  `panic`, `print_stdout`, `dbg_macro` denied; `indexing_slicing`, `cognitive_complexity` warn (deny under
  the gate's `-D warnings`). Tests are exempted by `clippy.toml` (`allow-{unwrap,expect,panic,indexing-slicing}-in-tests`);
  a non-test helper under `tests/` opts out at file level with a reason.

---

## Writing and running plans

**Writing a plan → read `CREATING_A_PLAN.md` first.** Plans live in `docs/plans/<slug>/` (gitignored):
`plan.md`, `progress.md`, `jira-tickets.md` (Jira project `MAC`). Every plan is refined per wave by
two read-only Opus roles before dispatch; refinement and execution are human-triggered, never
chained.

**Running a plan → read the plan, and nothing else.** Every plan carries its own execution appendix.

**Sibling repos:** the VS Code extension
(`/Users/nick/D3v/Dexsys/Extensions_Plugins/MDArtifacts/md-artifacts-snippets_and_tools-vscode`)
plans its own cutover (`docs/plans/rust-engine-cutover/` there). The two repos meet at three
contracts: the format spec, `conformance/`, and the JSON-RPC protocol. Neither repo's plan edits
the other.
