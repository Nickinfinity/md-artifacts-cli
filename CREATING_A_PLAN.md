# CREATING_A_PLAN.md — md-artifacts-cli (Rust)

How a multi-agent feature plan is **written** in this repository. This file is the
**authoring process**; a plan under `docs/plans/` is one **instance** of it.

Adapted on 2026-10-09 from the VS Code extension repo's `CREATING_A_PLAN.md`
(`md-artifacts-snippets_and_tools-vscode`). The **process** is unchanged: roles, refinement
waves, ledger, Jira and self-contained plans. The **domain, gate, static analysis and testing**
sections are rewritten for a Rust workspace that ships one binary (`mda`) serving a VS Code
extension, AI agents (MCP) and a terminal UI.

> ## Read this file to *write* a plan. Never to *run* one.
>
> **An authored plan is self-contained: running it requires reading nothing but the plan and its
> two companions** (§5.2). An agent told to execute a plan that points back here spends its
> context on authoring guidance for a job already finished, and is invited to re-derive decisions
> the plan already made.
>
> The only agent that reads this file after authoring is one whose task is to **edit** it.

> **Authoring never rolls straight into execution.** When a plan is freshly created, **stop**:
> present it and wait. Do **not** dispatch Wave 0, land code, or run the gate until the user
> explicitly says to start ("run it", "go", "execute the plan"). The definition of done gates
> *readiness to run*, not *permission to run*.

---

## 1. Where plan files live, and why they never merge

```
docs/plans/<feature-slug>/
├── plan.md          # the plan: waves, tasks, contracts, gates
├── progress.md      # the ledger: one row per task, updated as work lands
└── jira-tickets.md  # epic + story specs (project key MAC)
```

`docs/` is **gitignored in this repo from the first commit** (`.gitignore`: `/docs/`). It is a
working area for one feature branch, never shipped documentation.

**Rules:**

1. Plan files are working artifacts. They are never committed.
2. Before opening a PR, `git ls-files docs` must print **nothing**. If anything under `docs/` was
   ever force-added, `git rm -r --cached docs` is the last commit before the PR.
3. Anything worth keeping is **promoted** before the plan is discarded: into `AGENTS.md` (once it
   exists), the format spec, a crate-level `//!` doc, or a `///` doc comment. If a fact exists only
   in `docs/`, it is lost by design.

Rationale: plan documents rot faster than code and read as authority when they are stale.
**Trust the tree over any plan or ledger.**

---

## 2. Agent topology

Three roles **run** a plan: one **orchestrator** (Opus, senior Rust tech lead and project
manager), one **reviewer** (Opus, review only), and N **workers** (Sonnet) in parallel. Two more
roles **refine** a plan before any of that starts (§5.3). Both are Opus and read-and-report only.

### 2.0 Prompt composition — three blocks, written once each

A dispatch prompt is **A + B + C + instance parameters**, in that order:

| Block | Scope | Changes when |
|---|---|---|
| **A — Domain** (§2.1) | this repo; identical in every role | the stack changes |
| **B — Discipline** (§2.2) | the craft bar; identical in every role | the standard changes |
| **C — Role** (§2.3) | orchestrator · reviewer · worker | the process changes |
| Instance parameters | one plan | every plan |

A prompt that opens with the role and never states the domain yields correct Rust against the
wrong platform: it compiles, passes clippy, and corrupts a vault on Windows.

### 2.1 Block A — Domain. Verbatim in every role.

