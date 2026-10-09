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
> variable bodies, structured values, directives) and are **proposed** until the engine-port plan's
> W1 refinement closes the open decisions they cite (D-2…D-11). Until the extension cuts over to the
> engine, parity with the extension's behaviour is measured by `conformance/`, not by reading code.

> Examples use `~~~` as the outer fence so inner ```` ``` ```` fences render literally. In a real file
> every fence is a standard triple backtick.

### Owners in the engine

| Concern | Module |
|---|---|
| Frontmatter, code fence, `##` blocks, flags, tokens | `mda-core/src/parse/` |
| The `vks` body (both dialects) | `mda-core/src/vks/` (codec) |
| Directive resolution (§10) | `mda-core/src/vks/` (template) + `mda-core/src/render.rs` |
| Emission | `mda-core/src/serialize.rs` |
| Artifact types, directories, per-type properties | `mda-core/src/registry.rs` |
| Language names, fence aliases, file extensions | `mda-core/src/language.rs` |
| Output file names (whole-file types) | `mda-core/src/naming.rs` |
| Index links and vault-authored relative paths | `mda-core/src/multi_index.rs` (`safe_rel_path`) |

Parse and validation failures are reported as **error codes with parameters** (e.g.
`vks.duplicate_key { key, line }`), never as prose. The code list lives in `mda-ops/src/error.rs`.

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
```vks
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
| `tags`, `paths` | inline array | `[a, b]`; brackets optional; split on `,`, trimmed, empties dropped. `paths` is index-only (§8) |
| `index` | boolean | **only the exact value `true`** is true (§8) |

- **`artifactType` is the only type key.** A legacy `type: <lowercase>` line is **not read** — not as
  a fallback, not as a second key. It is an unrecognised line; the other keys still parse, and the type
  falls back to the directory default. The way forward for such files is the frontmatter migration
  (`mda migrate frontmatter`, §11.1).
- **`index` and `paths` are read-side only**: parsed, never emitted (§8.7).
- A blank line after the closing `---` is optional.

### 1.2 Code block

The **first fenced block** after the frontmatter that is not a ```` ```vks ```` fence. Its info string
may be a language (`javascript`), `code`, or **empty** (plain text). Trailing whitespace is trimmed on
parse; the emitter trims too. A file may instead delimit its payload with **flags** (§7).

### 1.3 Variable defaults

Optional. Accepted forms, in priority order:

1. A ```` ```vks ```` fence (preferred). It may be preceded by a decorative label line (`vars:`,
   `vars`, `### VKs:`, prose); the label is ignored and the fence binds. The fence body is **YAML**
   (§9) or, for files written before §9, **legacy `KEY=value`** (§9.6). The engine reads both and
   **emits only YAML**.
2. **Legacy unfenced section:** a `vars:` / `vars` label directly followed by bare `KEY=value` lines,
   placed after the code block. Read forever; never emitted.

Keys use the full `VK-` prefix (`VK-host`). Variables are also **auto-detected** from the tokens in the
code (§4), so a defaults section is only needed to supply values. A var that appears only in the
defaults section is kept, after the detected ones.

### 1.4 Emission rules (single block)

- **Frontmatter key order:** `artifactType`, `title`, `description`, `language`, `extension`,
  `provider`, `model`, `version`, `tags`, `env`, `target`. Empty values are omitted (`artifactType`
  is always emitted). `index` / `paths` are never emitted.
- **Single-line values.** `title`, `description`, `env`, `target`, `extension`, `provider`, `model`,
  `version`: every `\r\n` / `\r` / `\n` becomes one space, runs of spaces collapse. A newline would
  otherwise inject a sibling key on re-parse. `tags` entries must not contain `,`, `]`, `\n` or `\r`
  (rejected at input; the emitter refuses as the last line of defence).
- **Language in both places:** a single-block file emits `language:` in the frontmatter **and** on the
  fence; the parser hoists the fence language into the model when frontmatter lacks it.
- **Plain text** (`language` empty): a bare ```` ``` ```` fence and **no** `language` key at all.
- **Defaults:** a `vars:` label then a ```` ```vks ```` fence, emitted **only if** at least one var has
  a non-empty value or a structured value (§9.3). Body per §9.5.

---

