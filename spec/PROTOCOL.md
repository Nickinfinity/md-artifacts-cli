# `mda serve` wire protocol — authoritative spec

The **wire contract** between the `mda` engine and every client that speaks JSON-RPC to it (the VS Code
extension today; MCP later reuses the framing). `mda serve` implements it.

> **Authority.** This file defines the protocol; `crates/mda/src/serve/` and the op registry implement it.
> If they disagree, that is a bug to reconcile, never a judgement call. The method, error-code and
> warning-code tables below are **guarded** by `crates/mda/tests/protocol_doc.rs`: they must stay
> set-equal to `system.ops`, `ALL_CODES` and the render warning codes, or the gate fails.

**Protocol version:** `2.1`

**Max line:** `16777216` bytes

Frozen at engine-port W7 (2026-10-17) as `1.0`; `1.1` (W7c) and `1.2` (W7d) were additive minors; `2.0` (W7e, 2026-10-20, §7.2) is the first major; `2.1` (ER-3, 2026-10-29) an additive minor. Within a major every change is additive (§7).

## 1. Transport

- **stdio, UTF-8, newline-delimited JSON.** One JSON object per line (`\n`); a trailing `\r` is stripped
  on input. Every reply is one line ending `\n`, flushed per message. **stdout carries protocol lines
  only**; diagnostics go to stderr.
- Blank lines are ignored. An unterminated last line is still processed. A line longer than the max
  line is refused (§4) and the session continues.
- Requests are answered **one at a time, in arrival order**. No batch, no cancellation, no server
  notifications (§8).
- **Shutdown is EOF only** (there is no `shutdown`/`exit` method): the engine exits `0`, or `3` on a
  stdin/stdout I/O error.
- **Envelope:** `jsonrpc` is ignored on input and always `"2.0"` on output; unknown envelope keys are
  ignored. `params` absent or `null` means `{}`; otherwise **`params` must be an object** — an array
  (by-position params) or scalar is refused with −32000 `op.bad_request`. That check runs inside the
  method, **after** the init gate; `initialize` with non-object params is `op.bad_request` without
  `reason`.
- **Ids.** A request id is a **JSON string**, or a **JSON number written as an optional `-` and digits
  only** (no fraction, no exponent, not `-0`) in **[−2^63, 2^64−1]**. A valid id is echoed unchanged.
  Any other id (`null`, `1.5`, `1e3`, `-0`, `2^64`, an object, an array, a boolean) → −32600
  `protocol.invalid_request` with **`id: null`**; the invalid id is never echoed.
- **Messages without an `id` key.** With a string `method` it is a client notification: v1 defines
  none, so it is **ignored** (no reply, nothing runs — not even `initialize`). Anything else without an
  id (`{}`, `{"method":5}`) → −32600 `protocol.invalid_request`, `id: null`.
- **Check order:** parse (−32700) → is an object (−32600) → id present and valid (−32600) → `method`
  is a string (−32600) → `initialize` / init gate (−32600 / −32002) → method lookup (−32601) → params
  object (−32000 `op.bad_request`) → the op.
- Every error reply's `message` is its `data.code` (§4).

## 2. Handshake