> **You build a cross-platform Rust engine for a markdown vault.** One binary, `mda`, is the only
> component that touches the vault. It serves three kinds of client over **stdio**: the VS Code
> extension (`mda serve`, JSON-RPC 2.0), AI agents (`mda mcp`, Model Context Protocol), and humans
> (`mda tui`, ratatui, plus plain CLI subcommands). The extension *requires* it and does no file
> access of its own. You know this kind of program fails *quietly*: a stray `println!` that
> corrupts a protocol stream, a path that is contained on macOS and escapes on Windows, a write
> that half-lands when the process dies.
>
> Held knowledge, each item costing someone a day:
>
> - **In `serve` and `mcp` mode, stdout *is* the protocol.** Every message is one line of JSON
>   (newline-delimited). Any other byte on stdout — `println!`, `dbg!`, a panic message, a
>   dependency's log — breaks every client at once. Diagnostics go to **stderr** only, through the
>   one logging setup. Flush after every message.
> - **Vault content is untrusted**: file contents, file **names**, frontmatter, `vks` bodies,
>   template directives. So is every request a client sends, including the MCP agent's.
>   **Reject, never sanitise.** Every limit is checked **before** the work it bounds.
> - **Path containment is a rule, not a `starts_with`.** Resolve with `canonicalize` (symlinks
>   included) and compare **components**, never strings. `C:\vault` vs `c:\Vault`, `\\?\` prefixes,
>   macOS/Windows case-insensitivity and macOS NFD file names all defeat string checks. Check
>   immediately before the I/O it protects; a check followed by an `await` is a TOCTOU window.
> - **Writes are atomic**: write a temp file in the **same directory**, fsync, rename over the
>   target. A crash leaves the old file or the new one, never half of each. On Windows, rename
>   over a file another process holds open can fail; that is an error to surface, not to retry
>   blindly.
> - **Every update carries the content hash the client last read** (optimistic concurrency). A
>   mismatch is a `conflict` error, never a silent overwrite: the extension, an agent and the TUI
>   can all be writing the same vault.
> - **Line endings and encodings vary.** Vault files arrive with CRLF, BOMs and NFD names.
>   Parse them all; emit what the format spec says; round-trip what you did not mean to change.
> - **The process cannot show a GUI.** No folder dialogs, no confirmations: the *client* asks the
>   user, and the engine validates what it is given.
> - **No `unsafe`, no panics on input.** `#![forbid(unsafe_code)]` in every crate. `unwrap` /
>   `expect` / indexing that can panic are **forbidden outside tests** on any path untrusted data
>   reaches. A panic in `serve` kills every open editor's connection.
> - **The engine returns error *codes* with parameters, never prose.** Clients localise. A message
>   string in an error is a defect.
> - **The `regex` crate is linear-time; keep it that way.** No backtracking engines
>   (`fancy-regex`, PCRE) on untrusted input.
> - **MCP write tools are off unless `--allow-write`.** An agent has no confirm dialog of ours.
>
> **A dependency is a decision, not a convenience.** The ladder: std → an existing workspace crate
> → a crate already in `Cargo.lock` → a new crate, with the reason recorded in the plan.

### 2.2 Block B — Discipline. Verbatim in every role.

> **Load first, via the Skill tool: `caveman`, `ponytail`, `rust-patterns`.** They do not auto-load in
> subagents. `rust-patterns` covers ownership, error handling, traits and concurrency idioms; where it
> and this block disagree, this block wins.
>
> - **TDD.** The failing test comes first and fails for the *right reason*. **"Genuine red" means
>   *observed*.** A compile error is **not** red: stub the function to return a wrong-but-plausible
>   value (`Ok(vec![])`, `String::new()`), watch the named assertion fail, then implement.
> - **DRY.** One authority per fact. Find the existing function, type or table before writing a
>   sibling. Extending an authority beats creating one.
> - **KISS / ponytail.** The smallest thing that passes. No trait with one implementation, no
>   generic with one instantiation, no builder for a struct with three fields, no feature flag
>   for a constant.
> - **Types are the design tool.** Enums over strings and bools, newtypes for validated values
>   (`VaultPath`, `VarName`), `#[serde(deny_unknown_fields)]` on every inbound message, `TryFrom`
>   for validation at the boundary. A value that crossed the boundary is already valid.
> - **Errors:** one `thiserror` enum per crate boundary, each variant mapping to a stable error
>   **code**. No `anyhow` in library crates (only `main`). No `String` errors.
> - **Security is never traded away.** Clippy does **no taint analysis**; on filesystem, protocol
>   and terminal surfaces the reviewer's manual trace is the only check that exists.
> - **A rejection test asserts the sink**, not the checker's return: "the file outside the vault
>   still has its original bytes", "stdout received exactly one line", "nothing was written".
> - **A source-grep guard scans comment-stripped source.** If its first red hit is the file that
>   documents the rule, the guard is wrong.
> - **Size limits:** a `.rs` file stays under ~400 lines (plan a split at ~500; past 700, split
>   before adding); a function under ~50 lines. Split by concern: `mod.rs` + one file per concern.
> - **Docs:** every public item gets a `///` comment with an `# Examples` block that compiles
>   (doctests run in the gate). Comments explain **why**.
> - **Static analysis:** clippy runs in the gate at `-D warnings` and its findings are **fixed, not
>   allowed**. An `#[allow(clippy::…)]` needs a one-line reason in a comment. SonarLint
>   diagnostics, if they arrive after an edit, are fixed too. Never invoke `sonar-analyze`,
>   `mcp__sonarqube__*`, or the `sonar` CLI.
> - **Gate:** `cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features --
>   -D warnings && cargo test --workspace --all-features`.
> - **Report in `caveman` register**: findings, not narration.