## 2. Multi-block structure

A file with one or more `## ` (h2) headings each followed by a fenced code block is a **multi-block
file**. Each section is a block.

- Sections split on `## ` **only**; `###`+ stays inside a block. An emitter never writes `## ` inside code.
- The text between a `## Heading` line and the section's code fence is that block's **description**.
  The frontmatter `description` is the file-level description; both may coexist.
- A section may carry its own defaults: a ```` ```vks ```` fence **anywhere after** its code fence
  (blank lines, a `### VKs:` marker, prose in between are ignored). Per-block vars = tokens detected in
  that block's code, overlaid with that fence's values (code order kept; vks-only keys appended).
- A section whose **first** fence is ```` ```vks ```` is a pure **sub-set** (Variables files, §3).
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

```vks
VK-port: "3000"
```

## Production

```bash
https://api.example.com
```
~~~

---

## 3. `artifactType: Variables` files

The content **is** the ```` ```vks ```` fence; there is no code fence. Used for environment variables
and for **variable sets** (§6).

~~~md
---
artifactType: Variables
title: Local services
env: dev
---

```vks
VK-api_url: http://localhost:3000
VK-db_url: mongodb://localhost:27017
```
~~~

- **Shapes**, classified from structure (never from a count):

| Shape | On disk |
|---|---|
| `blank` | frontmatter only |
| `flat` | one **untitled** ```` ```vks ```` fence |
| `sets` | one or more `## Title` + fence — **even one** titled section is `sets` |

- In a Variables file, vars are emitted **unfiltered**: an empty value is a key the user asked to
  record. A `## ` sub-set with no vars still gets an **empty** fence (```` ```vks ```` + ```` ``` ````),
  or the heading would be dropped on re-parse.
- Top-level keys in a Variables file **may** omit the `VK-` prefix (legacy files do: `API_URL=…`);
  such a key cannot be referenced by a token and is kept as written.

---

## 4. Variable tokens — `<VK-…>`

### 4.1 Plain tokens

- **Grammar:** `<` `/`? `VK-` *hint* `>`, where *hint* is `[A-Za-z][A-Za-z0-9_]*`.
- **The token is the variable name:** `<VK-host>` → `VK-host`. Matching, deduplication and var-set
  matching use the full name, case-sensitive.
- **Closing form `</VK-host>` is the same variable.** Both spellings detect, deduplicate and substitute
  identically.
- **Substitution is two passes:** an **adjacent** pair `<VK-x></VK-x>` collapses to the value **once**,
  then every remaining opening or closing token resolves individually. `<VK-a></VK-b>` is two tokens;
  `<VK-x> text </VK-x>` is two tokens.
- **Unknown variables stay literal**, so partial substitution is safe.
- **Value resolution, in order:** the value the client supplies → the default from §1.3 / §9 → otherwise
  the token stays literal.
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
  → frontmatter `extension:` (leading dot optional) → the fence language's extension (`language.rs`).
- **`extension:` and a typed name are path-injection surfaces:** a value containing `/`, `\`, `..` or
  NUL is **rejected, never sanitised**.

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
  extension-appended; absent → derived from the title. `target:` is a path-injection surface (same
  rejection rule as §5.1).
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
full `VK-…` name, exact and case-sensitive. (Scoring and apply behaviour are engine behaviour, not
format; see `AGENTS.md`.)

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
- **Text outside the flags is dropped.**
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

Tokens in the payload are auto-detected. A ```` ```vks ```` defaults fence is **file-level** and goes
**outside** the flags (anything between the flags is payload). Use a render-safe token spelling (§4.2).

### 7.4 Whole-file types

The region content is the payload, so the single-block rule (§5.1/§5.2) and output naming apply to a
flagged file exactly as to a fenced one. **Flags are read-side only:** the emitter writes the fenced
shape and never writes flags; flagged files are edited as raw text.

### 7.5 Flags are optional for whole-file types

Payload precedence, strictly ordered:

1. **Flags** — if any region exists, that is the payload.
2. **Code fence** — a ```` ```vks ```` fence does not count (it is the defaults section).
3. **Bare body** — **only for whole-file types** (`Template`, `AIAgentsConfig`): the whole body minus
   its `vars:` label and ```` ```vks ```` fence, with `language: markdown`.

