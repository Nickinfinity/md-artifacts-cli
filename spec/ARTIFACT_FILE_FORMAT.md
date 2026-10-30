# Artifact `.md` file format — authoritative spec

The **on-disk contract** for every MD Artifacts vault file. The `mda` engine implements it; every
client (CLI, VS Code extension, TUI, MCP) sees vault content only through the engine.

> **Authority.** This file defines the format; the code implements it. If `mda-core` and this spec
> disagree, that is a bug to reconcile, never a judgement call, and the reconciliation is recorded
> (a fix, or a spec change in the same commit). `parse(serialize(x)) ≅ x` must hold within the
> ceilings stated here.

> **Provenance.** Ported 2026-10-09 from the VS Code extension's `ARTIFACT_FILE_FORMAT.md`
> (`md-artifacts-snippets_and_tools-vscode` @ `b368c20`). §1–§8 describe the same format the extension
> reads and writes today; only the implementation references changed. **§9 and §10 are new** (YAML
> variable bodies, structured values, directives); the decisions they cite (D-2…D-11) were **ruled at
> the engine-port plan's W1 Role A pass 1 (2026-10-09)**. §11.2 stays proposed until W6. Until the extension cuts over to the
> engine, parity with the extension's behaviour is measured by `conformance/`, not by reading code.
> **§7.6 (lossless layout) and the Obsidian-style frontmatter (§1.1, §1.4, §8.7) were ruled at engine
> W7c (2026-10-19)**: `create(read(f))` loses nothing the format defines, and a file in canonical layout
> is written back byte for byte. Recorded deviations from the extension: `deviates_from_ts` in
> `conformance/manifest.toml`.
> **The defaults fence is ```` ```yaml VKs ```` since engine W7d (2026-10-20, §9)**; ```` ```vks ```` is no
> longer read. **The `KEY=value` dialect, the unfenced `vars:` section and the vks migration are gone since
> W7e** (2026-10-20, §9.6, §11.2).

> Examples use `~~~` as the outer fence so inner ```` ``` ```` fences render literally. In a real file
> every fence is a standard triple backtick.

### Owners in the engine

| Concern | Module |
|---|---|
| Frontmatter, code fence, `##` blocks, flags, tokens | `mda-core/src/parse/` |
| The `vks` body (both dialects) | `mda-core/src/vks/` (codec) |
| Token substitution and directive resolution (§4.1, §10) | `mda-core/src/render/` |
| Emission | `mda-core/src/serialize/` (`mod.rs`; flags and bare bodies `payload.rs`; the round-trip guard `guard.rs`) |
| Artifact types, directories, per-type properties | `mda-core/src/registry.rs` |
| Language names, fence aliases, file extensions | `mda-core/src/language.rs` |
| Output file names (whole-file types), vault file-name stems, the default slug (§14) | `mda-core/src/naming.rs` |
| Index links and vault-authored relative paths | `mda-core/src/multi_index.rs` (`safe_rel_path`) |

Parse and validation failures are reported as **error codes with parameters** (e.g.
`vks.duplicate_key { key, line }`), never as prose. Every code is listed in `mda-ops/src/error.rs` (`ALL_CODES`); the `vks.*` constants
are defined in `mda-core/src/error.rs`, which cannot depend on the ops crate.

---

## 1. Canonical single-block structure

~~~md
---
artifactType: Snippet
title: Human-readable title
description: Short explanation
language: javascript
tags: [tag1, tag2]
---

```javascript
const x = <VK-variableName>;
```

vars:
```yaml VKs
VK-variableName: defaultValue
```
~~~

### 1.1 Frontmatter

YAML-*looking* lines between `---` fences at the very start of the file. **It is not parsed as YAML:**
each line is split on its **first** `:`; the key and value are trimmed. Unknown keys are ignored.

| Key | Kind | Notes |
|---|---|---|
| `artifactType` | string | **Required in spirit.** Exact, case-sensitive PascalCase match against the registry: `Snippet`, `AIAgentsConfig`, `Command`, `Template`, `Variables`, `AIPrompt`. Missing or unrecognised → the type derived from the artifact's **directory** (§5). Never a hardcoded default, never a case-insensitive match |
| `title`, `description`, `language`, `env`, `target` | string | single line |
| `extension` | string | `Template` only (§5.1) |
| `provider`, `model`, `version` | string | `AIAgentsConfig` only (§5.2) |
| `tags`, `paths` | inline array or block list | `[a, b]`; brackets optional; split on `,`, trimmed, empties dropped. Or, with an empty value, the following `- item` lines (any indent; one pair of matching quotes stripped; empties dropped) — the form Obsidian's property editor writes (engine only; the extension reads inline arrays). **Emitted as a block list** (W7c). `paths` is index-only (§8) |
| `index` | boolean | **only the exact value `true`** is true (§8) |

- **`artifactType` is the only type key.** A legacy `type: <lowercase>` line is **not read** — not as
  a fallback, not as a second key. It is an unrecognised line; the other keys still parse, and the type
  falls back to the directory default. The way forward for such files is the frontmatter migration
  (`mda migrate frontmatter`, §11.1).
- **`index` and `paths` are emitted** when the model sets them (§8.7; `paths` since W7c).
- A blank line after the closing `---` is optional.

### 1.2 Code block

The **first fenced block** after the frontmatter that is not a ```` ```yaml VKs ```` fence. Its info string
may be a language (`javascript`), `code`, or **empty** (plain text). Trailing whitespace is trimmed on
parse; the emitter trims too. A file may instead delimit its payload with **flags** (§7).

### 1.3 Variable defaults

Optional. One form:

A ```` ```yaml VKs ```` fence. It may be preceded by a decorative label line (`vars:`, `vars`,
`### VKs:`, prose); the label is ignored and the fence binds. The fence body is **YAML** (§9). Nothing
else supplies defaults: the pre-W7e `KEY=value` dialect and the unfenced `vars:` section are gone (§9.6).

Keys use the full `VK-` prefix (`VK-host`). Variables are also **auto-detected** from the tokens in the
code (§4), so a defaults section is only needed to supply values. A var that appears only in the
defaults section is kept, after the detected ones.

### 1.4 Emission rules (single block)