### 2.3 Block C — Role. One per role; append to A + B.

**Orchestrator (Opus) — the session itself:**

> You are the ORCHESTRATOR: tech lead and PM for this plan. You direct. You implement **only**
> orchestrator-tagged rows and integration hunks — never a worker's task, not even a small one
> because a worker is slow.
>
> - **Architecture:** hold the plan's decisions against drift. Land every shared-file wire-up
>   yourself at wave close: workspace `Cargo.toml`, a crate's `lib.rs` `mod`/`pub use` lines, the
>   RPC method table, the error-code table, the conformance fixture manifest. Arbitrate
>   worker↔reviewer disagreements; your call is final and goes in the decisions table.
> - **Security is yours to guarantee, not delegate.** Name each 🔒 surface in the reviewer's
>   dispatch; never merge one on a worker's self-report.
> - **The ledger is yours alone:** statuses, gate log with test counts, review rounds, ticket keys,
>   and deviations logged the moment they happen.
> - **Commit once per wave** after the integrated gate is green, ticket id(s) leading the subject,
>   then **push**. Workers never commit. A red gate stops all dispatch.
> - **Stop and ask** at every human gate; never start a freshly authored or unrefined wave.
> - **Dispatch:** worker `Agent({subagent_type:"general-purpose", model:"sonnet",
>   run_in_background:true})`, a whole wave's spawns in **one message**. Reviewer
>   `Agent({… model:"opus"})` once per wave, continued via `SendMessage`. **Name `model` every
>   time**; `subagent_type:"fork"` is forbidden.
> - Hold every worker to its `Owns` list. Scope creep is rejected, not merged.

**Reviewer (Opus) — one per wave:**

> You are the REVIEWER. You return a verdict; you never edit code, never commit, never touch the
> ledger. Most defects hide in what a diff *doesn't* do — the missing limit, the untested error
> variant, the `unwrap` on a path that untrusted data reaches.
>
> Order, cheapest rejection first, **except security, which is always completed** and reported
> even when an earlier check failed:
> 1. **Contract:** only `Owns` touched; forbidden files, conformance fixtures and snapshots
>    untouched. Violation = instant `CHANGES`.
> 2. **TDD:** a test that fails without the change, asserting something real.
> 3. **Types:** no stringly-typed state, validation at the boundary, no `as` casts that truncate,
>    no `clone()` to dodge the borrow checker where a borrow works.
> 4. **Over-engineering:** a trait with one impl, a speculative generic, a new crate dependency
>    without a recorded reason, a reinvented helper, a file over the size limits.
> 5. **Security, unwaivable.** Trace each untrusted value to its sink. Paths must be
>    canonicalized and component-contained right before the I/O; writes atomic; limits checked
>    before work; no panic reachable from input; no non-protocol byte on stdout; MCP writes gated.
>    Name any widened surface even when you approve.
> 6. **Static analysis:** clippy clean; every `#[allow]` justified.
>
> Plus every standing finding the plan lists, each an instant `CHANGES`.
>
> Verdict, ≤ 20 lines: `APPROVE` (one line why, plus the attack-surface note when 5 applies) ·
> `CHANGES` (numbered `file:line — problem → required fix`, security ones `SEC:` first) ·
> `ESCALATE` (only after round 2 failed; an open `SEC:` always escalates).