`Snippet`, `Command`, `Variables` and `AIPrompt` never take rule 3.

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

- Both Obsidian forms, matched in **one pass** so document order is run order: wikilinks
  `[[target]]` (alias `|text` and anchor `#Heading` stripped) and markdown links `[text](target)`.
- **Duplicates are preserved** (a link written twice is two steps).
- **Images and embeds are not links**: `![alt](url)` and `![[Note]]` are ignored.
- **A link is single-line, and its target contains no `[` or `(`** — a hostile body of unclosed
  `[[[[…` must cost linear time. Such a target is unlinkable, never truncated into a different path.
- `.md` is appended when absent. Any other text is ignored.

### 8.3 Resolution and the rejection list

A link resolves **relative to the index file's own directory only**; there is no vault-wide search.
Every vault-authored string that becomes a path (link targets and `paths:` entries) goes through
**`safe_rel_path`**, which **rejects and never sanitises**:

| Rejected | Error code |
|---|---|
| `../escape`, `a/../../b` | `path.parent_segment` |
| `/etc/passwd` | `path.absolute` |
| `C:\tmp`, `file:///etc` | `path.drive_or_scheme` |
| `dir\sub\file` | `path.backslash` |
| any C0, DEL or C1 control character | `path.control_char` |
| `''`, `/`, `.` | `path.empty` |

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

### 8.7 Read-side only

`index` and `paths` are parsed and never emitted. Indexes are hand-authored.

---

## 9. The `vks` body *(proposed — closes at engine-port W1 refinement)*

The body of a ```` ```vks ```` fence. Decision references (D-n) point to the design record in the
engine-port plan; the info string stays `vks` (a ```` ```yaml ```` fence would be mistaken for the code
fence).

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

**Rejected** (each with its own error code; the whole fence yields no vars and the file carries a
`vars_error`): tabs; any other indentation; `- ` at column 0; a sequence directly inside a sequence;
a list mixing strings and records; flow maps `{…}` and non-empty flow lists `[a, b]`; `{}`; anchors
`&`, aliases `*`, tags `!`, merge keys `<<`; folded `>`; `|+` and indentation indicators; block
scalars inside sequences; `---` / `...`; `%` directives; `?` keys; duplicate keys at any level; a key
with both an inline value and children; control characters other than tab (and newline inside block
scalars or `"\n"`); any limit in §9.7 exceeded.

### 9.2 Typing (D-2)

**Every scalar is a string**, at every depth: `1`, `99.95`, `true`, `yes`, `null`, `~` are the strings
`"1"`, `"99.95"`, … . `key:` with nothing after it and no children is `""`. YAML quoting applies:
`name: "Alice"` is `Alice`.

### 9.3 Values (D-3)

A var's value is a **string**, a **list** (of strings, or of records — never mixed), or a **record**
(string keys → values), nested to the depth limit. The model keeps a derived scalar default for
every var: a string is itself; a list of strings → its first item (or `""` if empty); a list of
records or a record → `""`.

~~~md
```vks
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
  Inside a list, a newline is expressible only as `"\n"` in a double-quoted item.
- **Comments:** whole lines only. `name: a # note` and `color: #fff` are **errors**, never truncated.
- **Keys:** see §9.1. A top-level key with `-` after `VK-` is legal but cannot be referenced by a token.
  **Duplicate keys** at any level reject the body.

### 9.5 Emission (D-7)

Strings, first rule that applies: `""` → nothing after the colon (`key:`; `- ''` as a list item);
contains a newline → `|-` / `|` on a map value, double-quoted with `\n` on a list item; fits `plain` and
is not a YAML-special word (`true false yes no on off null ~`, numerals, leading or trailing spaces) →
plain; otherwise single-quoted with `'` doubled. Structures: `key: []` for an empty list; `- item` lines;
records in **compact form** (`- first: v`, then the rest two spaces in); maps indented; two spaces per
level; key and field order preserved. The emitter **refuses** (`vks.unrepresentable`): any string
containing three backticks (it would close the fence), a string with two or more trailing newlines, and
a var whose derived default disagrees with its value.

### 9.6 Legacy `KEY=value` bodies and the classifier (D-6)

