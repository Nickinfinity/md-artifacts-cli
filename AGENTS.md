# AGENTS.md

Guidance for AI agents working in this repository. **Trust the tree over this file** where they
disagree, and fix this file in the same change.

> **Status (2026-10-14): W5 landed (MAC-8)** — Variables-file edits: `mda-core/src/variables.rs` (sub-set addressing, shape
> transitions, the name rule), ten `vars.*` ops (`mda-ops/src/vars.rs` + `vars/requests.rs`, through the shared
> `artifact_write::rewrite`), `mda vars …` (`cli/vars.rs`), the emitter's 64 KiB vks body cap, conformance kind `idempotent`
> (5 cases). W5b (listing, search, var sets, `render.text`) follows. **W4 landed (MAC-7).** The workspace exists: `mda-core` (model, type registry, parse incl.
> flags and `<VK-…>` detection, the vks codec reading legacy and YAML bodies and emitting YAML, the `.md`
> serializer with its re-parse guard (`index: true` emitted last; empty whole-file bodies frontmatter-only), the
> byte-surgical patcher, the render engine `render/` with the directive engine and its four expansion limits,
> `language.rs`, `naming.rs` (incl. `slugify`), `multi_index.rs` (`safe_rel_path`, index links)), `mda-vault`
> (containment, bounded reads, directory listing, atomic writes with the SHA-256 content hash, vault validation +
> create-only type dirs `vault.rs`, the two config files `config.rs`), `mda-ops`
> (`system.version`, `system.ops`, `artifact.read` (+ `hash`), `artifact.tree`, `artifact.create|update|patch|delete`,
> `artifact.render|write_file|prefill`, `index.plan|run|create`, `vault.inspect|select|list|use`, `config.get`,
> `types.list|set_enabled`, the session resolver), `mda` (CLI incl. `mda call`,
> `mda artifact show|ls|new|update|patch|rm|render|write-file|prefill`, `mda index plan|run|create`, `mda vault
> inspect|select|list|use`, `mda config get`, `mda types list|set-enabled`, `serve` with
> `initialize.types`, debug mode), the conformance harness (121 `parse` + 28 `serialize` + 40 `render` cases) and the
> op-case harness (per-leg fixture copies, `expect_files`, per-leg workspace + `expect_workspace_files`, per-leg
> machine config dir + `machine_config`/`expect_config_files`/`vault_flag`).
> Sections marked *(W0)* describe the tree; everything else (variables, migration) is the **target**,
> built by the engine-port plan's W4–W7 (`docs/plans/engine-port/`).
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
│   │       │   └── text.rs    # JS string semantics (trim, line split) + decode
│   │       ├── vks/           # {mod, classify, legacy, yaml, value, emit}.rs — the vks codec (read both dialects, emit YAML)
│   │       ├── serialize.rs   # THE .md emitter
│   │       ├── patch.rs       # surgical in-place edits
│   │       ├── render/        # token substitution + the directive engine (paths, each/end, join, limits)
│   │       ├── naming.rs      # slugify, file names, whole-file output names
│   │       ├── multi_index.rs # index links, safe_rel_path (rejection authority)
│   │       ├── varset.rs      # scoring, apply, save (W5b — not yet)
│   │       ├── variables.rs   # Variables-file edits: sub-sets, vars, name rule, shapes (W5)
│   │       ├── migrate/       # frontmatter rewrite (vks rewrite later)
│   │       └── error.rs       # vks.* code constants + VarsError
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
│   │       ├── render.rs · write_file.rs · prefill.rs  # artifact.render / write_file / prefill (W3)
│   │       ├── index.rs · index_create.rs  # index.plan / run / create (W3b)
│   │       ├── vars.rs        # vars.file/meta/subset/var edits (W5)
│   │       └── <group>.rs     # varsets (W5b) · migrate · validate
│   ├── mda/                   # lib + bin: `mda`
│   │   └── src/
│   │       ├── main.rs        # 3 lines: `mda::cli::run()`
│   │       ├── lib.rs         # pub mod cli · serve · debug
│   │       ├── cli/           # clap derive, one enum per ops group (`artifact.rs`, `index.rs`, `vault.rs`), `mda call`; human printer + --json; exit codes
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