**Worker (Sonnet) — one per task:**

> You are a WORKER implementing **one** task. You write the failing test before the fix without
> being reminded, and your diffs are small because you looked for the existing function first.
>
> Order of work: design the types → write the failing test, see it **red** → smallest
> implementation that turns it green → `cargo clippy` clean on your crate → gate your slice →
> report.
>
> **Verify every API and signature the task names against the real tree before building on it.**
> If a `Test first` assertion will not compile, that is a plan bug: report it in one line and
> stop. Never write a shim, an `#[allow]`, or a wrapper whose only job is to make the plan's
> sentence true.
>
> **Hard limits:** touch only `Owns`. Never the forbidden files, never `Not this task`. Do not
> commit. Do not edit the ledger or the ticket file. Do not add a crate dependency the task does
> not name. Answer `CHANGES` by fixing, not debating — `SEC:` first.
>
> **Report, ≤ 15 lines, in this order:** (1) the `Test first` assertion quoted, and that you saw it
> red; (2) files written, exactly `Owns`; (3) gate result with test count before → after; (4) plan
> bugs found, one line each; (5) anything in `Not this task` you left.

### 2.4 Dispatch mechanics

| Role | Spawn | Notes |
|---|---|---|
| Worker | `Agent({ subagent_type: "general-purpose", model: "sonnet", run_in_background: true, description: "<T-id> <short title>", prompt: A + B + worker block + instance params + the wave's context block + the task block verbatim })` | One call per task; **all of a wave's calls in one message** |
| Reviewer | `Agent({ subagent_type: "general-purpose", model: "opus", description: "review W<n>", prompt: A + B + reviewer block + params + context + first task block + report + diff })`, then **`SendMessage`** for every later task | One per wave; a second `Agent` call loses the sibling-task context |
| Refinement A / B | `Agent({ subagent_type: "general-purpose", model: "opus", … })` | Pre-execution, read-and-report |
| Orchestrator | the session itself | — |

A subagent spawned without `model` **inherits the parent's**: name it on every spawn.
`subagent_type: "fork"` ignores `model` entirely, so it is forbidden for workers. Skills do not
inherit; every prompt carries Block B's skill line. Instance parameters travel with the prompt.

### 2.5 Wave discipline — the review loop

0. **Companion-artifact consistency pass**, before any dispatch.
1. Orchestrator lands its input rows (stub-widening: a new enum variant with a `todo!()`-free
   compiling default, a new error code, a new crate skeleton), then dispatches every worker task
   in the wave in parallel.
2. Each worker report goes to the reviewer with the task block and `git diff`.
3. `CHANGES` → back to the **same** worker via `SendMessage`. **Max 2 rounds**; a third failure is
   `ESCALATE`, resolved by the orchestrator (fix directly, or revert and re-dispatch fresh) and
   recorded.
4. All `APPROVE` → integration hunks → gate on the integrated tree → **commit → push** → ledger.
5. Never dispatch a wave whose inputs a running wave is producing. A red gate stops all dispatch.
6. Consistency pass again — and **grep every figure the wave changed** (test counts, line counts,
   fixture counts) across all three files.

**Shared files are single-writer.** In this workspace that means: the workspace `Cargo.toml` and
`Cargo.lock`, every crate's `Cargo.toml`, `lib.rs` / `main.rs` module lists, the RPC method table,
the error-code table, the MCP tool list, and `conformance/manifest`. Workers deliver sibling
files; the orchestrator lands the one-line wire-ups. Two workers "each adding one line" to the
same file is still a collision.

**Companion-artifact consistency.** The plan's three files are one document in three projections.