- **Frontmatter key order** (W7c: `tags` last, where Obsidian's property editor keeps it):
  `artifactType`, `title`, `description`, `language`, `extension`, `provider`, `model`, `version`,
  `env`, `target`, `index`, `paths`, `tags`. Empty values are omitted (`artifactType` is always
  emitted); `index` only as `index: true` (§8.7). `paths` and `tags` are YAML block lists
  (`tags:` then one `  - item` line each).
- **Single-line values.** `title`, `description`, `env`, `target`, `extension`, `provider`, `model`,
  `version`: every `\r\n` / `\r` / `\n` becomes one space, runs of spaces collapse. A newline would
  otherwise inject a sibling key on re-parse. A `tags` / `paths` item must not contain `\n` or `\r`
  and must not be wrapped in one pair of matching quotes (the reader strips them); the round-trip guard
  refuses such an item (`artifact.unrepresentable`). A `,` is fine in a block list (W7c).
- **Language in both places:** a single-block file emits `language:` in the frontmatter **and** on the
  fence; the parser hoists the fence language into the model when frontmatter lacks it.
- **Plain text** (`language` empty): a bare ```` ``` ```` fence and **no** `language` key at all.
- **Defaults:** a `vars:` label then a ```` ```yaml VKs ```` fence, emitted **only if** at least one var has
  a non-empty value or a structured value (§9.3). Body per §9.5.
- **Empty whole-file body:** a `Template` / `AIAgentsConfig` model with empty code and no defaults to
  emit is written as **frontmatter only** (no fence), which reads back with empty code and **no** language
  (§7.5: an empty bare body falls through to the fenced reading).
  *(W3b R-28(1); the extension writes an empty fence, which its own parser then reads back as the fence
  text — recorded deviation.)*

---

## 2. Multi-block structure

A file with one or more `## ` (h2) headings each followed by a fenced code block is a **multi-block
file**. Each section is a block.

- Sections split on `## ` **only**; `###`+ stays inside a block. An emitter never writes `## ` inside code.
- The text between a `## Heading` line and the section's code fence is that block's **description**.
  The frontmatter `description` is the file-level description; both may coexist.
- A section may carry its own defaults: a ```` ```yaml VKs ```` fence **anywhere after** its code fence
  (blank lines, a `### VKs:` marker, prose in between are ignored). Per-block vars = tokens detected in
  that block's code, overlaid with that fence's values (code order kept; vks-only keys appended).
- A section whose **first** fence is ```` ```yaml VKs ```` is a pure **sub-set** (Variables files, §3).
- **The same token in two blocks is two separate vars** — block-scoped.

### 2.1 Emission rules (multi-block)

- **Nothing before the first `##`**: no top-level code fence and no top-level vks fence. The parser
  reads the first fence and first vks fence of the file unconditionally, so emitting one would hoist a
  block's content to file level on re-parse.
- **No `language:` in frontmatter**; each block's fence carries its own.
- **Per block:** `## <heading>`, optional one-line description, blank line, the code fence, then the
  optional defaults fence (same emit-when rule as §1.4).

~~~md
---
artifactType: Snippet
title: API URLs
---

## Development
Local dev server.

```bash
http://localhost:<VK-port>
```

### VKs:

```yaml VKs
VK-port: "3000"
```

## Production

```bash
https://api.example.com
```
~~~

---

## 3. `artifactType: Variables` files

The content **is** the ```` ```yaml VKs ```` fence; there is no code fence. Used for environment variables
and for **variable sets** (§6).

~~~md
---
artifactType: Variables
title: Local services
env: dev
---

```yaml VKs
VK-api_url: http://localhost:3000
VK-db_url: mongodb://localhost:27017
```
~~~

- **Shapes**, classified from structure (never from a count):

| Shape | On disk |
|---|---|
| `blank` | frontmatter only |
| `flat` | one **untitled** ```` ```yaml VKs ```` fence |
| `sets` | one or more `## Title` + fence — **even one** titled section is `sets` |

- In a Variables file, vars are emitted **unfiltered**: an empty value is a key the user asked to
  record. A `## ` sub-set with no vars still gets an **empty** fence (```` ```yaml VKs ```` + ```` ``` ````),
  or the heading would be dropped on re-parse.
- Top-level keys in a Variables file **may** omit the `VK-` prefix (`API_URL: …`); such a key cannot
  be referenced by a token and is kept as written.

---

## 4. Variable tokens — `<VK-…>`

### 4.1 Plain tokens

- **Grammar:** `<` `/`? `VK-` *hint* `>`, where *hint* is `[A-Za-z][A-Za-z0-9_]*`.
- **The token is the variable name:** `<VK-host>` → `VK-host`. Matching, deduplication and var-set
  matching use the full name, case-sensitive.
- **Closing form `</VK-host>` is the same variable.** Both spellings detect, deduplicate and substitute
  identically.
- **Substitution is one left-to-right pass:** an **adjacent** pair `<VK-x></VK-x>` is one unit and
  resolves to the value **once**; every other opening or closing token resolves individually.
  `<VK-a></VK-b>` is two tokens; `<VK-x> text </VK-x>` is two tokens. **Inserted values are never
  re-scanned**: a value containing `<VK-y>` is output as those characters. *(Engine-port W3 R-11; the
  extension's two-pass `resolveVars` re-scans values placed by an adjacent pair — recorded deviation.)*
- **What is a variable when rendering:** every default (§1.3 / §9) **plus every token detected in the
  code being rendered**, in every layout (fenced, flagged, bare body). *(W3 R-25(1); the extension's
  fenced reader knows declared defaults only, so a fenced token without a default could never be
  filled — recorded deviation. `artifact.read` still reports a fenced file's defaults only.)*
- **Unknown variables stay literal**, so partial substitution is safe. Supplied values whose name matches
  no variable are ignored.
- **Value resolution, in order:** the value the client supplies → the default from §1.3 / §9 → otherwise
  the token stays literal. **An empty string counts as not supplied**: an empty client value falls back
  to the default, and an empty default leaves the token literal (W3 R-12, extension parity).
- Collision-free by design: `<VK-` does not occur in JS/TS generics, JSX, HTML, CSS, Vue, Python, shell,
  Jinja or Handlebars syntax. Never use `{{name}}`.

### 4.2 Render-safety in markdown payloads

Inside a fence nothing is parsed as HTML. In a **flagged payload** (§7), the note body *is* markdown,
and `<VK-repo>` is a legal HTML open tag (tag names allow letters, digits and hyphens): Obsidian treats
it as an unclosed element and swallows every block after it.

| Spelling | Obsidian renders | Resolves to |
|---|---|---|
| `<VK-repo>` | ✗ swallows the rest of the note | value |
| `<VK-repo></VK-repo>` | ✓ empty element | value **once** |
| `</VK-repo>` | ✓ stray end tag | value |
| `<VK-repo_name>` | ✓ `_` is illegal in a tag name → text | value |
| `<VK-users.name>`, `<VK-each:users>`, `<VK-join:tags>` (§10) | ✓ `.` / `:` directly after the name → not a tag → text | per §10 |

This is a **rendering** concern only: the engine resolves every spelling identically.

---

## 5. Artifact types

Each type lives in a fixed vault directory (directory names are not configurable, decision E-10). The
registry (`registry.rs`) is the single source for these properties.

| `artifactType` | Directory | `language` | Payload | Multi-block | Whole-file | Notes |
|---|---|---|---|---|---|---|
| `Snippet` | `Snippets/` | free (or plain text) | code fence | yes | no | |
| `Command` | `Commands/` | **locked to `bash`** | code fence | yes | no | The emitter writes `bash` in frontmatter (single-block) and on every fence; `sh` or an empty fence still parses and is normalised on write |
| `Template` | `Templates/` | free | code fence, flags, or bare body | **no** | **yes** | §5.1. May be an index (§8) |
| `AIAgentsConfig` | `AIAgentsConf/` | optional | flags, code fence, or bare body | yes (authoring) / one region to write | **yes** | §5.2. May be an index (§8) |
| `Variables` | `Variables/` | n/a | the vks fence | yes (sub-sets) | no | §3, §6 |
| `AIPrompt` | `AIPrompts/` | always `markdown` | **flags** | yes (named regions) | no | §5.3 |

**Whole-file types** produce a file in a workspace folder instead of text for insertion. Only these
two types may be indexes. The engine performs that write (decision E-1), contained to a root the client
supplies.

### 5.1 `Template`

- **Single block only.** A 2+ block template is a validation error at write time; nothing is written.
- **Output file name — precedence:** the name the user typed (if it carries an extension it wins whole)
  → frontmatter `extension:` (leading dot optional) → the extension of **frontmatter `language`**
  (`language.rs`). A typed name without an extension gets the extension appended. The stem, when not
  typed, is the **raw title**, else the vault file's name (without `.md`), else `template` — trailing
  dots stripped, never slugified (W3 R-25(2), extension parity).
- **Every input is a path-injection surface** — the typed name, `extension:` and the title fallback: a
  value containing `/`, `\`, `..` or NUL is **rejected, never sanitised** (`naming.path_injection`).
  The final name is also validated, first failure wins: more than 255 bytes; empty; a leading or trailing
  space; a **trailing** dot (or all dots — a leading dot is allowed: `.env`, `.cursorrules`); any of
  `\ / : * ? " < > |`; a control character (C0, DEL or C1 — C1 since engine 0.12.0); a Windows reserved name, also as the stem before the first
  dot (`CON.txt`), case-insensitive (W3 R-6, R-7, R-16, R-25(3, 21); the reserved-stem and leading-dot
  rules differ from the extension's input box).

### 5.2 `AIAgentsConfig`

~~~md
---
artifactType: AIAgentsConfig
title: Code reviewer
provider: Claude
model: Opus
version: "4.8"
target: CLAUDE.md
---
~~~

- `provider` / `model` / `version`: optional, free text, single line, metadata only (they change no
  behaviour). Empty values are omitted. They round-trip.
- **Output file name:** `target:` **verbatim** (e.g. `CLAUDE.md`, `.cursorrules`), never
  extension-appended; absent → the **raw title**, else the vault file's name (trailing dots stripped,
  not slugified) + `.md` unless it already carries an extension, else `agent.md`. A typed name is used
  verbatim. The typed name, `target:` and the fallback are path-injection surfaces (same rejection and
  validation rules as §5.1).
- Writing requires a single block/region, even though authoring may use several.
- Usually authored with flags (§7); a fence or a bare body also works (§7.5).

### 5.3 `AIPrompt`

Authored with flags exactly like an agent config's markdown payload; language is always `markdown`.
It is **not** a whole-file type, so the bare-body fallback (§7.5) never applies: a prompt with **no
flags and no fence parses to an empty payload**. That is accepted behaviour, not a bug. It is the only
type whose rendered output may be delivered to either an editor or a terminal (clients decide which;
the engine only renders).

---

## 6. Variable sets — storage shape

Variable sets are `artifactType: Variables` files in `Variables/` (any depth). A flat file is one set;
a `sets` file holds one independent sub-set per `## Heading`. Matching against an artifact uses the
full `VK-…` name, exact and case-sensitive. Scoring, apply, save and the shape transitions are engine
behaviour, specified in §13.

---

## 7. Flags — plain-markdown payloads

When the payload is itself markdown (agent configs, prompts), a fence is wrong (the payload may contain
fences) and a `##` heading would swallow the author's notes. **Flags** delimit it, using Obsidian's
comment syntax so they are invisible in reading view:

~~~md
---
artifactType: AIAgentsConfig
title: Code reviewer
target: CLAUDE.md
---

Scratch notes — not part of the artifact.

%%oa:start%%
# Reviewer

Review <VK-repo_name> and report findings.

```bash
npm test
```
%%oa:end%%
~~~

### 7.1 Syntax

| Flag | Form |
|---|---|
| Start | `%%oa:start%%` or `%%oa:start Some name%%` |
| End | `%%oa:end%%` |
| Visual rule | `***` alone on a line **inside** a region — dropped from the payload |

- A flag is the only thing on its line; surrounding whitespace and spaces inside `%%…%%` are tolerated
  (`%% oa:start Dev %%`). The name is everything after `oa:start`, trimmed. Case-sensitive, lowercase.
- One module spells the marker: `mda-core/src/parse/` (flags).

### 7.2 Parsing rules

- **Flags beat fences.** A file with at least one start flag takes the flagged path; its fences and
  `##` headings are payload. A file without flags parses exactly as §1–§2.
- **Text outside the flags is not payload.** It never reaches render or insert; `artifact.read` reports
  it as **notes** (§7.6) so that writing the file back keeps it.
- **Content is verbatim**, inner fences included; only blank lines at the two ends are trimmed.
- **Fenced regions are skipped while scanning** (CommonMark rules: a fence closes on the same character,
  same length or longer), so a payload documenting the flag syntax inside a sample fence does not end
  itself.
- **One region → single-block file** (its name is decorative). **Two or more → one block per region**,
  the flag name as the block heading.
- `language` defaults to `markdown` (an explicit `language:` wins).
- **Lenient:** an unterminated start runs to end of file; a second start inside an open region is content.
- **`***` inside a region is chrome:** every line consisting only of asterisks (`***`, `*****`, padded)
  between a start and an end is dropped. `**`, `* * *`, `***text***` are content. Outside regions, in
  flag-less files and inside fenced code, `***` is content. `---` is never special.

### 7.3 Variables in flagged payloads

Tokens in the payload are auto-detected. A ```` ```yaml VKs ```` defaults fence is **file-level** and goes
**outside** the flags (anything between the flags is payload). Use a render-safe token spelling (§4.2).

### 7.4 Whole-file types

The region content is the payload, so the single-block rule (§5.1/§5.2) and output naming apply to a
flagged file exactly as to a fenced one. **Flags are written back** (W7c, §7.6): a model whose layout is
`flags` is emitted with its flags, notes and `***` wrapping, so a payload holding fences or `## ` lines
survives a rewrite.

### 7.5 Flags are optional for whole-file types

Payload precedence, strictly ordered:

1. **Flags** — if any region exists, that is the payload.
2. **Code fence** — a ```` ```yaml VKs ```` fence does not count (it is the defaults section).
3. **Bare body** — **only for whole-file types** (`Template`, `AIAgentsConfig`): the whole body minus
   its `vars:` label and ```` ```yaml VKs ```` fence, with `language: markdown`.
   An **empty** bare body (after trimming) does not take this rule: the payload is the empty fenced
   reading — empty code, no language (W3b R-29; §1.4 relies on it).

`Snippet`, `Command`, `Variables` and `AIPrompt` never take rule 3.


### 7.6 Layout — writing a file back losslessly *(ruled — engine W7c, 2026-10-19)*

`artifact.read` reports how the payload sits in the file; the model (`artifact.create|update`) carries
the same object back. Absent (the default) means **fenced** (§1–§2).

```json
"layout": {
  "payload": "flags",
  "regions": [ { "name": "Scope", "notes": "Text before this start flag.", "rule": true } ],
  "notes": "Text after the last region."
}
```

| Field | Meaning |
|---|---|
| `payload` | `fence` (default, omitted) · `bare` (§7.5 rule 3, whole-file types, one block) · `flags` (§7) |
| `regions[i].name` | the flag name; with two or more regions it is block `i`'s heading |
| `regions[i].notes` | the text between the previous region (or the frontmatter) and this start flag; omitted when empty |
| `regions[i].rule` | the region's first and last lines are `***` visual rules (chrome); omitted when false |
| `notes` | the text after the last region, **minus** the defaults section: the first ```` ```yaml VKs ```` fence outside the regions and the `vars:` label just above it |

- **Notes** are verbatim text with whole blank lines trimmed at both ends; they are never payload.
- **Canonical flagged layout**, written by the emitter:

~~~md
---
<frontmatter; no `language:` when it is the default `markdown`>
---

<regions[0].notes>

%%oa:start <name>%%
***
<payload>
***
%%oa:end%%

<regions[1].notes, then region 1 …>

<notes>

vars:
```yaml VKs
<every valued default>
```
~~~

  Blank lines separate the parts; an empty part is left out with its blank line; `***` lines only
  when `rule` is true. The defaults fence is file-level (§7.3) and holds **every valued var of every
  block** (the reader gives every region every default, so blocks that disagree on a value are refused).
- **Canonical bare layout:** frontmatter, a blank line, the trimmed body, then the defaults section.
  A bare model with an empty body is the empty fenced shape (§1.4).
- **Normalisations** (the round-trip guard applies them to both sides, nothing else): notes and
  payload trimmed as the reader trims them; missing `regions` entries are default; with several
  regions the name is the heading; the payload language defaults to `markdown`.
- **Refused, never written lossy** (`artifact.unrepresentable{field, reason: round_trip}`): notes holding
  a start flag, a defaults fence or an unclosed fence; a payload holding an end flag, an all-asterisk
  line or an unclosed fence; `bare` on a non-whole-file type or with a fence in the body; `flags` on a
  Variables file; regions or notes on a fenced model.
- **Byte for byte:** for a file already in canonical layout, `serialize(from_parsed(parse(f))) == f`.
  A hand-written file converges in one rewrite; what changes is spacing, key order, list style, the
  defaults fence's position and key order, and a missing end flag — never text the reader saw.
- **Ceilings:** an all-asterisk line in the **middle** of a region is chrome and is dropped (§7.2); only
  the wrapping pair is recorded. A fence-less `## ` section of a multi-block file is not a block (§2) and
  is not carried; neither is prose around the fence of a fenced file (§1.2).
- **Older clients:** a 1.0 client never sends `layout` / `paths`. `artifact.update` (and every `vars.*`
  edit) on a flagged file whose model is not `flags`, or on an index whose `paths` the model drops, stays
  `artifact.not_serializable{reason: flagged | index}` — the pre-W7c answer, so such a client can never
  erase notes or `paths`. A flagged **Variables** file is always refused (`flagged`). A frontmatter
  `artifact.patch` is byte-surgical and allowed on a flagged file; a code patch there stays refused.
---

## 8. Template indexes — `index: true`

A whole-file artifact carrying `index: true` whose body **links** sibling artifacts; running it writes
**every linked file** in one run. No new type, directory or command is involved.

~~~md
---
artifactType: Template
title: React component scaffold
index: true
paths: [src/components, packages/ui/src]
---

1. [[dir_2/subdir1/Button]]
2. [T](dir_2/subdir1/Button.test.md)
3. [[dir_1/barrel]]
~~~

### 8.1 Keys

| Key | Meaning |
|---|---|
| `index` | Only the exact value `true` marks an index. Valid only on whole-file types; elsewhere it is ignored |
| `paths` | Extra destination folders offered for every link: workspace-relative, POSIX, file-level, suggestions only |

An index is **never written verbatim**: running it writes the linked files, not itself.

### 8.2 Links

- Both Obsidian forms, matched in **one pass** over the index's body (the parsed payload) so document
  order is run order: wikilinks `[[target]]` (alias `|text` stripped first, then anchor `#Heading`) and
  markdown links `[text](target)`. Exact grammar (W3b, extension parity): wikilink target
  `[^\[\]\n]+`, link text `[^\[\]\n]*`, markdown target `[^()\n]+`; targets are trimmed.
- **Duplicates are preserved** (a link written twice is two steps).
- **Images and embeds are not links**: `![alt](url)` and `![[Note]]` are ignored (a match preceded by
  `!` is skipped and scanning resumes one character later, so `!![[n]]` and `![![…` floods stay linear).
- **A link is single-line, and its target contains no `[` or `(`** — a hostile body of unclosed
  `[[[[…` must cost linear time. Such a target is unlinkable, never truncated into a different path.
- `.md` is appended when absent (checked case-insensitively: `Note.MD` stays as written). Any other
  text is ignored.
- **Limits:** at most **1 000 links** per index (`index.plan`), **1 000 steps** per `index.run` and
  **1 000 paths** per `index.create`, and at most **64 `paths:` entries** per index (normalised once, before
  planning — candidates are per link, so the response grows with links × entries), checked before any read
  → `index.limit{limit: links|steps|paths, max}` (W3b, #26).

### 8.3 Resolution and the rejection list

A link resolves **relative to the index file's own directory only**; there is no vault-wide search.
Every vault-authored string that becomes a path (link targets and `paths:` entries) goes through
**`safe_rel_path`**, which **rejects and never sanitises**:

Checks run **in this order**; the first that matches decides the code (W3 R-20, extension parity):

| # | Rejected | Error code |
|---|---|---|
| 1 | `''` | `path.empty` |
| 2 | any C0, DEL or C1 control character | `path.control_char` |
| 3 | `C:\tmp`, `file:///etc` (any `:`) | `path.drive_or_scheme` |
| 4 | `dir\sub\file` | `path.backslash` |
| 5 | `/etc/passwd`, `/` | `path.absolute` |
| 6 | `../escape`, `a/../../b` | `path.parent_segment` |
| 7 | `.`, `./`, empty after dropping `.` and empty segments | `path.empty` |

Accepted paths are normalised to POSIX (doubled separators and `.` segments collapse); segment
content is **never decoded**, so `%2e%2e%2f` stays one inert segment. After `safe_rel_path`, the
**resolved** path is still containment-checked (canonicalized, component-wise) immediately before any
read or write.

### 8.4 What a linked file must be

Read and parsed as an ordinary artifact, then checked: a **whole-file type**, **single-block**, and
**readable**. A failure **skips that step** with its reason; the rest of the run continues.

### 8.5 Destinations

Per step, offered in order: (1) the **mirrored folder** (the folder the run started from plus the
link's own directory relative to the index), pre-selected; (2) each accepted `paths:` entry; (3) any
other folder inside the workspace root. Candidates are deduplicated, first wins. Missing folders are
created (intermediate levels included) **after** containment against the workspace root. A skipped
step continues the run; an explicit abort stops it. A run ends with one summary: written / skipped
counts and the reasons.

### 8.6 Variable carry-over

Values supplied for one step pre-fill the **same-named** var (full `VK-…` name, exact) in later steps
of the same run. A carried value replaces that file's default; the user can still change it. Carry-over
is in-memory and per run; nothing is written to the vault.

### 8.7 Emission

`index` is an ordinary frontmatter property of the model (W3b Q4): a model with `index: true` emits
`index: true` (unquoted) and `index: false` is never written. `paths` is a model field too since W7c,
emitted right after `index` as a block list; `tags` follows (§1.4). `index.create` and
`artifact.create` both use them. `artifact.update` rewrites an index file when the model carries its
`paths` (a model that drops them is refused, §7.6).

---

## 9. The `vks` body *(ruled — engine-port W1 Role A pass 1, 2026-10-09)*

The body of a ```` ```yaml VKs ```` fence. Decision references (D-n) point to the design record in the
engine-port plan.

**The info string is exactly `yaml VKs`** *(ruled — engine W7d, 2026-10-20; was `vks`)*. Markdown takes the
first word as the language, so Obsidian and GitHub highlight the body as YAML; the second word `VKs`
keeps a defaults fence apart from a YAML **code** fence (a docker-compose or Kubernetes snippet), which
a bare ```` ```yaml ```` would collide with. It is case-sensitive and the only info string with a space
that opens a fence (§1.2: any other ```` ```lang words ```` opener is not a fence). The pre-W7d
```` ```vks ```` spelling is **no longer read**: such a fence is an ordinary code fence with language
`vks`. `vault.validate` warns `vars.untagged_defaults{info}` on a ```` ```yaml ```` (any info but
`VKs`) or ```` ```vks ```` fence whose first significant line is a `VK-` key. A read reports a
defaults fence's `fenceLang` (and a Variables file's hoisted `language`) as `yaml VKs`.

### 9.1 Grammar — a strict YAML subset (D-1, closed: hand-rolled, no YAML library)

```
body       := map(0)                                   ; the top level is always a map
map(n)     := entry(n)+                                ; every key at exactly indent n
entry(n)   := ind(n) key ':' ( ' '+ scalar
                             | ' []'                    ; empty list
                             | ' |' | ' |-'             ; block scalar, lines at indent n+2
                             | EOL ( map(n+2) | seq(n+2) ) )
seq(n)     := item(n)+
item(n)    := ind(n) '- ' scalar                       ; string item
            | ind(n) '- ' key ':' rest                 ; compact record item; further entries at n+2
key        := top level: [A-Za-z][A-Za-z0-9_-]*
              nested:    [A-Za-z][A-Za-z0-9_]*         ; must be usable as a path segment (§10)
scalar     := plain | single-quoted | double-quoted
plain      := does not start with any of  [ ] { } & * ! | > ' " % @ ` # , ? -␠
              contains no ': ' and no ' #', does not end with ':'; trimmed
single     := ' … '   with '' as the only escape
double     := " … "   with \\ \" \n \t as the only escapes
comment    := a whole line whose first non-space character is '#'
```

Indentation is **exactly two spaces per level**; a sequence item's nested lines sit two spaces past
its dash. CRLF is read as LF.

**Rejected.** The whole fence yields no defaults; tokens detected in the code are still reported, with
`""`. The error is `varsError: { code, params }`, on the **file** for its top fence and on the **block**
for a trailing fence or sub-set. Every code carries `params.line` (1-based within the fence body; `0`
for `vks.limit` on the body size). The first violation in line order is the one reported. Param values
in `code font` are the fixed vocabulary, part of the protocol like the codes themselves.

| Code | Extra params | Rejected |
|---|---|---|
| `vks.syntax` | `rule`: `dash_at_column_0` · `value_and_children` · `unclosed_quote` · `bad_escape` · `bad_plain` · `expected_entry` | `- ` at column 0; a key with both an inline value and children; an unterminated `'…'`/`"…"`; a `"…"` escape other than `\\ \" \n \t`; a plain scalar that starts with a reserved indicator not listed under `vks.unsupported`, contains `: `, or ends with `:`; text after a closing quote (other than ` #…`, which is `vks.inline_comment`); a bare `-`; any other line that is not an entry, item or comment |
| `vks.indent` | — | a tab in indentation; indentation not a multiple of two; an entry or item not at the indent its parent requires |
| `vks.bad_key` | `key` | a key outside the §9.1 key grammar (top level vs nested); includes `__proto__` |
| `vks.duplicate_key` | `key` | the same key twice in one map, at any level |
| `vks.unsupported` | `construct`: `flow_map` · `flow_list` · `anchor` · `alias` · `tag` · `merge_key` · `folded` · `block_indicator` · `block_in_list` · `nested_list` · `document_marker` · `directive` · `complex_key` | `{…}` and `{}`; non-empty `[a, b]`; `&`; `*`; `!`; `<<`; `>`; `\|+` or an indentation indicator (`\|2`); a block scalar as a list item; a sequence directly inside a sequence; `---` / `...`; a `%` line; a `?` key |
| `vks.mixed_list` | — | one list holding both strings and records |
| `vks.inline_comment` | — | ` #` after a value on the same line (`name: a # note`, `color: #fff`); never truncated |
| `vks.control_char` | — | a C0 control character or DEL other than tab; a newline is legal only as a line break or `"\n"` |
| `vks.limit` | `limit`: `body_bytes` · `depth` · `nodes` · `list_items` · `map_keys` · `block_defaults`; `max` | any §9.7 limit, checked before the work it bounds |
| `vks.mixed_dialect` | — | a non-comment line that is not a valid entry or item, contains `=`, and has no `:` before its first `=` — a `KEY=value` line of the removed dialect (§9.6) |

Emission has one more code, `vks.unrepresentable` (§9.5).

### 9.2 Typing (D-2)

**Every scalar is a string**, at every depth: `1`, `99.95`, `true`, `yes`, `null`, `~` are the strings
`"1"`, `"99.95"`, … . `key:` with nothing after it and no children is `""`. YAML quoting applies:
`name: "Alice"` is `Alice`.

### 9.3 Values (D-3)

A var's value is a **string**, a **list** (of strings, or of records — never mixed), or a **record**
(string keys → values), nested to the depth limit. The model stores the value only; the scalar
default is **derived** from it whenever it is emitted (so the two can never disagree): a string is itself; a list of strings → its first item (or `""` if empty); a list of
records or a record → `""`.

~~~md
```yaml VKs
VK-env:
  - dev
  - prod
VK-users:
  - id: 1
    name: Alice Smith
    tags:
      - core
      - ops
  - id: 2
    name: Bob Jones
    tags: []
VK-db:
  host: localhost
  port: "5432"
```
~~~

### 9.4 Multi-line values, comments, keys (D-5)

- **Multi-line strings:** `|` (keeps exactly one trailing newline) and `|-` (none), on map values only.
  Inside a list, a newline is expressible only as `"\n"` in a double-quoted item. Inside a block scalar
  at indent n, **every** line indented ≥ n+2, or blank, is content (a `#` line included); the block's
  indent (n+2) is removed. Trailing blank lines are stripped, then `|` adds one `\n`. An empty block is `""`.
- **Comments:** whole lines only, outside block scalars. `name: a # note` and `color: #fff` are **errors**, never truncated.
- **Keys:** see §9.1. A top-level key with `-` after `VK-` is legal but cannot be referenced by a token.
  **Duplicate keys** at any level reject the body.

### 9.5 Emission (D-7)

Strings, first rule that applies: `""` → nothing after the colon (`key:`; `- ''` as a list item);
contains a newline → `|-` / `|` on a map value, double-quoted with `\n` on a list item; fits `plain` and
is not a YAML-special word (`true false yes no on off null ~`, numerals, leading or trailing spaces) →
plain; otherwise single-quoted with `'` doubled. Structures: `key: []` for an empty list; `- item` lines;
records in **compact form** (`- first: v`, then the rest two spaces in); maps indented; two spaces per
level; key and field order preserved. The emitter **refuses** (`vks.unrepresentable`): any string
containing three backticks (it would close the fence) and a string with two or more trailing newlines.
An empty list is one value: `[]` reads as an empty list of strings and compares equal to an empty list
of records.

**Pinned at engine-port W2 (2026-10-10, B-RISK 4 + Role A pass 2):**
- *Numerals* are strings that parse as a floating-point number (`1`, `-2.5`, `1e3`, `inf`, `NaN`); *special
  words* match ASCII case-insensitively (`True`, `NULL`). Both are single-quoted.
- `|` / `|-` is used only when no non-empty line of the value is whitespace-only (the reader would turn it
  into an empty line) and the value is not all newlines; otherwise the value is double-quoted with `\n`
  escapes. So the rule order for a map value is: refusals → `""` → block (when it can carry the value) →
  double-quoted (multi-line) → plain → single-quoted.
- A **list item** is plain only if it is also not read as an item construct (`-`, `- …`, `<<…`, `%…`, `?`,
  `---`, `...`, `|…`, `#…`); otherwise it is quoted.
- An **empty record** (as a value or an item) is refused: `vks.unrepresentable{var, reason: empty_record}`.
  `vks.unrepresentable` params are `{var, reason}`, reason `backticks` · `trailing_newlines` · `empty_record`.
- Model-side checks (create/update, before emission) apply the §9.1 key grammar, §9.7 limits and the control
  rule to client values; U+2028 and U+2029 are rejected as `vks.control_char` (line splitting treats them as
  line breaks, so they cannot round-trip).

### 9.6 The `KEY=value` dialect is gone *(ruled — engine W7e, 2026-10-20; was D-6 dual-read)*

A defaults body is read as YAML (§9.1) only. There is no per-fence classifier and no unfenced `vars:`
section: a `KEY=value` line is `vks.mixed_dialect` (so a whole pre-§9 body is rejected with that code,
on its first line), and `vars:` followed by bare `KEY=value` lines after the code is plain text. An empty
or comment-only body still reads as no vars. The vks migration that converted old bodies was removed with
it (§11); a vault from before §9 needs converting with an engine older than 0.10.0 first.

### 9.7 Limits (checked before the work they bound)

| Limit | Value |
|---|---|
| fence body | 64 KiB |
| nesting depth (containers) | 6 |
| nodes (strings + containers) per body | 10 000 |
| items per list / keys per map (top-level vars included) | 1 000 / 255 |
| flagged regions × file defaults, per file (`block_defaults`) | 100 000 |

Counting: **depth** = containers inside one var's value (the top-level map is depth 0, so a var holding a
record is depth 1); **nodes** = every value (strings, lists, records), not keys and not the top-level map.
Each limit is checked before the push it bounds.
On emission, a body over 64 KiB is refused (`vks.limit{limit: body_bytes}`) by every writer (§13.1).

**`block_defaults`** bounds the parser, not the codec: every block of a flagged multi-block file (§7)
receives every file default, so the output grows with regions × defaults. The product is checked
before any per-block overlay; past it, the **file** carries `vks.limit{limit: block_defaults, max,
line: 0}` and the flagged payload falls back exactly as for a rejected defaults fence (§7.3: detected
tokens kept with `""`, no error on the blocks).

### 9.8 Round-trip (D-9)

`parse → serialize → parse` preserves strings (quotes, `=`, `:`, `#` mid-value, unicode), empty
strings and lists, one-item lists, records, nesting to the limit, key and field order, multi-line
values, sub-set headings and descriptions, untitled blocks. **Ceilings:** comments are dropped on write;
strings with two or more trailing newlines are refused at emission.

---

## 10. Structured values in code — paths, loops, implode *(ruled — W1; detection in W1, rendering in W3)*

Tokens beyond §4.1, all starting `<VK-`. The hint grammar can never contain `.` or `:`, so none of
these collides with a plain token, and **no variable name is reserved** (`<VK-each>` is still a plain
variable named `VK-each`).

```
path   := '<VK-' name ( '.' seg )+ '>'                     ; seg := [A-Za-z][A-Za-z0-9_]*
each   := '<VK-each:' name ( '.' seg )* ( ':' sep )? '>'
end    := '<VK-end:'  name ( '.' seg )* '>'
join   := '<VK-join:' name ( '.' seg )* ( ':' sep )? '>'
sep    := '"' ( any char except " \ > CR LF  |  \\ | \" | \n | \t )* '"'
```

The closing form `</VK-…>` and the adjacent pair exist **only** for plain tokens.

| Construct | Meaning |
|---|---|
| **Choice** — a plain token on a list of strings, outside any loop | resolves to the value the client supplies (any string, not only a listed item — W3 R-8), defaulting to the first item. Empty list → the token stays literal |
| `<VK-db.host>` (path through records only) | that string |
| `<VK-each:P>` … `<VK-end:P>` | repeats the body once per item of list `P`. Inside, a path **starting with `P`** resolves against the **current item**: `<VK-users.name>` in `each:users` is this user's name; `<VK-tags>` in `each:tags` is this tag |
| `<VK-each:P:", ">` | the separator goes **between** iterations, never after the last |
| nested `<VK-each:users.tags>` inside `each:users` | iterates **this user's** tags |
| `<VK-join:P>` / `<VK-join:P:" \| ">` | every string reached by `P`, **flattening every list on the way**, joined with `", "` (default) or the separator. Inside a loop, the loop prefix is the current item |
| **Block mode** | when **both** markers of a pair are alone on their lines (whitespace only), both lines are removed with their line terminators; the body is the text between them minus its final terminator (`\r\n` or `\n`); iterations are joined by separator + that terminator, followed by one terminator if there was at least one iteration (zero iterations remove the lines entirely). Otherwise inline: markers removed, iterations joined by the separator only. Default separator: `""` for `each`, `", "` for `join` |

~~~md
```js
const users = [
<VK-each:users:",">
  { id: <VK-users.id>, name: "<VK-users.name>", tags: [<VK-each:users.tags:", ">"<VK-users.tags>"<VK-end:users.tags>] }
<VK-end:users>
];
// all tags: <VK-join:users.tags:" | ">
```
~~~
with §9.3's `VK-users` renders:
```js
const users = [
  { id: 1, name: "Alice Smith", tags: ["core", "ops"] },
  { id: 2, name: "Bob Jones", tags: [] }
];
// all tags: core | ops
```

- **Failures never guess.** These leave the offending token(s) **literal** and add a warning
  (code + params) to the render result: an unknown variable; `each` on a non-list; an unterminated
  `each`; an `end` with no matching or a different `each` (markers must nest); a loop over a path it is
  already inside; a path or plain token reaching a list or record outside a loop (except Choice);
  `join` reaching a record; `join` with an empty result.
- **Failure scope:** a failed marker stays literal and the text between it and its partner renders
  once in the enclosing scope. Choice is a plain token resolved globally, also inside an unrelated loop.
  A loop over a path it is already inside means exact path equality with an open loop.
- **Warning params:** `name` = the full `VK-x` for plain tokens, `path` = the full `VK-a.b` for paths and
  directives (no `each:` prefix), `line` = 1-based line of the token (string). One warning per (code,
  name|path); warnings sorted by (line, code, name|path) (W3 R-25(7–10)).
- **Values are inserted raw** — no per-language quoting or escaping; the author writes the quotes.
- **Expansion limits** (checked during expansion, each before the work it bounds — W3 R-10):

  | Limit (`limit` param) | Max | Checked |
  |---|---|---|
  | `output_bytes` | 1 MiB | before every append, plain tokens included |
  | `iterations` | 100 000 loop iterations per render | before each iteration |
  | `steps` | 1 000 000 template nodes + value nodes visited | before each visit (bounds empty loops and repeated `join` walks that emit nothing) |
  | `depth` | 64 nested loops | while matching markers, before any recursion |

  Exceeding any **refuses** the whole render (`render.limit{limit, max}`), never truncates.
- **Warning codes** (each with `line` and `name` or `path`; one per code and token): `render.unknown_var`,
  `render.each_not_list`, `render.unterminated`, `render.unmatched_end`, `render.self_nested`,
  `render.not_scalar`, `render.join_record`, `render.join_empty`.
- The render result also reports whether the output contains a terminal control (`containsEscape`: any C0
  control except TAB and LF — a CR directly before LF is exempt — DEL, or any C1 control U+0080–U+009F), so
  terminal clients can refuse it uniformly.

---

## 11. Migrations *(ruled — engine-port W6 Role A pass 1, 2026-10-16)*

One migration: a **plan** op that writes nothing and an **apply** op that rewrites files surgically,
`migrate.frontmatter.plan|apply` (CLI `mda migrate frontmatter plan|apply`). Never automatic. (The vks
migration, `migrate.vks.plan|apply`, was removed at W7e with the dialect it converted, §9.6.) Rules:

- **Scope:** the six type directories (`Snippets/`, `Commands/`, `Templates/`, `AIAgentsConf/`,
  `AIPrompts/`, `Variables/`), walked once with one shared budget (depth 32, 10 000 entries, 64 MiB
  read; past it → `walk.limit{limit, max}`). Symlinked directories are **never descended** (deviation
  from the extension, which descends contained ones); a symlinked `.md` file inside the vault is read
  and written through (as `artifact.update`). `.obsidian/`, `.mda/`, `.git/`, `.trash/` are out of
  scope by construction.
- **Plan** → `{changes[…], skipped[{path, code, params}]}`; every change carries the file's `hash`.
  A plan never carries variable values.
- **Apply** takes `{files[{path, hash}]}` (≤ 10 000 entries; each path vault-relative, `.md`, under a
  type directory, else the whole request is refused `artifact.bad_path` before any I/O). Per file:
  bounded read (64 MiB budget across the call) → hash check (`file.conflict`) → the rewrite re-run on
  the fresh bytes → atomic replace. Result per file: `written{hash}` or `skipped{code, params}`
  (`migrate.up_to_date` when nothing is left to change), plus `written` / `skipped` counts. One file's
  failure never aborts the rest; a file the plan cannot read is listed in `skipped` with its code.
- **Bytes:** only the migrated span changes, except that the file is decoded lossily and written
  without a BOM, as every engine write (§1); the plan flags such files with `normalises: [bom |
  encoding]`. Idempotent: apply, then plan → no change.

### 11.1 Frontmatter — `type:` → `artifactType:`

Inside the first frontmatter block, when no `artifactType:` key exists, the first `type:` line whose
trimmed value is a legacy name (the registry type name in lowercase, plus `agent` → `AIAgentsConfig`;
`AIPrompt` has none) is rewritten to `artifactType: <TypeName>`, keeping its line ending. Unknown
values are left silently. Skipped with a code: a blank line before the frontmatter
(`migrate.leading_blank`), a quoted value (`migrate.quoted_type{value}`). Every other byte — `index:`,
`paths:`, unknown keys, block-list `tags:`/`paths:` (§1.1; not normalised) — is preserved.

### 11.2 *(removed — W7e)*

The `vks` migration (legacy `KEY=value` bodies → YAML) and its `migrate.unfenced_vars` report are gone with
the dialect (§9.6).

### 11.3 Validation — `vault.validate`

Same scope and walk as §11. Response `{files, errors, warnings, truncated, issues[{path, code, params,
severity}]}`, listing only files with issues; at most 10 000 issues are listed (`truncated: true`
past that; the counts stay exact). A block's `varsError` carries `block` in its params and is not
repeated when it equals the file-level one. **Errors:** a read failure (`file.too_large`,
`file.not_regular`, `io.failed`, `path.outside_root`) or a file/block `varsError`. **Warnings:** a
pending frontmatter migration (`migrate.pending{kind: frontmatter}`), a `## ` section with no defaults
fence in a Variables file (dropped by every rewrite), a mixed Variables file (§13.1), and a fence with
`VK-` keys that is not ```` ```yaml VKs ```` (`vars.untagged_defaults{info}`, §9). `mda vault
validate` exits 1 when `errors > 0`, else 0.

## 12. Engine configuration files *(ruled — engine-port W4 Role A pass 1, 2026-10-11)*

Two TOML files, read and written only by the engine (owner: `mda-vault/src/config.rs`). Both are untrusted input: each is capped at
**64 KiB** before parsing; a file that is not valid UTF-8 TOML, or breaks a rule below, is **`config.invalid{path}`** and is never
overwritten. Unknown keys are ignored on read and **preserved** on write; comments are not preserved.

### 12.1 Machine file — `<config dir>/config.toml`

`<config dir>` = `MDA_CONFIG_DIR` (absolute) if set, else macOS `$HOME/Library/Application Support/mda`, Windows `%APPDATA%\mda`,
otherwise `$XDG_CONFIG_HOME/mda` (absolute) or `$HOME/.config/mda`.

```toml
default = "/abs/path/to/vault"          # optional; must be one of `vaults`
vaults  = ["/abs/path/to/vault", "…"]   # known vaults: absolute, deduplicated by canonical path, at most 64
```

If no config dir can be determined (no `HOME`, or a relative `MDA_CONFIG_DIR`, which never falls back to the OS dir), operations that need the machine file return `config.invalid{path:""}`. The engine writes keys in alphabetical order; comments are not kept.

**Precedence:** `--vault <path>` (one process, never persisted; the machine file is then not read at startup) > a valid `default` > none. Stored paths are canonical. A `default` that no longer validates
does not stop the engine: operations that need a vault return `vault.unavailable{path}`.

**Vault validation** (`--vault`, `vault.select`, `vault.use`, the stored default): absolute → exists (`path.not_found`) → a directory
(`path.not_dir`) → contains a **directory** `.obsidian` (a symlink to one is accepted) else `vault.not_obsidian{path}`.

### 12.2 Vault file — `<vault>/.mda/config.toml`

Travels with the vault. Created on first write (`.mda/` included); contained to the vault root.

```toml
[types.Snippet]
enabled = false
```

Table names are `artifactType` values (§5). A type is **enabled** when its entry says so; with no entry, when its directory
(§5) exists. Enabling creates the directory; disabling never deletes it (directories are create-only). Whether a type is enabled
changes no engine operation — it is client UI state.

---

## 13. Variable sets behaviour *(ruled — engine-port W5 Role A pass 1, 2026-10-14)*

Engine behaviour over the §3 / §6 format (owners: `mda-core/src/variables.rs` mutations, `mda-core/src/varset.rs` scoring /
apply / save, `mda-vault/src/listing.rs` the walk).

### 13.1 Edits and shape transitions

Every edit rewrites the file through the §1.4 emitter (unknown frontmatter keys are not kept).
A sub-set is addressed by its index **and** heading; both must match. With no sub-set, an edit targets the flat (untitled) vars.

| From | Edit | To |
|---|---|---|
| `blank` | add a var (no sub-set) | `flat` |
| `blank` / `sets` | add a sub-set | `sets` |
| `flat` | add a sub-set | `sets`; the untitled block is titled `Default` (`Default 2`, … if taken) |
| `sets` | delete down to one sub-set | `sets` (one titled section stays `sets`) |
| `flat` / `sets` | delete the last var / sub-set | `blank` |

The flat (untitled) block is addressed as sub-set index 0 with an empty heading; renaming it gives a `sets` file, deleting it a
`blank` one. It takes no description. A file with an untitled fence **before** its first `## ` section cannot be edited this way
(`artifact.not_serializable{reason: variables}`): an edit would drop those vars. A `## ` section with **no** `vks` fence is not part of the
model (the parser skips it, §2) and is dropped by any rewrite (`artifact.update` and every Variables edit alike). The emitted fence body is capped at 64 KiB (§9.7)
for every writer. Var names follow the token grammar (§4.1, `VK-` + identifier, no dots); a prefix-less
key (`API_URL`) may be set, renamed or deleted, never added. Names are unique per sub-set; headings are trimmed, non-empty and
unique per file.

### 13.2 Listing and search

Variables files are found under `Variables/` at any depth. Symlinked directories are not followed. A call stops at depth 32,
10 000 entries (`.md` files and directories entered) or 64 MiB read (`walk.limit`, renamed from `vars.limit` in W6, M-2). A file that fails to read or parse is listed with its error; other `.md` files are
skipped; a missing `Variables/` lists nothing. Search is case-insensitive over file title, description and tags, sub-set heading
and description, var name and every string inside its value; a match keeps its ancestors and everything below it.

### 13.3 Scoring, apply, save

- **Score** of a non-empty sub-set against an artifact's variables (the render variable rule) and tags:
  `matchRatio × 0.7 + tagRatio × 0.3`. Results are ordered by score (descending), then path, then sub-set index.
  The response lists each file path once (matches refer to it by index) and the artifact's variables once; a
  sub-set's unmatched variables are the artifact's minus its matched ones.
- **Apply** returns the new values and one change per var: `filled` (old value absent or `""`), `overridden`, `kept`.
  Structured values are applied whole. Nothing is written.
- **Save** creates `Variables/<slug>.md` (or under a given `Variables/` subdirectory); the stem is the default stem of the
  title (§14.1: slugified, at most 252 bytes, `untitled` when empty) and must pass the vault stem rule (§14.1). An existing file is never overwritten. Empty values are kept (§3).
- **Render text** fills `<VK-…>` tokens in free text with values: the same response, rules and limits as rendering an
  artifact (§4.1, §10), with no file and no vault.

## 14. Vault file names and renames *(ruled — engine client-requests plan W2, 2026-10-29; ER-4, ER-6)*

### 14.1 The stem rule

A vault file is `<dir>/<stem>.md`. Every write that **creates a name** checks the **stem** of the last segment (the
`.md` excluded): `artifact.create`, the `newPath` of `artifact.rename`, the files `index.create` derives, and the
Variables files `vars.file.new` / `varsets.save` create. Directory segments keep the path rules (§8.3) and are not
name-checked. Updating, patching or deleting an existing file never checks its name, so no file a user already has
is stranded. The rule **rejects, never fixes**. Checks run **in this order**; the first failure decides the code:

| # | Rejected | Code |
|---|---|---|
| 1 | `<stem>.md` longer than 255 bytes (a stem over 252 bytes) | `naming.too_long{max: 255}` |
| 2 | empty after trimming | `naming.empty` |
| 3 | a leading or trailing space | `naming.edge_space` |
| 4 | a **leading or trailing** dot (`.hidden`, `trail.`) | `naming.edge_dot` |
| 5 | any of `\ / : * ? " < > \|` | `naming.illegal_char` |
| 6 | a control character: C0, DEL or C1 | `naming.control_char` |
| 7 | a Windows reserved name, whole or before the first dot (`CON`, `con.txt`), case-insensitive | `naming.reserved` |
| 8 | ends in `.md`, ASCII case-insensitive (`x.md` would be `x.md.md`) | `naming.md_extension` |

Unicode letters are allowed in a typed stem (`café`, `日本語`). Obsidian's link characters `# ^ [ ]` are allowed
(Obsidian allows the file; it warns only about links to it). The rule differs from the output-name rule (§5.1) in
the leading dot, the 252-byte stem and `naming.md_extension`.

**Default stem** of a title: `slugify(title)` (ASCII letters and digits, lower case, other runs → `-`), truncated
to 252 bytes, trailing `-` trimmed, `untitled` when empty (`Button Group` → `button-group`, `Café` → `caf`,
`日本` → `untitled`). A default that fails the rule (`Con` → `con`) is reported, never altered. `naming.check`
answers both questions without a vault: a given `fileName` is checked as given; without one the default stem of
`title` is derived and checked.

### 14.2 Rename

`artifact.rename` moves `path` to `newPath` **in the same type directory** (sub-folders allowed). `newPath` obeys
the artifact path rules and the stem rule; a target outside the type directory, also through an in-vault symlink,
is `artifact.bad_path`; `newPath == path` is `op.bad_request`. Every path rule is checked before the content hash
(`file.conflict`). An existing target is never overwritten (`file.exists`). Without a model the bytes move
verbatim (the hash is unchanged, the file is never parsed); with a model the moved file is written from it with
exactly the guards of `artifact.update`. The new file is published by hard link before the old one is removed, so
a crash leaves the old file, or both, never neither. A rename that changes only letter case on a
case-insensitive file system renames in place.

### 14.3 Links on rename

- **Template-index links** (§8.2) to the old path are **reported, never rewritten**: one warning
  `rename.index_link{index, link}` per (index, link) — `index` the index's vault path, `link` the raw link
  target (alias and anchor stripped). Only the old path's type directory is searched (a link resolves only
  below its index's own directory, §8.3), with the bounded walk (§13.2 caps); the comparison is ASCII
  case-insensitive. A walk that stops early adds one `rename.links_unchecked` (`limit`, `max` when a cap
  stopped it); the rename itself has succeeded.
- **Obsidian wikilinks** in other notes are neither scanned, rewritten nor reported. Obsidian resolves them by
  name and rewrites them only for renames made inside Obsidian.