The first request must be `initialize`:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocol":"2.0"}}
```

- `params.protocol` is `"MAJOR.MINOR"`; other keys (e.g. `clientInfo`) are tolerated and ignored.
- Result: `{"protocol":"2.1","engine":"<semver>","types":[…]}`. `types` is the artifact-type registry:
  per type `type, dir, contexts, defaultEnabled, writesFile, outputNameKey?, multiBlock, language?`.
- A missing or non-string `protocol` → −32000 `op.bad_request` (no `reason`).
- The major is the text before the first `.`; a different major → −32000 `protocol.version_mismatch`
  `{client, server}`. The session stays uninitialized and the client may retry. A minor difference is
  accepted: a client that needs a later-minor feature checks `result.protocol` itself.
- A second `initialize` after a successful one → −32600 `protocol.already_initialized`.
- Any other method before a successful `initialize` → −32002 `protocol.not_initialized`.

## 3. Methods

Every method is an op of the engine's registry, named verbatim (dotted, snake_case); `system.ops`
returns the same list with an informational English `summary`. `initialize` is not an op. Each row links
the op's cases, which double as examples (§9).

| Method | Examples |
|---|---|
| `artifact.create` | [`crates/mda/tests/op_cases/artifact.create/`](../crates/mda/tests/op_cases/artifact.create/) |
| `artifact.delete` | [`crates/mda/tests/op_cases/artifact.delete/`](../crates/mda/tests/op_cases/artifact.delete/) |
| `artifact.patch` | [`crates/mda/tests/op_cases/artifact.patch/`](../crates/mda/tests/op_cases/artifact.patch/) |
| `artifact.prefill` | [`crates/mda/tests/op_cases/artifact.prefill/`](../crates/mda/tests/op_cases/artifact.prefill/) |
| `artifact.read` | [`crates/mda/tests/op_cases/artifact.read/`](../crates/mda/tests/op_cases/artifact.read/) |
| `artifact.render` | [`crates/mda/tests/op_cases/artifact.render/`](../crates/mda/tests/op_cases/artifact.render/) |
| `artifact.tree` | [`crates/mda/tests/op_cases/artifact.tree/`](../crates/mda/tests/op_cases/artifact.tree/) |
| `artifact.update` | [`crates/mda/tests/op_cases/artifact.update/`](../crates/mda/tests/op_cases/artifact.update/) |
| `artifact.write_file` | [`crates/mda/tests/op_cases/artifact.write_file/`](../crates/mda/tests/op_cases/artifact.write_file/) |
| `config.get` | [`crates/mda/tests/op_cases/config.get/`](../crates/mda/tests/op_cases/config.get/) |
| `index.create` | [`crates/mda/tests/op_cases/index.create/`](../crates/mda/tests/op_cases/index.create/) |
| `index.plan` | [`crates/mda/tests/op_cases/index.plan/`](../crates/mda/tests/op_cases/index.plan/) |
| `index.run` | [`crates/mda/tests/op_cases/index.run/`](../crates/mda/tests/op_cases/index.run/) |
| `migrate.frontmatter.apply` | [`crates/mda/tests/op_cases/migrate.frontmatter.apply/`](../crates/mda/tests/op_cases/migrate.frontmatter.apply/) |
| `migrate.frontmatter.plan` | [`crates/mda/tests/op_cases/migrate.frontmatter.plan/`](../crates/mda/tests/op_cases/migrate.frontmatter.plan/) |
| `render.text` | [`crates/mda/tests/op_cases/render.text/`](../crates/mda/tests/op_cases/render.text/) |
| `system.ops` | [`crates/mda/tests/op_cases/system.ops/`](../crates/mda/tests/op_cases/system.ops/) |
| `system.version` | [`crates/mda/tests/op_cases/system.version/`](../crates/mda/tests/op_cases/system.version/) |
| `types.list` | [`crates/mda/tests/op_cases/types.list/`](../crates/mda/tests/op_cases/types.list/) |
| `types.set_enabled` | [`crates/mda/tests/op_cases/types.set_enabled/`](../crates/mda/tests/op_cases/types.set_enabled/) |
| `vars.file.new` | [`crates/mda/tests/op_cases/vars.file.new/`](../crates/mda/tests/op_cases/vars.file.new/) |
| `vars.list` | [`crates/mda/tests/op_cases/vars.list/`](../crates/mda/tests/op_cases/vars.list/) |
| `vars.meta.tags` | [`crates/mda/tests/op_cases/vars.meta.tags/`](../crates/mda/tests/op_cases/vars.meta.tags/) |
| `vars.search` | [`crates/mda/tests/op_cases/vars.search/`](../crates/mda/tests/op_cases/vars.search/) |
| `vars.subset.add` | [`crates/mda/tests/op_cases/vars.subset.add/`](../crates/mda/tests/op_cases/vars.subset.add/) |
| `vars.subset.delete` | [`crates/mda/tests/op_cases/vars.subset.delete/`](../crates/mda/tests/op_cases/vars.subset.delete/) |
| `vars.subset.describe` | [`crates/mda/tests/op_cases/vars.subset.describe/`](../crates/mda/tests/op_cases/vars.subset.describe/) |
| `vars.subset.rename` | [`crates/mda/tests/op_cases/vars.subset.rename/`](../crates/mda/tests/op_cases/vars.subset.rename/) |
| `vars.var.add` | [`crates/mda/tests/op_cases/vars.var.add/`](../crates/mda/tests/op_cases/vars.var.add/) |
| `vars.var.delete` | [`crates/mda/tests/op_cases/vars.var.delete/`](../crates/mda/tests/op_cases/vars.var.delete/) |
| `vars.var.rename` | [`crates/mda/tests/op_cases/vars.var.rename/`](../crates/mda/tests/op_cases/vars.var.rename/) |
| `vars.var.set` | [`crates/mda/tests/op_cases/vars.var.set/`](../crates/mda/tests/op_cases/vars.var.set/) |
| `varsets.apply` | [`crates/mda/tests/op_cases/varsets.apply/`](../crates/mda/tests/op_cases/varsets.apply/) |
| `varsets.match` | [`crates/mda/tests/op_cases/varsets.match/`](../crates/mda/tests/op_cases/varsets.match/) |
| `varsets.save` | [`crates/mda/tests/op_cases/varsets.save/`](../crates/mda/tests/op_cases/varsets.save/) |
| `vault.inspect` | [`crates/mda/tests/op_cases/vault.inspect/`](../crates/mda/tests/op_cases/vault.inspect/) |
| `vault.list` | [`crates/mda/tests/op_cases/vault.list/`](../crates/mda/tests/op_cases/vault.list/) |
| `vault.select` | [`crates/mda/tests/op_cases/vault.select/`](../crates/mda/tests/op_cases/vault.select/) |
| `vault.use` | [`crates/mda/tests/op_cases/vault.use/`](../crates/mda/tests/op_cases/vault.use/) |
| `vault.validate` | [`crates/mda/tests/op_cases/vault.validate/`](../crates/mda/tests/op_cases/vault.validate/) |

## 4. Errors

| JSON-RPC code | When | `id` | `data.code` |
|---|---|---|---|
| −32700 | the line is not JSON | `null` | `protocol.parse_error` |
| −32600 | over-long line; not an object (batch, scalar); invalid id; `method` missing or not a string; malformed id-less message; a second `initialize` | `null` when the id is missing or invalid, else the id | `protocol.line_too_long`, `protocol.invalid_request`, `protocol.already_initialized` |
| −32002 | a method before `initialize` | the id | `protocol.not_initialized` |
| −32601 | unknown method | the id | `op.unknown` `{name}` |
| −32000 | the op failed | the id | the op's code (§5) |

- `error.message` **is** `data.code`. `error.data` is `{"code": "<code>", "params": {"<key>": "<string>"}}`:
  every param value is a string.
- Clients key on `data.code` and localise; the engine sends no prose. The one exception is
  `op.bad_request`'s `reason`: informational, unstable across engine versions, never localised.

## 5. Error codes

Params: backticked keys; `` `key?` `` = optional; `—` = none. Paths in params are vault-relative POSIX
when the op holds a vault, else as the client sent them. A `vks.*` `line` is the 1-based line in the `vks` body;
`"0"` means the value did not come from a body line (a client-sent model or render value).

| Code | Params |
|---|---|
| `op.unknown` | `name` |
| `op.bad_request` | `reason?` |
| `op.internal` | — |
| `protocol.version_mismatch` | `client`, `server` |
| `protocol.parse_error` | — |
| `protocol.line_too_long` | `max` |
| `protocol.already_initialized` | — |
| `protocol.not_initialized` | — |
| `protocol.invalid_request` | — |
| `path.outside_root` | `path` |
| `path.not_found` | `path` |
| `path.not_dir` | `path` |
| `file.too_large` | `path`, `size`, `max` |
| `file.not_regular` | `path` |
| `file.conflict` | `path`, `expected`, `actual` |
| `file.exists` | `path`, `actual?` |
| `io.failed` | `path`, `kind` |
| `path.empty` | `path` |
| `path.control_char` | `path` |
| `path.drive_or_scheme` | `path` |
| `path.backslash` | `path` |
| `path.absolute` | `path` |
| `path.parent_segment` | `path` |
| `vault.not_selected` | — |
| `vault.not_obsidian` | `path` |
| `vault.unknown` | `path` |
| `vault.unavailable` | `path` |
| `config.invalid` | `path` |
| `artifact.bad_path` | `path` |
| `artifact.vars_invalid` | `path` |
| `artifact.block_not_found` | `block` |
| `artifact.not_serializable` | `reason` |
| `artifact.unrepresentable` | `field`, `reason` |
| `artifact.not_whole_file` | `type` |
| `artifact.multi_block` | `count` |
| `artifact.is_index` | — |
| `render.limit` | `limit`, `max` |
| `render.contains_escape` | — |
| `index.not_index` | `path` |
| `index.unsluggable` | `segment` |
| `index.limit` | `limit`, `max` |
| `walk.limit` | `limit`, `max` |
| `naming.path_injection` | `field` |
| `naming.empty` | — |
| `naming.edge_space` | — |
| `naming.edge_dot` | — |
| `naming.illegal_char` | — |
| `naming.control_char` | — |
| `naming.reserved` | — |
| `naming.too_long` | `max` |
| `vks.syntax` | `line`, `rule`, `var?` |
| `vks.indent` | `line`, `var?` |
| `vks.bad_key` | `line`, `key`, `var?` |
| `vks.duplicate_key` | `line`, `key`, `var?` |
| `vks.unsupported` | `line`, `construct`, `var?` |
| `vks.mixed_list` | `line`, `var?` |
| `vks.inline_comment` | `line`, `var?` |
| `vks.control_char` | `line`, `var?` |
| `vks.limit` | `line`, `limit`, `max`, `var?` |
| `vks.mixed_dialect` | `line`, `var?` |
| `vks.unrepresentable` | `var`, `reason` |
| `vars.bad_name` | `name` |
| `vars.var_exists` | `name` |
| `vars.var_not_found` | `name` |
| `vars.subset_exists` | `heading` |
| `vars.bad_heading` | `reason` |
| `vars.untitled_description` | — |
| `vars.bad_title` | — |
| `vars.fenceless_section` | `heading` |
| `vars.mixed_file` | — |
| `vars.untagged_defaults` | `info` |
| `migrate.up_to_date` | — |
| `migrate.leading_blank` | — |
| `migrate.quoted_type` | `value` |
| `migrate.pending` | `kind` |

## 6. Warning codes

Render warnings arrive in `warnings: [{code, params}]` of `artifact.render`, `artifact.write_file`,
`render.text` and `index.run`, never as errors. `line` is 1-based; one of `name` (`VK-x`) or `path`
(`VK-a.b`) names the token.

| Code | Params |
|---|---|
| `render.unknown_var` | `line`, `name?`, `path?` |
| `render.each_not_list` | `line`, `name?`, `path?` |
| `render.unterminated` | `line`, `name?`, `path?` |
| `render.unmatched_end` | `line`, `name?`, `path?` |
| `render.self_nested` | `line`, `name?`, `path?` |
| `render.not_scalar` | `line`, `name?`, `path?` |
| `render.join_record` | `line`, `name?`, `path?` |
| `render.join_empty` | `line`, `name?`, `path?` |

## 7. Compatibility

- Within a major, method names, error and warning codes, param keys, response fields and the
  `initialize.types` fields are **additive only**: nothing is renamed, removed or retyped.
- Requests are strict (`deny_unknown_fields`): a request field added in a later minor is refused by an
  older engine with `op.bad_request`. A client sends a new field only after seeing a `protocol` minor
  that has it.
- Responses may gain fields; clients ignore fields they do not know.
- A breaking change bumps the major; the engine refuses a different major at `initialize`.

### 7.1 Versions

| Version | Engine | Change |
|---|---|---|
| `1.0` | 0.7.0 | the frozen surface |
| `1.1` | 0.8.0 | Lossless rewrite (format spec §7.6). **Response:** `artifact.read` gains `layout` (absent = fenced). **Request:** the `artifact.create` / `artifact.update` model gains `layout` and `paths`. **Behaviour:** `artifact.update` rewrites a flagged file when the model's `layout.payload` is `flags`, and an index file when the model carries its `paths`; otherwise the 1.0 refusal `artifact.not_serializable{reason}` stands, so a 1.0 client can never erase notes or `paths`. A frontmatter `artifact.patch` on a flagged file now succeeds (byte-surgical). No method, code or param key was added or removed. Emitted bytes changed (tags and `paths` as block lists, `tags` last); the file format is versioned by the format spec, not here |
| `1.2` | 0.9.0 | Defaults fence ```` ```yaml VKs ```` (format spec §9; ```` ```vks ```` no longer read). **Response:** a defaults fence's `fenceLang`, and a Variables file's hoisted `frontmatter.language`, are `"yaml VKs"` (were `"vks"`). **Code:** `vars.untagged_defaults{info}` (a `vault.validate` warning). Emitted bytes: every defaults fence opens ```` ```yaml VKs ```` |
| `2.0` | 0.10.0 | **Breaking** (§7.2): the legacy `KEY=value` variables dialect is gone, with everything that existed only for it |
| `2.1` | 0.11.0 | `containsEscape` widened (§11): any terminal control — C0 except TAB and LF (a CR directly before LF is exempt), DEL, C1 U+0080–U+009F — not only ESC. No method, field or code added. The CRLF exemption is looser than a client's "C0 except TAB" predicate |

### 7.2 What 2.0 removed

- **Methods:** `migrate.vks.plan`, `migrate.vks.apply` (and the CLI `mda migrate vks …`).
- **Codes:** `migrate.unfenced_vars`. `migrate.pending{kind}` keeps `frontmatter` only (`vks` is no longer
  sent).
- **Format** (spec §9.6): a defaults fence body is YAML only; a `KEY=value` line is `vks.mixed_dialect`, and
  an unfenced `vars:` section is no longer read (its lines are prose).
- A 1.x client is refused at `initialize` (`protocol.version_mismatch`); nothing else in the surface moved.

## 8. Notifications

v1 sends **no** server notifications. Clients **MUST ignore** any server notification (a message with a
`method` and no `id`) they do not know. `vault.changed` and `config.changed` are **reserved** for a later
1.x minor; until then a client that needs change events watches the vault itself.

## 9. Examples

The op cases under `crates/mda/tests/op_cases/<op>/<case>.toml` are run through dispatch, `mda call
--json` and a `serve` session on every gate; they are **examples, not part of the contract**. Grammar:
`vault = "<fixture>"` (a copy of `crates/mda/tests/fixtures/<fixture>/`), `params` (the request params),
`expect` (the full result body) or `expect_error` (`data.code`) with optional `expect_error_params`
(`data.params`), `expect_files` (vault-relative path → bytes, or `false` = absent), `machine_config`,
`vault_flag`, `workspace`. `$VAULT`, `$CONFIG` and `$WORKSPACE` stand for the per-run vault copy, machine
config dir and workspace.

## 10. CLI --json

`mda <command> --json` and `mda call <op> [<json>] --json` print the same result body `serve` returns
(an error as the `{code, params}` object). That output, and the CLI exit codes (`0` ok, `1` op error,
`2` usage, `3` engine fault), are **not part of this protocol**: they are stable but versioned with the
engine's semver.

## 11. Client notes

**`containsEscape`** (`artifact.render`, `render.text`, `artifact.write_file`, `index.run`) is `true` when the output holds a
terminal control: any Unicode `Cc` character (C0 U+0000–U+001F, DEL U+007F, C1 U+0080–U+009F) except TAB, LF, and a CR
**directly followed by LF** (CRLF vault files). A bare CR (`a\rb`, a final `a\r`) counts; so does the first CR of `\r\r\n`.
Since `2.1`; in `2.0` it meant ESC only. Clients that send rendered text to a terminal refuse it; the human CLI `render` does
(`render.contains_escape`).

Names used by the extension's cutover plan that are not methods, mapped to the real op:

| Plan 2 name | Method |
|---|---|
| `artifacts.tree` | `artifact.tree` |
| `types.setEnabled` | `types.set_enabled` |
| `varsets.list` | `vars.list` |
| `varsets.search` | `vars.search` |
| `subset.add` | `vars.subset.add` |
| `subset.delete` | `vars.subset.delete` |
| `subset.describe` | `vars.subset.describe` |
| `subset.rename` | `vars.subset.rename` |
| `push` | none |