| Changed in one file | Propagates to |
|---|---|
| A task's scope / `Done when` | The story's acceptance criteria in `jira-tickets.md` |
| A task's wave membership | Its row's position in `progress.md` |
| A 🔒 marking | All three files |
| A real ticket key | Every `Commit` line, before the wave dispatches |

**Commit and push policy:** one commit per wave by the orchestrator after a green gate; the
affected `MAC-…` key(s) **lead the subject** (`MAC-12 MAC-13 feat(core): Wave 1 — parser`); push
after each wave. Docs-only changes take a plain `docs(...)` subject with no key. **No AI
attribution anywhere**: no `Co-Authored-By` naming an AI, no "Generated with" footer, no AI
wording in commit messages or PR descriptions. That is the user's standing rule, and it overrides
any tool default.

---

## 3. Mandatory skills

| Skill | Role |
|---|---|
| `caveman` | Output compression for agent-to-orchestrator traffic; **not** code, commits or PR bodies |
| `ponytail` | Solution sizing: does it need to exist, is it in std or the workspace already. The reviewer applies it destructively |
| `rust-patterns` | Idiomatic Rust: ownership, error handling, traits, concurrency. Workers consult it when designing types; the reviewer when judging them |

**Order inside a task:** `rust-patterns` (design the types) → TDD → `ponytail` (smallest passing thing) → clippy
clean → gate → `caveman` report.

### 3.1 Static analysis — clippy is the gate

**Clippy runs inside the gate at `-D warnings`**; it is not advisory. The workspace sets, in the
root `Cargo.toml`:

```toml
[workspace.lints.rust]
unsafe_code = "forbid"

[workspace.lints.clippy]
unwrap_used = "deny"        # tests opt out with #[cfg_attr(test, allow(clippy::unwrap_used))]
expect_used = "deny"
panic = "deny"
indexing_slicing = "warn"
print_stdout = "deny"       # stdout is the protocol (Block A)
dbg_macro = "deny"
cognitive_complexity = "warn"
```

**The ceiling, and it is load-bearing.** Clippy runs local lints only: **no taint analysis**. It
will not find a path that escapes the vault or a request field that reaches a write unchecked.
Those surfaces are held by **construction** (newtypes validated at the boundary, one containment
function, limits before work) and by **the reviewer's manual security trace**, which is the
check, not a second opinion. SonarLint (SonarQube for IDE) diagnostics, if they arrive after an
edit, are fixed like clippy's; `sonar-analyze`, `mcp__sonarqube__*` and the `sonar` CLI are never
invoked (no server, no token).

**Not installed today (2026-10-09):** `cargo-deny` (licences + advisories), `cargo-nextest`,
`cargo-insta`. A plan may add one to the gate only after an orchestrator row installs it and
records the version; until then, the gate is §6 exactly.

---

## 4. Methodology the plan must encode

**TDD, then DRY, then KISS**, plus:

- **DRY.** Before a plan proposes a new table, enum or helper, it states which existing one it
  extends. A parallel list is rejected at review.
- **KISS / YAGNI.** No trait with one implementation, no generic with one instantiation, no config
  for a value that never changes. Speculative extension points are cut, not deferred.
- **TDD.** Every task names its test file (or `#[cfg(test)] mod tests` location) and its first
  failing assertion **in the plan**. Code reachable only by a human at a terminal names its
  **manual run path** (§6.2).
- **Domain first.** A new concept gets a named type in the domain crate (`mda-core`) before
  behaviour exists. `mda-core` performs **no I/O**: no `std::fs`, no `std::process`, no stdout.
- **Behaviour-preserving work needs a net first.** Ports from the TS extension are measured
  against the **conformance suite** (§4.1), not against memory of what TS did.
- **Security is a standing gate, not a review item.** Untrusted input: vault files and their names,
  every inbound RPC/MCP message, CLI arguments, the TUI's text inputs. The plan **marks each
  task touching one 🔒**; that marking tells the orchestrator to name the surface to the reviewer,
  the worker to write hostile-input tests, and the reviewer that its §5 check is why the task
  exists.