- **Dual-read is permanent.** Per fence, the **first significant line** (not blank, not `#`) decides:
  `^[A-Za-z][A-Za-z0-9_-]*:(\s|$)` → YAML; anything else → legacy. The whole fence is read in that
  dialect. The unfenced `vars:` section is always legacy.
- **Legacy rules:** each line splits on its **first `=`**; lines without `=` or starting with `#` are
  skipped; name and value are trimmed; empty names are dropped; duplicates are kept in order (the last
  wins on overlay). **Values are literal:** `VK-value="active"` is the string `"active"`, quotes
  included. Control characters are rejected as in §9.1.
- Examples: `KEY=value: x` → legacy (`KEY` = `value: x`); `VK-url: http://a=b` → YAML;
  `VK-a:b=c` → legacy (`VK-a:b` = `c`); a YAML fence with a later `K=v` line → YAML error.
- **Writing any file converts its bodies to YAML**, value-preserving: `VK-value="active"` becomes
  `VK-value: '"active"'`, so the output a template produces is byte-identical before and after.

### 9.7 Limits (checked before the work they bound)

| Limit | Value |
|---|---|
| fence body | 64 KiB |
| nesting depth (containers) | 6 |
| nodes (strings + containers) per body | 10 000 |
| items per list / keys per map | 1 000 / 200 |

### 9.8 Round-trip (D-9)

`parse → serialize → parse` preserves strings (quotes, `=`, `:`, `#` mid-value, unicode), empty
strings and lists, one-item lists, records, nesting to the limit, key and field order, multi-line
values, sub-set headings and descriptions, untitled blocks. **Ceilings:** comments are dropped on write;
a legacy body is rewritten as YAML (value-preserving); strings with two or more trailing newlines are
refused at emission.

---

## 10. Structured values in code — paths, loops, implode *(proposed — closes at W1 refinement)*

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
| **Choice** — a plain token on a list of strings, outside any loop | resolves to the value the client supplies, defaulting to the first item. Empty list → the token stays literal |
| `<VK-db.host>` (path through records only) | that string |
| `<VK-each:P>` … `<VK-end:P>` | repeats the body once per item of list `P`. Inside, a path **starting with `P`** resolves against the **current item**: `<VK-users.name>` in `each:users` is this user's name; `<VK-tags>` in `each:tags` is this tag |
| `<VK-each:P:", ">` | the separator goes **between** iterations, never after the last |
| nested `<VK-each:users.tags>` inside `each:users` | iterates **this user's** tags |
| `<VK-join:P>` / `<VK-join:P:" \| ">` | every string reached by `P`, **flattening every list on the way**, joined with `", "` (default) or the separator. Inside a loop, the loop prefix is the current item |
| **Block mode** | an `each`/`end` marker alone on its line (whitespace only) removes that line, including its newline, and iterations are joined by separator + `\n`. Otherwise inline: joined by the separator only |

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
- **Values are inserted raw** — no per-language quoting or escaping; the author writes the quotes.
- **Expansion limits** (checked during expansion, before each append): rendered output ≤ 1 MiB, loop
  iterations per render ≤ 100 000. Exceeding either **refuses** the whole render (`render.limit`),
  never truncates.
- The render result also reports whether the output contains an `ESC` character, so terminal clients
  can refuse it uniformly.

---

## 11. Migrations

### 11.1 Frontmatter — `type:` → `artifactType:`

`mda migrate frontmatter --dry-run` lists every file whose frontmatter has a legacy `type: <value>` and
no `artifactType:`; `--apply` rewrites that one line to `artifactType: <PascalCase>` (`snippet` →
`Snippet`, `agent` → `AIAgentsConfig`, …) inside the first frontmatter block. Every other byte, `index:`
and `paths:` included, is preserved. Idempotent; never automatic; symlinks are followed only within
the vault.

### 11.2 `vks` bodies — legacy → YAML *(proposed)*

`mda migrate vks --dry-run` / `--apply` rewrites **only the bytes between a legacy ```` ```vks ```` line
and its closing fence** into YAML (§9.5), preserving values (§9.6). YAML fences are skipped
(idempotent); unfenced legacy `vars:` sections are **reported, not rewritten**; a legacy body containing
a control character is reported, not rewritten. Never automatic.