### Configuration *(E-8…E-10; W4, spec §12)*

Two engine-owned scopes, written only through ops: the **machine file** `<config dir>/config.toml`
(known vaults ≤ 64 and the default) and **`<vault>/.mda/config.toml`** (`[types.<Type>] enabled`;
travels with the vault). `<config dir>` = absolute `MDA_CONFIG_DIR`, else the OS dir (macOS
`~/Library/Application Support/mda`, Windows `%APPDATA%\mda`, else `$XDG_CONFIG_HOME/mda` or
`~/.config/mda`); a relative override never falls back. Clients pick folders (the engine has no GUI);
the engine validates (`validate_vault`: exists, a directory, `.obsidian/` a directory) and persists.

- **One resolver** (`mda_ops::vault::resolve`), used by the CLI and `serve`: `--vault` (one process,
  never persisted, fatal if invalid) > a valid stored default > none. A stale or corrupt default
  **never aborts startup**: vault-needing ops return `vault.unavailable` / `config.invalid`.
  `vault.select` / `vault.use` switch the session's vault (`Ctx.root` is a `RefCell`).
- **Directories are create-only** (`create_type_dir`): enabling a type or selecting a vault creates
  missing type dirs; nothing ever deletes one (a deliberate deviation from the extension's toggle).
  Whether a type is enabled changes no op — it is client UI state.
- **Config files are untrusted:** 64 KiB cap before parsing, unknown keys preserved on write,
  corrupt files refused and never overwritten, writes hash-checked (`file.conflict` on a race).
  A machine `config.toml` symlinked out of its dir is refused (a symlinked dir is fine).
- **Tests never touch the real user config:** `common::bin()` sets an uncreated per-spawn
  `MDA_CONFIG_DIR`; in-process tests use `Ctx::with_config_dir`; `std::env::set_var` is forbidden.

Artifact directory names are fixed (`Snippets/`, `Commands/`, `Templates/`, `AIAgentsConf/`,
`AIPrompts/`, `Variables/`).

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
| `.md` emission, frontmatter key order, the single-line rule | `mda-core/src/serialize.rs` |
| `<VK-…>` token grammar | `mda-core/src/parse/tokens.rs` (`TOKEN_PATTERN`) |
| Token substitution, directives, expansion limits | `mda-core/src/render/` (tokenizes only through `TOKEN_PATTERN`) |
| The vks codec | `mda-core/src/vks/` |
| `vks.*` codes | `mda-core/src/error.rs` (listed in `mda-ops` `ALL_CODES`) |
| JS trim / line split / decoding | `mda-core/src/parse/text.rs` |
| Slugs and file names | `mda-core/src/naming.rs` |
| Vault-authored relative paths (index links, `paths:`) | `mda-core/src/multi_index.rs` — `safe_rel_path` |
| Variables-file edits (addressing, shape transitions, names) | `mda-core/src/variables.rs` |
| Path containment | `mda-vault/src/contain.rs` |
| Atomic writes + content hash (`content_hash`, SHA-256) | `mda-vault/src/write.rs` |
| Configuration files | `mda-vault/src/config.rs` |
| Vault validation, type-dir creation | `mda-vault/src/vault.rs` (`validate_vault`, `create_type_dir`) |
| Session vault resolution | `mda-ops/src/vault.rs` (`resolve`) |
| The operation list | `mda-ops/src/ops_list.rs` (registered through `registry.rs`) |
| Error codes | `mda-ops/src/error.rs` |
| NDJSON framing | `mda/src/serve/` (shared with `mda-mcp` later) |

The on-disk format is specified by **`spec/ARTIFACT_FILE_FORMAT.md`** (ported from the extension;
§9–§10, the YAML variables and directives, were ruled at the engine-port W1 Role A pass 1). **If the spec and the
parser disagree, that is a bug to reconcile, never a judgement call.**

---

## Conformance — parity with the TypeScript extension

`conformance/` (tracked) holds language-neutral cases: `parse/` (input `.md` → expected JSON),
`serialize/` (model JSON → expected bytes; kind `idempotent`: a `.md` that must serialize back byte for byte), `render/` (file + values → expected `{output, warnings, containsEscape}`) and
`manifest.toml`. TS-derived cases are generated **once** by an exporter in the extension repo from
its current services, its goldens and the real vault. `tests/conformance.rs` walks the manifest.

- **Workers never edit `conformance/`.** The orchestrator adds or regenerates cases; the reviewer
  reads the diff as a finding surface.
- A disagreement is a **recorded decision**: fix Rust, or mark the case
  `deviates_from_ts = "<reason>"`.
- Features the extension never had (vks YAML, structured values, loops) get Rust-only cases.
- **No vault file is ever committed** (the repo is public). Parity on the real vault is a local-only,
  `#[ignore]`d test: `MDA_TEST_VAULT=<vault> cargo test -p mda --test vault_local -- --ignored`, against
  TS expectations generated into gitignored `docs/`.

---

## Testing commands and client requests — op cases *(W0)*

Every op is tested **once as data, three ways**. A case is a file,
`crates/mda/tests/op_cases/<op>/<case>.toml`: the request, the fixture vault, and either the expected
response JSON (`expect`) or the expected error code (`expect_error`). One harness (`tests/op_cases.rs` + `tests/op_harness/`)
runs every case through `mda_ops::dispatch`, through `mda call <op> '<json>' --json` (exit code included), and
through a `serve` NDJSON session (asserting every stdout line is JSON).

- **Coverage guard:** every registered op needs at least one `expect` case and one `expect_error`
  case, or the suite fails. A new op without cases cannot land.
- **Human CLI output** (the non-`--json` form) is asserted per command in `tests/cli_human*.rs` (split by group) and `tests/cli_<group>.rs`.
- **Fixture vault:** `crates/mda/tests/fixtures/vault/` (with `.obsidian/`), tracked, extended per wave;
  W4 adds `vault_bare/`, `vault_cfg/`, `vault_badcfg/`, `not_vault/`.
- **Machine config per leg:** every leg gets a fresh canonical config dir (`MDA_CONFIG_DIR`);
  `machine_config` seeds it, `expect_config_files` asserts it, `vault_flag = false` copies the fixture
  without passing `--vault`; `$VAULT` / `$CONFIG` / `$WORKSPACE` are substituted. `expect` is full-body equality.
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
  checked **before** the work they bound. Render bounds output bytes, iterations, steps (nodes
  visited) and loop nesting (spec §10), so empty loops and repeated `join` walks are bounded too.
- **One containment rule** (`contain.rs`): canonicalize (symlinks resolved) and compare path
  **components**. Checked immediately before the I/O it protects. Vault writes are contained to the
  vault root; workspace writes (whole-file artifacts, scaffolds) to an allowed root the client
  passes, never one derived from vault content, and through the **same** `write.rs` path as vault
  writes (atomic, UTF-8, no BOM). Every output-name input (typed name, `extension:`, `target:`, the
  title fallback) is a path-injection surface (`naming.rs`).
- **Atomic writes** only; a crash leaves the old file or the new one.
- **No panics on input:** `#![forbid(unsafe_code)]`; `unwrap`/`expect`/`panic!` denied outside tests.
- **stdout discipline** in `serve`/`mcp` (above): a stray byte breaks every client.
- **Terminal safety for rendered output:** clients that send rendered text to a terminal refuse
  content containing `ESC` (`\x1b`); the engine reports it in the render response (`containsEscape`),
  so every client can apply the same rule. The CLI's human `render` refuses it
  (`render.contains_escape`); `--json` returns the response unchanged.
- **MCP write tools are off unless `--allow-write`** (when MCP exists): at least `artifact.create|update|patch|delete`,
  `artifact.write_file`, `index.run|create`, `vault.select|use`, `types.set_enabled`, `vars.file.new`, `vars.meta.tags`,
  `vars.subset.*`, `vars.var.*` (`varsets.save` at W5b).

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