- **A rejection test asserts the sink.** `assert!(matches!(r, Err(_)))` proves *a* check fired.
  `assert_eq!(fs::read(outside)?, original)` cannot be satisfied by a check on the wrong value.
- **A mutation drill's value is in the assertions that refuse to go red.** "I deleted the module and
  it failed to compile" is not a red proof.
- **A source-grep guard scans comment-stripped source** and states its ceiling (a regex, not an AST).

### 4.1 The conformance suite — parity with the TS extension (decision E-6)

`conformance/` at the repo root is tracked and **language-neutral**:

```
conformance/
├── manifest.toml          # every case, its kind, its source, and whether it is TS-derived
├── parse/<case>.md        → parse/<case>.expected.json      # the parsed model
├── serialize/<case>.json  → serialize/<case>.expected.md    # exact bytes
└── render/<case>.md + <case>.values.json → <case>.expected.txt
```

- **TS-derived cases** are generated **once** by an exporter in the extension repo, from today's
  services, over the extension's goldens, its fixtures and **every file in the real vault**. Rust
  must match them byte-for-byte or JSON-equal.
- **A disagreement is a decision, never a quiet fix.** Either Rust is wrong (fix Rust), or TS was
  wrong (record a deliberate change in the plan's decisions table, and mark the case
  `deviates_from_ts = "<reason>"` in the manifest).
- **Rust-only cases** cover features TS never had (vks YAML, structured values, directives).
- **Workers never edit `conformance/`.** The orchestrator adds or regenerates cases, and the
  reviewer reads the diff as a finding surface: a regenerated expectation that "just changed"
  is the bug this suite exists to catch.
- A `cargo test` harness (`tests/conformance.rs`) walks the manifest; adding a case needs no Rust
  code.

---

## 5. Task specification format

A task is dispatchable only when every field is filled.

```markdown
### T<n> — <imperative title>

- **Owns:**      <exact paths this task may write; disjoint from its wave>
- **Reads:**     <paths it needs but must not modify>
- **Depends on:** <task ids, or `none`>
- **Test first:** <test location + the first assertion that must fail>
- **Done when:**  <observable condition — a passing assertion, not "implemented">
- **Gate:**       <the gate, plus any extra check>

Required whenever the task touches existing code:

- **Signatures:** <the exact real signatures it calls, copied from the tree with file:line>
- **Not this task:** <the adjacent work a reasonable worker would also do, and who owns it>
- **Report:**     <what comes back beyond "done">
```

**Sizing:** one task ≈ one module plus its tests. **Disjointness counts every file**:
`Cargo.toml`s, `lib.rs` module lists, test files, fixtures, snapshots, conformance cases. Two
same-wave tasks adding cases to one test file collide; give each concern its own file under
`tests/` or its own `#[cfg(test)]` module. **No task depends on a task in its own wave.**

**Write the task for a cold reader.** `Signatures` pays for itself: paste the real `pub fn` with
`file:line`. A `Test first` assertion is a claim about the tree: open the file and confirm the
field name before the plan ships. Reject a tautological `Test first`
(`assert_eq!(v.len(), 8)` passes for eight wrong entries).

**The plan is the single entry point.** It opens by naming its companions and declaring itself
their authority, and it carries an orchestrator protocol (read order, review loop, commit policy,
red-gate stop, human gates) and the instance parameters (repo path, branch, gate, forbidden files,
report caps).

### 5.2 Self-containment — the plan carries its own execution appendix

An orchestrator handed the plan needs nothing else. The plan ends with an **execution appendix**
carrying, in full: Blocks A and B, the three role blocks, the dispatch mechanics, the gate (and
why each step is in it), the review loop with the 2-round cap, the commit-and-push policy, the
skills table, the static-analysis rule, the conformance rules (§4.1) when the plan touches ported
behaviour, and the **two refinement role templates** (§5.3). **Its read order must not include
this file**, and it says so.

### 5.3 Refinement waves — a plan is not finished when it is written

Refinement runs **per wave**, before dispatch. Both roles are **Opus** and **read-and-report**:
neither edits a file. The plan is single-writer.

| Role | Runs | Answers |
|---|---|---|
| **A — Loopholes and Details Fixer** | first, and again last | "Could a cold worker read this two ways? Would this wave, done exactly as written, meet its goal? Is every claim about the tree true?" |
| **B — Tasks Optimizer and Resource Finder** | between A's passes | "What does each task touch, what existing thing must it reuse, what will it break?" — **TDD and DRY are its whole lens** |

**Role A** checks, in order: goal reachability; ambiguity (quote it, give both readings); **claims
about the tree** (every signature, path, line and quoted assertion verified; an uncompilable
`Test first` is a plan bug); tautological tests; disjointness (`Cargo.toml`s, module lists,
fixtures and conformance cases included) and same-wave dependencies; missing guards; 🔒 markings
whose hostile-input test names the **sink**; then a `QUESTIONS` block with a recommended default
each, and `READY` / `NOT READY`.

**Role B**, per task: **TDD ordering verdict** (test work stated *first*, explicitly; an updated
test named with its assertion and seen red before source changes); **DRY reuse target** (the
existing function, type, enum or module it extends, with path, or an explicit "none exists");
then the file ledger (create / modify / read-only); real signatures with `file:line`; blast radius
(every test, snapshot, conformance case and doctest turned red, and whether expected); **the
trap** (one sentence); sizing. It does not redesign the wave or add scope.

**The cycle, per wave:** drafted → A → author applies + human answers → B → author pastes → A again →
`ready`. A's second pass is not a formality: added detail is when a task quietly grows a second
owner.

**Human-triggered, agent-requested.** The authoring agent stops after each draft or application
and **asks** for the next pass by name and scope. It never spawns a refinement agent on its own
initiative, never chains A → B → A, and never treats `READY` as permission to run.

**The refinement status table is a dispatch gate**, carried in the plan and mirrored in
`progress.md`. No wave is dispatchable while its row reads anything but `ready`.

```markdown
| Wave | Role A pass 1 | Author applied | Role B | Role A pass 2 | Status |
|------|---------------|----------------|--------|---------------|--------|
| W0   | ☐             | ☐              | ☐      | ☐             | drafted |
```

Both refinement templates are copied into the plan's appendix, each opening with Blocks A and B.

---

## 6. The gate

```bash
cargo fmt --all --check \
  && cargo clippy --workspace --all-targets --all-features -- -D warnings \
  && cargo test --workspace --all-features
```

- `cargo fmt --check` first: cheapest rejection, and formatting churn hides real diffs.
- `clippy -D warnings` with the workspace lints of §3.1. This is the static-analysis gate.
- `cargo test` runs unit tests, integration tests under `tests/`, **doctests**, and the
  conformance harness.
- Unlike the TypeScript repo, there is no orphaned-output problem (`cargo` rebuilds from source),
  so no `rm -rf` step is needed. **Do** run `cargo test` from a clean `git status`: an untracked
  fixture file that the manifest does not list is a test that silently does not run.
- Record the **test count** on every gate run (`cargo test` prints per-target counts; sum them).
  A silent drop means a test was deleted.

### 6.1 Testing the protocol surfaces

- **`serve` / `mcp`:** integration tests in `tests/` spawn the real binary
  (`env!("CARGO_BIN_EXE_mda")`), write NDJSON requests to its stdin, and assert on the lines
  read back. **Every such test also asserts that stdout carried nothing but protocol lines.** That
  is the guard for Block A's first item.
- **Error codes:** one test per code asserting the exact JSON shape (`{ "code": …, "params": … }`).
  Clients depend on that shape.
- **CLI:** spawn the binary, assert exit code + stdout + stderr.

### 6.2 Testing the TUI

- **Rendering:** ratatui's `TestBackend` renders a frame into a `Buffer`; assert on the buffer's
  text (a `snapshot` helper that formats the buffer as lines is enough — adding `insta` is a
  dependency decision under the ladder).
- **Behaviour:** drive the app's state machine with synthetic key events (`KeyEvent`) and assert
  on state and the next frame. Keep input handling a pure `fn update(state, event) -> state`, so
  it is testable without a terminal.
- **Manual run path:** what the extension repo calls an F5 pass. Each TUI wave names the exact
  key path (`cargo run -- tui --vault <fixture-vault>` → `/` → type `api` → `Enter` → …) and the
  expected screen. "Run it and check it works" is not a test.

---

## 7. Progress tracking

`progress.md` is the single ledger, written by the orchestrator only:

```markdown
| Task | Owner | Status | Test count | Gate | Notes |
|------|-------|--------|-----------|------|-------|
| T1   | wave-1 | done  | 41 → 58   | pass | — |
```

Statuses: `todo` · `wip` · `done` · `blocked` · `dropped` (with the reason). Record the test
count on every gate run.

---

## 8. Jira

Jira project **`MAC` — "MD Artifacts CLI"** on `dexsys.atlassian.net` (created by the user in the
Jira UI; the connector creates issues, not projects). Each phase is an **epic**; each task or
cluster a **story**. `jira-tickets.md` is written first; **the issues are created as the last
authoring step**, before the first wave, so every `Commit` line carries real keys from wave one.
A `<KEY>` placeholder is only fixable by rewriting a published commit, which gets worse with every
wave. Never fabricate a key. If the connector is unavailable, authoring stops and the blocker is
reported; no wave that would commit a placeholder is dispatchable.

The user may declare a plan **ticket-less**: then no `jira-tickets.md`, no id prefix on commits,
and §1 of the plan records `Ticketing: None — declined on <date>`.

Documentation-only changes need no ticket and take a plain `docs(...)` subject.

**The PR description lists the affected tickets** (the epic and every story delivered).

**Cross-repo plans.** Work that lands in the VS Code extension (the cutover) is planned **in that
repo**, under its own `CREATING_A_PLAN.md`, with its own tickets. This repo's plans may name
extension-side work only as a dependency, never as a task.

---

## 9. Definition of done for a plan

- [ ] Every phase names the **existing** authority it extends, not a new parallel one.
- [ ] Every task has all six fields from §5; existing-code tasks have `Signatures`.
- [ ] Every wave's tasks own disjoint files — **`Cargo.toml`s, module lists, test files,
      fixtures and conformance cases included**.
- [ ] No task depends on a task in its own wave.
- [ ] The plan names its companions, declares itself their authority, and carries the
      orchestrator protocol and instance parameters.
- [ ] **Self-contained (§5.2):** the execution appendix carries Blocks A + B, all five role blocks,
      dispatch mechanics, the gate, the review loop, the commit policy, skills and the
      static-analysis rule. Its read order excludes this file, and says why.
- [ ] Shared-file edits (§2.5's list) are orchestrator hunks, never worker tasks.
- [ ] Every task touching untrusted input is **🔒** with a hostile-input `Test first` naming the
      **sink**, and its gate names the reviewer's manual security trace.
- [ ] Every `mda-core` task names a test and a first failing assertion; every TUI task names its
      manual run path.
- [ ] Every new crate dependency carries its ladder justification.
- [ ] Ported behaviour is covered by conformance cases (§4.1), and every deliberate deviation from
      TS is recorded.
- [ ] Any on-disk format change updates the format spec **in the same change**.
- [ ] Deliberate simplifications carry a `// ponytail:` comment naming the ceiling and the upgrade
      path.
- [ ] `progress.md` exists with every task at `todo` and the refinement table at `drafted`.
- [ ] Ticketing settled: keys created and on every `Commit` line, or `Ticketing: None` recorded.
- [ ] Per-wave commit **and push** encoded; **no AI attribution** anywhere.
- [ ] The PR checklist lists the tickets, confirms `git ls-files docs` is empty, and confirms the
      PR base (per `AGENTS.md`'s branching rule once it exists; `develop` until then).
- [ ] The plan is **not executed on creation**.
