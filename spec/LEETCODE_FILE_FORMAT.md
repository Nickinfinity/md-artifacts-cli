# LeetCode Exercise `.md` File Format — Authoritative Spec

> **Engine copy (D-23).** Imported into `md-artifacts-cli` from the LeetCode extension's
> `ARTIFACT_LEETCODE_FILE_FORMAT.md` at `84c437d` (2026-11-04). The engine owns the format from here: the
> discriminator is **`artifactType: LeetCode`** (the extension's lowercase `leetcode` is migrated by
> `migrate.frontmatter`, D-21, D-37). §5 and §6 are **client-owned and informative**: the engine does not
> run exercises. §10 is the wire shape the engine sends. If the engine's parser and this file disagree,
> that is a bug to reconcile.


This file is the **single source of truth** for the on-disk structure of an
`artifactType: LeetCode` vault `.md` file — what an author writes and what the
extension reads back. The parser (`src/services/leetcode-parser.service.ts` +
`src/services/leetcode-sections.helpers.ts`) implements this format; any writer
(the author by hand, or the extension's status / attempt writers) must produce
exactly this shape so `parse(write(x))` round-trips.

> **The discriminator is `artifactType:`, renamed from `type:`.** The rename is a
> **hard cut**: an artifact still carrying a bare `type: leetcode` and no
> `artifactType:` fails `verifyExercise` by name (§2.2). The vault was migrated
> in one pass; a hand-written file must use the new spelling.

> When authoring an exercise, writing a test fixture, or changing a writer, this
> file — not memory — defines the contract. If the parser and this doc disagree,
> that is a bug in one of them; reconcile, do not guess.

> ⚠️ **Keep this file in sync with the format.** Any change to the on-disk `.md`
> format — a new or renamed frontmatter field, a changed section heading, a new
> fence info-string, or a new `<!-- … -->` comment shape — **must** be reflected
> here in the *same* change, alongside the parser edit. This document is the
> contract; letting it drift from the parser defeats its entire purpose.

> Examples below use `~~~` as the **outer** fence purely so the inner ` ``` `
> code fences render literally. In a real `.md` file every fence is a standard
> triple-backtick.

---

## 1. Canonical file structure

**Frontmatter carries the artifact's *shape* and its summary; execution
configuration lives in the body**, in ` ```yaml leetcode ` fences placed next to
what they configure (§2.5). The shape — which of the three leetcode types this
is — is frontmatter precisely because the picker must read it without parsing
the body (§2.1).

~~~md
---
artifactType: LeetCode
leetcodeType: function
title: Two Sum
difficulty: easy
status: unsolved
algorithm: hash-map
tags: [leetcode, arrays, hash-map]
---

Problem description as Markdown prose.

```yaml leetcode
function: twoSum
params:
  - name: nums
    type: int[]
  - name: target
    type: int
returns: int[]
```

## Examples
```example
input: nums = [2,7,11,15], target = 9
output: [0,1]
```

```yaml leetcode
test:
  type: function
  timeoutMs: 5000
practice:
  timeLimit: 30
  locked: false
  options: [noCompletion, noAiAgents]
```

## Tests
```json
[
  { "input": { "nums": [2,7,11,15], "target": 9 }, "expected": [0,1] }
]
```

## Final Tests
```json
[
  { "input": { "nums": [1,5,3], "target": 8 }, "expected": [1,2] }
]
```

# Setup

## JavaScript
```javascript
function twoSum(nums, target) {
  // solution here
}
```

# Solutions

## Java
### Hash Map
```java
public static int[] twoSum(int[] nums, int target) { /* … */ }
```
~~~

**Order of the top-level blocks is fixed by convention** (frontmatter →
description → `## Examples` → `## Tests` → `## Final Tests` → `# Setup` →
`# Solutions` → `# Attempts`), but the parser locates each by its heading
regex, not its position — a section may be absent, and the extension appends
`# Attempts` after `# Solutions`.

**Config-fence placement is convention only — the parser is
order-independent** and reads every ` ```yaml leetcode ` fence wherever it
sits. The placement above is what the migrator writes and what a reader
expects; a hand-edited file that puts a fence elsewhere still parses
identically. See §2.5.

> The `practice:` block above is **illustrative, not a template**. A fence is
> written only for a key the file actually declares — most artifacts carry no
> `practice:` block at all, and an absent one stays absent rather than being
> materialised at its defaults.

---

## 2. Frontmatter

YAML between the leading `---` fences. It **must** open the file:
`FRONTMATTER_RE = /^---\r?\n([\s\S]*?)\r?\n---\r?\n?/`. Keys are matched by
`KV_RE = /^(\w+):\s*(.*)$/` — a key is `\w+` (letters/digits/underscore, **no
hyphens**). Unknown keys are silently ignored. Invalid `difficulty` / `status`
values are dropped, leaving the default.

**Frontmatter is summary plus shape, and carries no execution configuration.**
It holds exactly what Obsidian's Properties UI and the exercise picker read —
the seven fields in §2.2 — and nothing else. Everything that governs how the
exercise *runs* (`function`, `functions`, `params`, `returns`, `test`,
`practice`, `libs`, `checks`, `services`) lives in body config fences (§2.5).

The retained set is deliberately **exactly `LeetCodeSummary`**
([`src/types/leetcode.types.ts`](src/types/leetcode.types.ts)) plus the
`artifactType` discriminator, which is what keeps `parseFrontmatterOnly` — the
picker's one-read-per-directory-level fast path — reading frontmatter alone, and
keeps `patchFrontmatterField(raw, 'status', …)` writing where it always did.
`leetcodeType` joined that set rather than the body for exactly this reason: the
picker must know whether a row is a buffer or a tree without parsing the body.

### 2.1 The two axes

An artifact declares **two independent** things, and they answer different
questions. One value used to answer both, which is why four test types were
reserved and why a `service` could be opened but never graded.

| Axis | Answers | Declared in | Authority |
|---|---|---|---|
| **Leetcode type** | What *is* this artifact — one buffer, one package, several packages? | frontmatter `leetcodeType` | `LEETCODE_TYPES` ([`src/types/leetcode-type.ts`](src/types/leetcode-type.ts)) |
| **Test type** | How is a case delivered and compared? | `test.type` (single suite) **or** `checks[].kind` (per check) | `TEST_TYPES` ([`src/types/constants.ts`](src/types/constants.ts)) |

| `leetcodeType` | shape | meaning |
|---|---|---|
| `function` | `buffer` | One candidate buffer per language — a bare top-level callable. |
| `package` | `tree` | One buildable unit in one language: a Java package, a Python package, a Rust crate, a TS/JS package. **One file or many.** May declare libraries. |
| `stack` | `trees` | Several packages, each with its own language and ecosystem, wired to each other at boot. |

`shape` is the single field `isMultiFile` reads — it is a property of the
leetcode type, never a hardcoded set of test-type ids.

### 2.2 Field table

| Field | Type | Required | Default | Notes |
|---|---|---|---|---|
| `artifactType` | `'LeetCode'` | **yes** | — | Discriminator. **Read, and enforced**: `verifyExercise` fails an artifact where it is absent, where its value is not `leetcode`, or where a bare legacy `type: leetcode` appears without it ([`exercise-verify/frontmatter.rules.ts`](src/services/exercise-verify/frontmatter.rules.ts), Rules 1 and 3). The parser's `applyScalar` still ignores it — it decides whether a file is an exercise at all, not how one runs. |
| `leetcodeType` | `function`\|`package`\|`stack` | no¹ | *derived* | The shape axis. ¹Absent ⇒ **derived** from a legacy `test.type` (§2.3), so an unmigrated or hand-written file still parses. An unrecognised value falls back to the derived one **and warns**. |
| `title` | string | yes | `''` | Single line. |
| `difficulty` | `easy`\|`medium`\|`hard` | no | `easy` | Any other value → `easy`. |
| `status` | `unsolved`\|`attempted`\|`solved` | no | `unsolved` | **Extension-owned** — auto-written by Submit. Any other value → `unsolved`. |
| `algorithm` | string | no | — | Category tag (e.g. `hash-map`). |
| `tags` | string[] | no | `[]` | Inline `[a, b]` or a YAML `- a` list. |

Every other key is an execution-config key and belongs in a body fence — see
§2.5 for the list and for what happens when one is left here.

#### Canonical key order — a rule, enforced

**`artifactType` · `leetcodeType` · `title` · `difficulty` · `status` ·
`algorithm` · `tags`.** `verifyExercise` reports the first out-of-order pair as
a named failure (`orderViolation`,
[`frontmatter-order.helpers.ts`](src/services/frontmatter-order.helpers.ts)); the
order lives in exactly one constant, `CANONICAL_FRONTMATTER_ORDER`.

Three things this rule deliberately does **not** do:

- **It never affects reading.** The parser is order-independent, so a mis-ordered
  artifact still parses correctly and fails with a message instead of mis-parsing.
- **It never positions an unknown key.** A custom key (`source:`, a Dataview
  field) is skipped, not ordered, and never fails an artifact.
- **It cannot be violated by the extension itself.** `patchFrontmatterField` —
  the only frontmatter write the extension makes — replaces `status:` in place
  when it is present and **inserts it at its canonical index** when it is absent
  (§8), reading that index from the same constant.

### 2.3 Derivation and the hard cut — two mechanisms, deliberately different

| Key | Mechanism |
|---|---|
| `leetcodeType` | **Derived when absent.** The shape is recoverable from the legacy `test.type` without ambiguity, so derivation costs nothing and keeps hand-written artifacts working. |
| `artifactType` | **Hard cut.** A renamed discriminator is derivable from nothing; a bare `type:` is a named `verifyExercise` failure saying what to rename. |

```
leetcodeType absent  →  test.type function   →  leetcodeType function
                     →  test.type project    →  leetcodeType package
                     →  test.type service    →  leetcodeType stack
                     →  anything else        →  leetcodeType function
                        (absent, a typo, `__proto__` — all the single-buffer default)

leetcodeType declared but not one of the three ids
                     →  the derived value is used, and the parse WARNS

frontmatter carries `type: leetcode` and no `artifactType:`
                     →  verifyExercise FAILS: "legacy: 'type:' is renamed to 'artifactType:'"
```

**How the two axes are checked against each other, precisely.** There is no
general "these two values contradict" rule. The leetcode type instead selects a
**rule set** (`VERIFY_RULES`, keyed on the axis and compiler-exhaustive over it),
and each set states its own floors — which is what makes an artifact fail with
`package: no ## Files declared` rather than being measured against the function
floors it does not have. The one place the axes are read together is the mirror
rule (§2.5.1).

**Derivation is a default, not a dual read.** It runs on the **raw** declared
scalar, before the unknown-value fallback — by the time a `TestConfig` exists an
unrecognised `test.type` has already collapsed to the default, and deriving from
*that* would turn every unmigrated `project` artifact into a `package` graded as
a single empty `call` suite. `project` and `service` are accepted **only** as
derivation inputs and **only** when `leetcodeType` is absent; they are not
leetcode-type ids and never appear on disk as one.

**Both spellings are findable, and that is not a contradiction.**
`isLeetCodeArtifact` (the migrator's "is this file mine to rewrite?") accepts
`type: leetcode` **and** `artifactType: LeetCode`; `verifyExercise` ("is this
file conformant?") requires the new one. Collapsing the two would leave the
migrator unable to find the files it exists to fix.

**An artifact declaring both keys with the same value passes**, and it passes
because both rules are narrow by construction. `frontmatter.rules.ts` Rule 1
fires only on a `type:` whose value is exactly `leetcode` **and** no
`artifactType:` beside it; Rule 3 only requires `artifactType` to be present and
to equal `LeetCode`. So neither has anything to say about `type: leetcode` +
`artifactType: LeetCode`: the two assert the same thing, there is no wrong
answer to pick, and `type` is an unknown key thereafter — skipped, never
positioned, never a failure.

### 2.5 Config fences

Execution configuration is written in the body, in fences whose info-string is
`yaml` followed by the bare token `leetcode`:

~~~md
```yaml leetcode
test:
  type: function
  timeoutMs: 5000
```
~~~

**The marker.** `yaml` first, then a bare `leetcode` token. `yaml` first means
Obsidian still syntax-highlights the block (the first info-string token wins).
The `leetcode` token must be **bare**: a `## Files` entry always carries
`path=` (`parseFiles` skips every fence without it), so
` ```yaml leetcode path=src/x.yml ` is a *file*, not config, and never matches.
A plain ` ```yaml ` fence in prose stays prose.

**Do not put a fence-shaped line inside a config fence.** A config fence closes
on a line of exactly three backticks and nothing but whitespace; the
description's heading search, which it shares with section slicing, toggles on
*any* line opening with three backticks. A ` ```yaml ` line inside a config
fence body therefore desynchronises the two, and the description runs on past
the first heading — swallowing `## Examples` and everything after it as prose.
It degrades (over-included prose, still escaped) rather than losing data, but
the artifact will look wrong on the challenge screen.

**Content starts at column 0.** A config fence's body is concatenated with the
frontmatter text and parsed by the same `KV_RE = /^(\w+):\s*(.*)$/`, which is
column-0 anchored — so a top-level key must begin at column 0 exactly as it
would in frontmatter. Sub-keys indent normally beneath it. A top-level line
indented off column 0 is kept verbatim and **warned**, because it will not
parse as a key.

**The parser reads every config fence and calls `parseFrontmatter` once**, over
`frontmatter text + '\n' + every fence body joined in document order`. One call
means defaults are applied exactly once and there is no merge layer to
disagree with itself.

**Order-independence.** Placement is convention (§1, and the table below); the
parser does not enforce it. A fence anywhere in the body is read.

| Block | Canonical placement | Reason |
|---|---|---|
| `function` · `functions` · `params` · `returns` | after the description, before `## Examples` | the signature is what the description just described |
| `test` · `practice` | immediately before `## Tests` | they govern the suites that follow |
| `libs` · `packages` | before `# Setup` (a `function` artifact) or before `## Files` (a `package` / `stack`) | declared next to the code they install for |
| `checks` (nested under `test:`) | with the `test:` block, before `## Tests` | `## Tests` fences bind to checks by `check=` |

**A config fence is written only for a key the file declares.** No `practice:`
in the source means no `practice:` fence in the output — an absent block keeps
its defaults, and materialising one at its defaults is a semantic no-op that
only adds lines to read.

#### Duplicate keys across fences — the precedence is asymmetric

A top-level key repeated in two fences resolves **differently depending on the
key**, and the asymmetry is stated here rather than papered over with a uniform
rule the parser does not implement:

| Keys | Winner | Why |
|---|---|---|
| `function`, `functions`, `params`, `returns`, `test`, `practice`, `tags` | **last** occurrence | `parseFrontmatter` walks every line and overwrites the accumulator on each match. |
| `libs`, `checks`, `services` | **first** occurrence | `parseLibs` and `parseChecks` locate their block with `lines.findIndex(…)` and stop at the first hit. |

Either way the extractor **warns**, naming the key and which occurrence won.
Do not rely on the precedence: declare each key once.

#### The hard cut — an execution-config key left in frontmatter

There is **no dual read**. A key from the body set
(`function`, `functions`, `params`, `returns`, `test`, `practice`, `libs`,
`checks`, `services`) found in frontmatter is:

1. **ignored** — the parser does not read it;
2. **warned** — the warning names the key and the fence it belongs in;
3. a **`verifyExercise` failure** — such an artifact reports `ok: false`.

So a v1 artifact does not silently degrade into a half-configured exercise: it
fails loudly, with a message that says what to move where.

#### 2.5.1 `test:` block

Indented sub-keys, both optional:

| Sub-key | Type | Default | Rule |
|---|---|---|---|
| `type` | `TestTypeId` | `function` | Unknown value → `function` (a typo must not make the exercise unrunnable). **Declared only by a single-suite artifact** — see the mirror rule below. |
| `timeoutMs` | number | `5000` | Per case. Clamped to `[100, 60000]`. Suite budget = `cases × timeoutMs`, capped at 60 s. |
| `checks` | list | — | The per-check grading list of a `package` / `stack` (§9.2). Mutually exclusive with `type` — see below. |

**`test.type` and a check's `kind:` draw from one vocabulary** (`TEST_TYPES`,
[`src/types/constants.ts`](src/types/constants.ts)). They used to be two tables
that both listed `function` meaning different things; merging them is what lets
one multi-package artifact grade each check differently.

| id | delivers a case by | status **in the tree today** |
|---|---|---|
| `call` | positional args to a named callable via a generated driver | **implemented** — registered for java · javascript · python · rust · typescript, for `leetcodeType: function`. Also the one kind a check inside a tree may declare against a single file. |
| `program` | argv, named flags or stdin; compares what the program writes to `$LEET_OUT` | **implemented** for `leetcodeType: package` — registered for the same five languages. One **process per case**, so it is the one test type whose suite budget is not `cases × timeoutMs` (§2.5.5). |
| `http` | a real request to a booted server on an assigned loopback port | **implemented** as a check `kind:` — dispatched per check, never resolved through the registry |
| `build` | — | **implemented** as a check `kind:` — a declared argv exits `0` |
| `dom-assert` | a declarative `RenderStep[]` against a jsdom mount | **implemented** as a check `kind:` |
| `css-assert` | as above | **implemented** as a check `kind:` — a **declared** style property or class presence, never layout geometry |
| `class` · `in-place` | — | **reserved** |

**`function`, `stdin-stdout`, `project` and `service` are not ids at all any more.** `function` was the
legacy spelling of `call` and `stdin-stdout` of `program`; both are accepted as *derivation inputs*
(§2.3) and normalise to the current id. `project` and `service` were artifact **shapes** wearing a test
type's clothing — they are `leetcodeType` values (§2.1), never a `kind:` a check may declare, and
`project` is no longer a registry key either: the five stub envs behind it were deleted, so a tree
registers nothing and is graded by dispatching its checks.

**Only two rows go through the registry**, and that is the point of the table rather than an accident:
`call × {the five languages}` for a buffer, and `program × {the five}` for a package. Every other
implemented id is a check `kind:` dispatched by `runOneCheck` against an already-written directory, so
`languagesForType('build', 'package')` is `[]` and always will be — an empty cell there means *"not
resolved this way"*, not *"unimplemented"*.

A **reserved** id parses and validates, but nothing is registered, so
`languagesForType()` resolves it to `[]`, the language selector renders empty and
the panel explains itself instead of dying inside a compiler.

#### The two spellings are exclusive — the mirror rule

A `test:` block declaring `checks:` must **not** also declare a genuine `type:`,
and `verifyExercise` fails it by name
([`exercise-verify/package.rules.ts`](src/services/exercise-verify/package.rules.ts)):

```
package: checks declared but test.type is 'in-place' —
a checks-graded exercise must not also declare a top-level execution strategy
```

Without it a check-graded artifact that simply omits `type:` inherits the default
`function`, which the compatibility matrix permits for a `package` — so it
verifies as a single empty suite with no `params`, no `returns` and no cases,
while the checks that actually grade it are never consulted, and the mis-parse
survives all the way to a run.

**An absent `type:` is the target state, not a violation.** The migration deletes
that line from every `test:` block declaring `checks:`. Two values are therefore
tolerated beside `checks:` and only these two: the **default** (an absent `type:`
is indistinguishable from an explicitly declared default, because the parser
collapses both before the verifier sees them) and the **legacy shape markers**
`project` / `service`, so an unmigrated artifact is not failed twice for one
thing. Any other id is a deliberately named strategy and is refused.

#### 2.5.2 `practice:` block

Indented sub-keys, all optional:

| Sub-key | Type | Default | Rule |
|---|---|---|---|
| `timeLimit` | number (minutes) | `0` | `0`/empty ⇒ **unlimited**: the in-view clock counts **up** (elapsed) with a "no limit" label and never auto-submits. `> 0` ⇒ **bounded**: the clock counts **down** and auto-submits at zero. Negative / unparsable → `0`. |
| `locked` | boolean | `false` | Exactly the string `true` locks it. Locked ⇒ the panel disables the controls and the extension ignores the option list / time limit the webview posts, using the artifact's own block instead. |
| `options` | `PracticeOptionId[]` | `[noCompletion, noAiAgents]` | Inline `[a, b]` **or** a YAML `- a` list. Unknown ids are dropped. `options: []` ⇒ **no** restrictions. Absent `options:` ⇒ the defaults. |

`PracticeOptionId` ∈ `noCompletion` · `noAiAgents` · `noSnippets` ·
`noParameterHints`.

#### 2.5.3 `params:` block

Two accepted forms:

~~~md
params: []
~~~

~~~md
params:
  - name: nums
    type: int[]
  - name: target
    type: int
~~~

Only the multi-line `- name:` / `type:` form parses — each field on its own
indented line. The parser reads `key: value` lines only
([`assignParamField`](src/services/leetcode-parser.helpers.ts) → `KV_RE`), so a
YAML **inline flow-map** (`- { name: nums, type: int[] }`) does **not** parse:
the line starts with `{`, matches no `key:` pair, and the entry is silently
dropped. A `ParamDef` is kept only when **both** `name` and `type` are set. Any
non-empty value directly after `params:` (other than `[]`) yields an empty list
— the entries must be on the following indented lines.

#### 2.5.4 Signature fields

| Field | Type | Required | Default | Notes |
|---|---|---|---|---|
| `function` | string | yes¹ | `''` | Default/fallback function name to implement. Parsed onto `functionName`. ¹Required for `test.type: function`; a `project`/`service` artifact names its targets in `checks:` instead (§9). |
| `functions` | map `<lang>: <name>` | no | — | Per-language override of `function`. Keys resolve through the language-alias table (`py:` → `python`). Read via `functionNameFor(parsed, langId)`, never `functionName` directly, for language-specific code. |
| `params` | `{ name, type }[]` | yes | `[]` | Generic types (see §5). Input keys in `## Tests` must match these `name`s. See §2.5.3. |
| `returns` | string | yes | `''` | Generic return type. |

---

#### 2.5.5 `program:` block

Configures the `program` test type: how a case reaches the program, and which
file is its entry point. Only meaningful for `leetcodeType: package`.

```yaml leetcode
program:
  channel: argv        # argv (default) · flags · stdin
  entry: Main.java     # optional — defaults per language (below)
  flags: [--nums, --target]   # `flags` channel only, paired with `params:` in order
```

| Field | Type | Required | Default | Notes |
|---|---|---|---|---|
| `channel` | `argv` \| `flags` \| `stdin` | no | `argv` | How a case's `input:` map is delivered. An unknown value degrades to `argv` **and warns**. |
| `entry` | path | no | per language | Relative to the run directory. `Main.java` · `main.py` · `main.js` · `main.ts` · `main.rs`. |
| `flags` | string[] | no | — | Read only for `channel: flags`; paired positionally with `params:`. |

**A `program` package declares `## Tests` cases, not `checks:`** — the two are
mutually exclusive (§2.5.1's mirror rule), and the fork is read from the
artifact: a `program:` block with no `checks:` is graded as a suite, everything
else by its checks. `params:` is **required**, because it names the order a
case's input is serialised in.

Serialisation, per channel: a string value passes bare, anything else through
`canonicalJson`. On `argv`/`flags` a NUL byte is refused by name (Node rejects
it at the C level anyway; this turns an opaque spawn crash into a case-level
failure). `--flag value` is emitted as two argv elements, never `--flag=value`.

**One process per case, and the budget says so.** Unlike every other test type,
a `program` suite cannot share one process — argv and stdin differ per case — so
the budget is `cases × (timeoutMs + 2 s spawn allowance)`, capped at 180 s, and
each case is killed on its own clock. A slow case therefore fails alone instead
of spending the allowance of the cases after it.

#### 2.5.6 `$LEET_OUT` — where the graded value goes

**The graded value is not the program's stdout.** The harness mints one file
path per case and passes it as the `LEET_OUT` environment variable; the program
writes its answer there as JSON, and that file is what is compared against
`expected`.

```python
import json, os, sys
answer = solve(sys.argv[1])
with open(os.environ["LEET_OUT"], "w") as f:
    json.dump(answer, f)
```

Two consequences, both deliberate:

- **stdout is entirely the solver's.** `print` / `console.log` / `System.out`
  are free for debugging and cannot corrupt grading — unlike the `__LEET__`
  sentinel protocol the `call` types use, where stray output on the same stream
  is a real hazard.
- **A program that writes no file fails.** A missing `$LEET_OUT` is a failed
  case with a named reason, never an empty-and-therefore-green one — the same
  rule that makes a killed sentinel run fail rather than pass. The file is also
  size-capped (64 KiB) before it is read, so a runaway write fails its case
  instead of the editor.

The `# Setup` starter every language emits already carries this write.

## 3. Body sections

Everything after the closing `---`. Section headings are anchored, so they must
match exactly (trailing whitespace allowed):

| Section | Heading regex | Level | Slice ends at |
|---|---|---|---|
| Description | *(none — prose before the first `#`/`##`)* | — | first `^#+ ` heading |
| Examples | `^## Examples\s*$` | `##` | next `#`/`##` |
| Tests | `^## Tests\s*$` | `##` | next `#`/`##` |
| Final Tests | `^## Final Tests\s*$` | `##` | next `#`/`##` |
| Setup | `^# Setup\s*$` | `#` | next `#` only |
| Solutions | `^# Solutions\s*$` | `#` | next `#` only |
| Attempts | `^# Attempts\s*$` | `#` | next `#` only |
| *(config fence)* | ` ```yaml leetcode ` — **not a section** | — | its own closing fence |

`#`-level sections (Setup / Solutions / Attempts) are **not** terminated by
`##` sub-headings — those are the language headings inside them.

**A config fence is not a section.** It has no heading, it does not open or
close one, and it may appear inside any of them — the extractor collects fences
by their info-string, independent of section structure. The only interaction
with section slicing is the description (§3.1).

Section slicing is **fence-aware** (`sectionBounds`), which matters more in v2
than it did in v1: a column-0 `#` inside a config fence is a YAML **comment**,
not a heading, and must not truncate the section that contains it.

### 3.1 Description

The trimmed prose between the closing frontmatter `---` and the first Markdown
heading (`#` or `##`), **minus every config-fence span**. The whole body
(trimmed, minus config fences) when there is no heading.

**A config fence sitting between the description and `## Examples` is not part
of the description** — that is the canonical position for the signature block
(§2.5), and it falls inside the description slice by position. The parser
subtracts config-fence spans from the slice, so the fence contributes nothing
to `parsed.description`.

This is load-bearing, not cosmetic: the panel renders the description through
`renderMarkdownLite`, which has **no fenced-code rule**, so an un-subtracted
fence would show its raw YAML body as prose on the challenge screen.

A column-0 `#` line *inside* a config fence — a YAML comment — likewise does
not truncate the description.

### 3.2 `## Examples`

Zero or more ` ```example ` fenced blocks, each with an `input:` line and an
`output:` line. **Both are required** for the example to be kept; the values are
free-form display strings (not parsed as data).

~~~md
## Examples
```example
input: nums = [2,7,11,15], target = 9
output: [0,1]
```
~~~

### 3.3 `## Tests` and `## Final Tests`

Each holds case fences: an array of
`{ input: Record<string, unknown>, expected: unknown }`. The `input` keys must
match the `params` names. A malformed fence, a missing fence, or a missing
section all yield `[]` — the parser never throws.

**Two spellings are accepted, ` ```yaml ` and ` ```json `.** The info-string
says which grammar the body is written in, so this is not a dual read of one
thing — it is two notations for the same case list, each parsed by exactly one
parser. `yaml` is the preferred form and what the migrator writes; `json`
remains valid indefinitely.

~~~md
## Tests

```yaml
- input:
    arr: [1, -2, 0, 3]
  expected: 3
- input:
    arr: [5]
  expected: 5
```
~~~

#### The YAML here is **not** YAML 1.1 — it is JSON's type system with YAML's syntax

This is the single most important rule in this section, and it exists because a
silently re-typed *value* is worse than a loud parse failure: an exercise's
reference solution is graded against its own expecteds, so a coerced expected
makes the harness verify **green while teaching the wrong answer**.

| Unquoted scalar | Becomes | Note |
|---|---|---|
| `true` · `false` | boolean | exact lower-case only |
| `null` · `~` | null | |
| strict JSON number (`3`, `-0.5`, `1e3`) | number | |
| **everything else** | **string** | |

So every YAML 1.1 implicit type is **left as a string**: `yes`/`no`/`on`/`off`,
`y`/`n`, leading-zero octals (`0051` stays `"0051"`, not `41`), sexagesimals
(`1:1` stays `"1:1"`, not `61`), `.inf`, `.nan`. A **quoted** scalar is always a
string and is never re-typed.

A key must be followed by whitespace or end-of-line, so `1:1` and
`http://example.com` are scalars, not mappings.

**Not supported, by design** — each is a parser-complexity or ambiguity risk
with no use in case data: anchors and aliases (`&`/`*`), explicit tags (`!!str`),
multiple documents (`---`), block scalars (`|`, `>`), complex keys, merge keys.
Use flow style (`[a, b]`, `{k: v}`) for nested collections; the migrator does.

**Writing it by hand:** if a string could be read as a number, a boolean, `null`,
or contains `,` `:` `#` `[` `]` `{` `}`, quote it. The migrator quotes
automatically, and `emit → parse` is verified to be the identity on every value
JSON can express.

- `## Tests` — the **public** suite: shown in the panel, run by **Run Tests**.
- `## Final Tests` — the **hidden** grading suite, appended by **Submit**. Counts
  are shown (`2 public · 1 final`) but inputs/expected are never rendered. An
  exercise with no `## Final Tests` falls back to grading the public list at
  Submit time (resolved in `submitSuite`, never doubled in the parser).

#### An `http` case is a different shape — `request` / `expect`, not `input` / `expected`

`{ input, expected }` above is the shape for `call` and `program`. A case bound to a
**`kind: http`** check declares a request and the subset of the response to assert
([`http-case.helpers.ts`](src/services/test-envs/http/http-case.helpers.ts)). Writing an `http`
case in the `input`/`expected` shape fails with **`case 0: request must be an object`** — measured
on all four of the vault's pre-existing `stack` artifacts, which were authored before this kind was
dispatchable.

~~~md
## Tests

```yaml check="api filters by price"
- request:
    method: GET
    path: "/items?maxPrice=20"
  expect:
    status: 200
    body: {items: [{name: cable, price: 5}]}
```
~~~

| Field | Required | Rule |
|---|---|---|
| `request.method` | **yes** | One of `GET` `POST` `PUT` `PATCH` `DELETE` `HEAD` `OPTIONS`, case-insensitive. There is **no default** — an absent method is refused. |
| `request.path` | **yes** | A non-empty **path**, never a URL. See the loopback rule below. |
| `request.headers` | no | An object of **string** values; a non-string value is refused by name. |
| `request.body` | no | Any JSON value. **Refused with `GET` or `HEAD`** — the Fetch API rejects a body there, so it is refused here with a named reason instead of failing obscurely. |
| `expect.status` | no | An **integer**. A *quoted* status is a string and is refused — the type rule is deliberate. |
| `expect.headers` | no | Object of string values; each named header is compared individually. |
| `expect.body` | no | Compared as **JSON**: the response text is parsed, and a response that is not valid JSON fails by name. |

**Only what `expect` declares is asserted.** An undeclared field is never checked, so a case may
assert a status alone, a single header alone, or a body alone. Comparison runs through the same
`canonicalJson` every other test environment uses, so key order and whitespace never decide a case.

**The host is never artifact-supplied (S1).** A case names a *path*; the request is always built
against `127.0.0.1:<the port this run assigned>`. An absolute URL, a protocol-relative `//host/path`,
or anything else resolving off that host is **refused at validation, by name** — including an
absolute URL that merely *names* the loopback host. The rule is enforced twice, at validation and
again when the request is built, so it holds for a case that somehow skipped the validator.
Redirects are **not followed** (`redirect: 'manual'`): a `3xx` is returned as an ordinary observable
response, because the server answering is the artifact's own overlay and a `Location:` header would
otherwise carry the request off the loopback host.

**Every request is bounded** — a per-case timeout and a 64 KiB response-body cap, both of which
**fail** the case rather than silently truncating it into a comparison that could read green.

Cases bind to their check with a `check=<name>` fence attribute (§9). A bare fence binds only when
the artifact declares exactly one check.

### 3.4 `# Setup`

`## <Language>` headings, each with a fenced code block. Only the **first** fence
per language is taken — a setup is a single starter stub (the function
definition the solver begins from), never a labelled list. A language with no
setup falls back to generated boilerplate at Solve It.

### 3.5 `# Solutions`

`## <Language>` → optional `### <Label>` → fenced block. Multiple solutions per
language are allowed. Direct (unlabeled) fences under a `##` heading are kept
separately; when there are 2+, they auto-number `Solution #1`, `#2`, … — a lone
unlabeled fence keeps no label. Reference solutions are spoilers (collapsed
behind a `<details>` in the panel).

### 3.6 `# Attempts` *(extension-written)*

The attempt trail. `## <Language>` → each fenced block is preceded by a
**mandatory** `<!-- attempt: { … } -->` comment (see §4). `appendAttempt`
prepends, so entries are newest-first per language. A fence whose comment is
absent, malformed, or missing a required field is skipped (no partial entry).

> Authors normally do not write `# Attempts` by hand — the extension creates and
> maintains it on Submit. It is documented here because it is part of the
> on-disk format the parser reads back.

### 3.7 Fence info-strings

- ` ```example ` — examples.
- ` ```yaml ` / ` ```json ` — test suites (§3.3). A **bare** ` ```yaml ` fence
  inside `## Tests` / `## Final Tests` is case data; ` ```yaml leetcode ` is
  config even there, and is never read as an empty suite.
- ` ```yaml leetcode ` — a **config fence** (§2.5). `yaml` first so Obsidian
  highlights it; the `leetcode` token must be **bare** — a second token of the
  form `key=value` (e.g. `path=`) makes it a `## Files` entry instead.
- Setup / Solution / Attempt fences require a **language** info-string
  (`FENCE_LANG_RE = /```\w+\r?\n/`) — e.g. ` ```java `, ` ```python `,
  ` ```javascript `. A **bare** ` ``` ` fence is not matched and the block is
  skipped. Trailing whitespace inside a fence is trimmed on parse.

---

## 4. HTML-comment payloads

Both are a single-line JSON object inside an HTML comment, placed **immediately
before** the code fence they annotate. Parsed by `parseHtmlCommentJson`; never
throws (malformed ⇒ ignored).

**Solution meta** — optional, on a `# Solutions` fence:

~~~md
<!-- meta: { "solved_at": "2026-07-15T14:32:00Z", "duration": "8m22s" } -->
```java
public static int[] twoSum(int[] nums, int target) { /* … */ }
```
~~~

- `solved_at` — ISO-8601 string (optional).
- `duration` — `XmYs` string (optional).

**Attempt** — mandatory, on every `# Attempts` fence:

~~~md
<!-- attempt: { "at": "2026-07-15T14:32:00Z", "duration": "8m22s", "passed": true, "bigO": "O(n)", "confidence": "medium" } -->
```java
public static int[] twoSum(int[] nums, int target) { /* … */ }
```
~~~

| Key | Type | Required | Notes |
|---|---|---|---|
| `at` | string (ISO-8601) | **yes** | Missing/wrong-type ⇒ the whole entry is dropped. |
| `duration` | string (`XmYs`) | **yes** | Same. |
| `passed` | boolean | **yes** | Same. |
| `bigO` | string | no | Static Big-O estimate (e.g. `O(n^2)`). |
| `confidence` | string | no | `high` / `medium` / `low`. |

---

## 5. Generic → language type mapping *(client-owned, informative)*

`params[].type` and `returns` use language-neutral generics; `mapType(generic,
language)` translates them. Unknown generics pass through unchanged; unknown
languages return the generic as-is. Java boxes primitives inside generics
(`int` → `Integer`).

| Generic | Java | Python | JavaScript | Rust | TypeScript |
|---|---|---|---|---|---|
| `int` | `int` | `int` | `number` | `i32` | `number` |
| `float` | `double` | `float` | `number` | `f64` | `number` |
| `string` | `String` | `str` | `string` | `String` | `string` |
| `bool` | `boolean` | `bool` | `boolean` | `bool` | `boolean` |
| `int[]` | `int[]` | `List[int]` | `number[]` | `Vec<i32>` | `number[]` |
| `int[][]` | `int[][]` | `List[List[int]]` | `number[][]` | `Vec<Vec<i32>>` | `number[][]` |
| `map<string,int>` | `Map<String, Integer>` | `Dict[str, int]` | `Record<string, number>` | `HashMap<String, i32>` | `Record<string, number>` |

TypeScript's container syntax (`array`/`map`) is identical to JavaScript's — both are the
same structural type system at the declaration level the codegen writes to.

---

## 6. Test environments — capability matrix *(client-owned, informative)*

How a test executes is data, not a branch. A **test environment** is a
`(test type × language)` pair that also declares which **leetcode types** it
serves. `testEnvFor(testType, langId, leetcodeType)` returns the pair or
`undefined`; the absence of a pair **is** the capability matrix, and
`languagesForType(testType, leetcodeType)` drives the panel's language selector.

**The registry stays keyed `"<testType>::<language>"`** and filters on the env's
declared `leetcodeTypes`. A three-key table would be 3 × 8 × 5 slots almost all
empty, plus a second list to drift out of sync with the first. All three
arguments are **required**: an absent shape used to mean "any", and answering it
that way is what let a `stack` resolve a single-buffer environment.

| leetcode type | test type | Languages with an environment |
|---|---|---|
| `function` | `call` | `java`, `javascript`, `python`, `rust`, `typescript` |
| `package` | `program` | `java`, `javascript`, `python`, `rust`, `typescript` |
| *(any)* | `http`, `build`, `dom-assert`, `css-assert` | *(none — dispatched per check, never registered)* |
| *(any)* | `class`, `in-place` | *(none — reserved)* |

Read that table as *the registry today*. Two rows that used to be in it are
gone, and both for the same reason: `function` was renamed `call` on this axis,
and `project` was never a way to deliver a case at all — it was the artifact's
shape. The five `project` envs keyed on it refused every candidate they were
handed, so what they registered was a claim that a tree could be graded
*through the suite runner*, which it never can. A tree is graded by dispatching
its checks (§9.2).

**`program` is registered but is not a `TestEnv`.** `RegisteredEnv` is a union:
a `TestEnv` runs a whole suite in one process and recovers outcomes from
`__LEET__` sentinel lines, while a `ProgramEnv` starts **one process per case**
and reads each answer back from `$LEET_OUT` (§2.5.5, §2.5.6). Both are in the
registry because it answers *can this triple be graded?* — a different question
from *how does a suite execute?* — and every caller narrows with `isBatchEnv`
before reaching the batch runner. It is registered for `package` only: a
`stack` boots several packages, which is the `http` axis, not this one.

**A check `kind:` is not looked up here.** `build`, `dom-assert`, `css-assert`
and `http` have **no** registry entry — `languagesForType('build', 'package')`
is `[]` — because they are dispatched per check by `runOneCheck` against an
already-written directory, not resolved per language before a buffer is
compiled. Both questions draw from one vocabulary; only one of them goes
through the registry.

`build` and `call` checks are language-agnostic. The two **render** kinds are
narrower: `dom-assert` / `css-assert` mount a JavaScript bundle in jsdom, so a
check whose file is python, rust or java is refused at validation, naming the
language. `javascriptreact` / `typescriptreact` are display ids with no runtime
of their own, and a `.jsx` / `.tsx` file maps onto the runnable pair at bundle
time. An `http` check is language-agnostic in a different way again: what it
grades is a **process**, booted from the argv its `packages:` entry declares, so
no language is consulted at all.

**A `call` check inside a tree resolves the buffer environment, not a tree one.**
`runOneCheck` asks `testEnvFor('call', <the file's language>, 'function')`: the
check extracts one file and grades it exactly as a single candidate buffer, so
the shape asked of the registry is the buffer's, not the artifact's.

### 6.1 Opening is one question, grading is another

They are answered by different authorities, and conflating them is what let a
`service` artifact parse a tree nothing ever opened.

- **Can it be opened?** `isMultiFile(leetcodeType)` — `shape !== 'buffer'`, so
  `package` and `stack` both materialise their `## Files` tree and open the tabs.
- **Can it be graded?** The registry, plus a refusal consulted **before anything
  is written**: an artifact declaring any check `kind:` that no environment
  implements is **ungradeable as a whole**, not merely stripped of those checks.
  The reason names the kinds:

  ```
  checks declare kind(s) no environment implements yet: class —
  the whole artifact is ungradeable, not just the checks that parsed.
  ```

  Grading only the survivors is the false-green vector this closes: a tree whose
  `class` checks were dropped at parse time would otherwise be graded on its
  surviving `build` check and reported **solved**.

  The example used to be `http`, and the change is worth stating rather than
  quietly editing: `http` is **dispatched** now, so the artifacts that were
  refused by name are graded. What still lands here is a kind the vocabulary
  declares and nothing runs — `class`, `in-place`, or `program` named as a
  check `kind:` rather than as a suite `type:`.

**Verification is a third question, and it deliberately answers differently.**
`verifyExercise` still reports structure-only `ok` for such an artifact — *well
formed* and *executable* are not the same claim — and the CLI says so out loud
rather than printing a bare `OK`:

```
structure only for kind(s) class — no environment implements them, nothing executed
```

**A `stack` is skipped by the default sweep.** `verify-exercise.mjs` prints
`SKIP … — stack, LEET_STACK_E2E not set` and exits `0` unless `LEET_STACK_E2E=1`
is in the environment. A stack installs several ecosystems and boots several
servers; a gate that goes from seconds to minutes and needs a network is a gate
that stops being run. A `package` declaring an `http` check boots one server and
stays in the default sweep.

The five `function` environments are self-contained (the extension ships zero
runtime dependencies). The solver's code is written verbatim as its own file and
a generated driver links to it — the candidate is **never** spliced into a
wrapper, and must be a bare declaration of the function, never a program that
reads stdin.

**Candidate contract — pure per call.** A suite runs as **one process (one
module/`vm` context) per suite**, not one per case: the driver loads the
candidate once and calls it for every case in order, emitting one `__LEET__`
sentinel line each. So module-level and global state **persists across the cases
of a suite** — a memo table, a counter, a mutated module array survives into the
next case, and the same candidate graded case-by-case would behave differently.
A candidate must therefore be **pure per call**: its answer for a case may depend
only on that case's arguments, never on state left behind by an earlier one.
Authors must not write an exercise whose expected values require cross-case
state, and must not rely on per-case isolation to reset a global. (This is
by design — one process per suite is what makes a compiled language pay `javac`
once instead of once per case.)

**Per-language ceilings.** Rust serialises results through a local `LeetJson`
trait (compact, key-sorted JSON) rather than serde or `{:?}` Debug — neither
reliably matches `canonicalJson`'s formatting — so structs and enums are not
yet supported as param/return types. TypeScript never invokes `tsc`: the default path
strips types in-process with Node's own `stripTypeScriptTypes` and runs the
result in a `vm` sandbox, gated on `detect()` finding Node ≥ 22.18 (22.x line)
or ≥ 23.10 (23.x line) — older Node drops TypeScript out of the language
selector rather than failing mid-run.

---

## 7. Language headings and aliases

`## <Language>` heading text is stored lower-cased as authored, then resolved to
a canonical `languageId` downstream via `resolveLangId` (the same table the
`functions:` keys use). So an aliased heading (`## JS`, `## py`) groups under its
canonical id (`javascript`, `python`) and matches the language selector. Use the
canonical name when in doubt: `JavaScript`, `Python`, `Java`.

---

## 8. What the author writes vs. what the extension writes

| Part | Written by |
|---|---|
| Frontmatter (except `status`), **every ` ```yaml leetcode ` config fence**, description, `## Examples`, `## Tests`, `## Final Tests`, `# Setup`, `# Solutions` | **Author** |
| `status:` frontmatter field | **Extension** — `solved` on an all-green Submit, `attempted` on a failing live Submit. A dry run against a stored solution never writes it. |
| `<!-- meta: … -->` on a solved solution | **Extension** — on a successful Submit. |
| `# Attempts` section + `<!-- attempt: … -->` entries | **Extension** — every live Submit (pass or fail), newest-first, in a single read-patch-write alongside the `status` update. |

`status` + the solved `meta` + the attempt entry are applied in **one**
read-patch-write per Submit, so a manual edit made between a run's start and its
Submit is preserved everywhere except those three writer-owned spots.

**The extension cannot author an order violation.** `patchFrontmatterField` —
its only frontmatter write — replaces `status:` **in place** when the key is
present, and **inserts it at its canonical index** (§2.2) when it is absent,
reading that index from the same constant the verify rule checks against. This
is not a detail: a vault artifact that has never been submitted carries no
`status:`, and appending one before the closing `---` would land it after
`tags:` — the extension writing a file its own verifier then rejects.

**The extension never writes a config fence.** Config fences are author-owned
in full: nothing in the extension creates, rewrites, reorders or deletes one.
The only body writes it makes are `# Attempts` and the solved `<!-- meta: … -->`
comment, and its only frontmatter write is `status:` — so a config fence, and
the placement the author chose for it, survives every Submit untouched.

The **migrator** (`scripts/migrate-artifact-format.mjs`) is the one tool that
writes config fences, and it is a one-time author-side v1 → v2 transform run
explicitly from the CLI, not part of the extension.

---

## 9. `package` and `stack` — multi-file exercises

> **These are `leetcodeType` values, not test types** (§2.1). The ids `project` and
> `service` that used to sit here were an artifact *shape* wearing a test type's clothing;
> they survive **only** as derivation inputs (§2.3). `project/` remains a directory name
> under `src/services/test-envs/`, never an id an author writes.
>
> **`project` is not a registry key either, and this line used to say it was.** T3.5 deleted
> the five `projectEnvFor` stubs: each refused every candidate it was handed, so what they
> registered was a claim that a tree could be graded *through the suite runner*, which it never
> can. A tree registers **nothing** — `languagesForType('build', 'package')` is `[]` and always
> will be — and is graded by dispatching its checks instead. An empty cell there means *"not
> resolved this way"*, never *"unimplemented"*.

> **Status, precisely.** A `package` is **parsed and graded**: `## Files`, `libs:`, `checks:`
> and the `check=<name>` case binding all land on the parsed artifact, and all **five** check
> kinds — `dom-assert`, `css-assert`, `build`, `call`, and now `http` — dispatch through
> `runOneCheck` and grade end to end. `packages:` is parsed (`parseLeetCode` calls
> `parsePackages`, and a parsed artifact carries `packages`) and **reached**: an `http` check
> resolves its `package:` name against that list, boots the one server it names
> (`bootServer`), fires every bound case at it in request/response order, and tears it down on
> every path out. A `package` declaring one `http` check is graded exactly like one declaring
> only `build`/`call`/`dom-assert`/`css-assert` — see the `package`/`http` smoke artifact,
> `CoderByte/Tests/package/sum-endpoint.md` in the vault.
> A `stack` **parses, opens and boots**: it is a file tree, so *Solve It* materialises
> `## Files` and opens the tabs, and `bootStack` (VSX-178) now installs every package, boots
> them in `dependsOn` order, and computes each `exposeAs` value from the **already-assigned**
> ports of the packages it depends on (§9.3). It also has its own **verify rules**
> (`stack.rules.ts`, VSX-181): more than one package, every `http` check naming a package the
> artifact actually declares, and a `call` check only where top-level `params:` give its cases a
> shape — everything else delegating to the `package` rules rather than restating them. What a
> `stack` still lacks is any worked artifact of its own.
> A `stack` is therefore still **skipped** by the default vault sweep
> unless `LEET_STACK_E2E=1` (§6.1) — not because `http` is unimplemented any more, but because
> exercising several booted ecosystems at once is comparatively slow and network-bound. With
> the flag on, a `stack` whose checks each resolve to one already-implemented kind (a
> single-package `http` check, a `build`) grades exactly as a `package` would.
> Reference artifacts live in the **Obsidian vault**, not in this repo — see CLAUDE.md,
> *Artifacts live in the vault*.

- **`package`** — one buildable unit in one language, opened as several editor tabs and
  graded by declared **checks** rather than one return value. **One file or many.**
- **`stack`** — several packages, each with its own language and ecosystem, wired together at
  boot. A package's checks may be any declared `kind:` — `call`, `build`, `dom-assert`,
  `css-assert` — exactly as inside a standalone `package`; `http` is simply the one kind that
  talks to a server the extension boots on an assigned port.

### 9.1 `## Files`

One fenced block per file, the info-string carrying the language then attributes:

````markdown
```typescript path=src/lib/catalogue.ts role=editable
export function filterInStock() { /* … */ }
```
````

| Attribute | Values | Meaning |
|---|---|---|
| `path` | POSIX-relative | Location inside the run directory. Never absolute, never `..`, and never containing a `node_modules` segment **at any depth** — that name is **reserved**: a run directory's `node_modules` holds symlinks into a package cache shared by every exercise, and a declared write through one would leak into all of them. A `stack` links a tree per sub-package, so `client/node_modules/react/index.js` is exactly as dangerous as a root-level one. |
| `role` | `editable` | Written and opened — the solver's work. **The default** when `role=` is absent. |
| | `readonly` | Written and opened with a **read-only file mode**, not to be edited. VS Code has no per-editor config scope, so file mode is the mechanism. |
| | `hidden` | Written, never opened — scaffolding the solver should not see. |

A fence with **no** `path=` is prose, not a file, and is skipped. An unrecognised `role=`
degrades to `editable` **with a warning** — a silent drop reads exactly like an artifact
that never declared the role. The language is the info-string's first token, resolved
through the usual alias table (`tsx` → `typescriptreact`, `css` → `css`).

Paths are reported by the parser exactly as written and are normalised and
containment-asserted by the **writer**, immediately before it writes — one authority, at
the point of use, rather than a check the parser could be bypassed around. That same
authority (`resolveContained`) refuses any path with a reserved segment at **any** depth,
per normalised segment and never as a raw-string check — so `client/node_modules/react` is
caught while `my_node_modules/x` is unaffected (`src/../../node_modules/x` is refused one step
earlier still, by the escape check, having left the run directory) — and it
is the one rule shared by the `## Files` writer, a `build` check's `dir:`, and a `function`
check's `file:`, so the reservation holds no matter which door a path arrives through.

The comparison is **case-folded** (NFKC, then lower-cased), because APFS and NTFS fold case
and `NODE_MODULES/react/index.js` reached the same directory on disk. Linux consequently
over-refuses a directory genuinely named `NODE_MODULES` — the deliberate trade, since a
guard whose safety depends on which machine graded the artifact is worse than a uniform one.

A file **no check references is ungraded scaffolding, explicitly** (the CSS tab exists
for the solver, not the grader).

### 9.1b `# Solutions` overlays — how a project ships unsolved *and* verifies

`## Files` holds the **starter** a solver begins from, so grading the tree as authored would
fail every well-formed exercise by design. A `project` therefore puts its reference
implementations in `# Solutions`, as fences carrying the **same `path=`** as the starter they
replace:

````markdown
# Solutions

```javascript path=src/App.jsx
export default function App() { /* the working component */ }
```
````

- The verification harness (`verify-exercise.mjs`) grades the tree **with** the overlays
  applied — so a green result means the reference works.
- A solver's run uses `## Files` as authored; overlays are reference material, exactly as
  `# Solutions` already sits behind a spoiler in the panel for a `function` exercise.
- An overlay naming a path `## Files` does not declare is appended rather than dropped.
- A `# Solutions` fence **without** `path=` is an ordinary function-type reference solution
  and never touches a project's tree, so both grammars coexist in one section.

**A starter that passes is a bug in the exercise**, not a convenience: check it by grading the
tree without overlays and confirming it goes red.

### 9.2 `checks:` — how a project is graded

A body config fence (§2.5), canonically placed before `## Tests` so it sits with
the cases that bind to it:

````markdown
```yaml leetcode
test:
  timeoutMs: 10000             # no `type:` — see the mirror rule, §2.5.1
  checks:
    - name: catalogue filter     # unique; results group by it
      kind: call                 # runs on the existing `call` environments
      file: src/lib/catalogue.ts
      function: filterInStock
    - name: app builds
      kind: build                # declared argv must exit 0
      dir: client                # optional, relative to the run directory
      argv: ["npx", "tsc", "--noEmit"]
    - name: orders api
      kind: http                 # boots a `packages:` entry and grades responses
      package: api               # the entry's name — no `file:`
    - name: order list
      kind: dom-assert           # mounts client/src/App.jsx and asserts the DOM
      file: client/src/App.jsx
      package: api               # optional — which booted package its `fetch` may reach
```
````

**No `type:` line.** A check-graded artifact declares `checks:` and nothing else on that
axis; declaring both is a named verify failure (§2.5.1).

Note the `#` comments above are **inside** a fence and are YAML comments, not
headings — §3 covers why that distinction is load-bearing for section slicing.

A `kind:` draws from the one test-type vocabulary (§2.5.1), narrowed to the ids
`runOneCheck` can actually dispatch:

| `kind` | Compares | Status |
|---|---|---|
| `call` | one file's export, through the five `call` environments; needs `file:` and `function:` | **dispatched** |
| `build` | a declared argv **array** exits 0; optional `dir:` runs it in a contained subtree | **dispatched** |
| `css-assert` | **declared** style: inline/`style` properties and class presence; needs `file:`, optional `package:` | **dispatched** |
| `dom-assert` | DOM after mounting the component and firing events; needs `file:`, optional `package:` | **dispatched** |
| `http` | a real request to a booted server on an assigned loopback port; needs `package:`, never `file:` | **dispatched** |
| `program` · `class` · `in-place` | — | reserved: parses, then the whole artifact is refused for grading (§6.1) |
| `function` · `project` · `service` | — | **not kinds at all.** `function` was renamed `call`; the other two are shape ids (§2.5.1). A check declaring one is *unknown*, not reserved, and is dropped with a typo-style warning. |

The two are told apart deliberately, because they send an author to different places:
`kind: htpp` is a spelling mistake they can fix, while `kind: class` is spelled correctly and
names a contract this extension has not implemented yet. Both are **dropped** from `checks`
at parse time — a check nothing can run must not reach the panel's check line, and must
never count as a red check for `--starter-red`, which requires a starter to fail *on its
merits*. The drop is hygiene; the artifact-level refusal (§6.1) is what makes it safe.

**A malformed check is a third case, and it is neither.** An `http` check with no
`package:`, or a `call` check with no `function:`, is dropped as the author's own
mistake — named in a warning, and **not** recorded as an unimplemented kind, so it
does not make the artifact ungradeable. The distinction is which side the gap is
on: this extension's, or the artifact's.

`css-assert` never asserts layout geometry — the render environment is jsdom, which
computes no layout, so a width-from-box-model assertion is refused rather than silently
passed.

**`dom-assert` / `css-assert` may bind `package:` too (T4.8, VSX-228)** — which booted
`packages:` entry the mounted component's `fetch` may reach, the same field an `http` check
carries. It exists for a `stack` whose component talks to more than one booted package: with
no binding, exactly one booted package resolves automatically (the S1 worked example — one
backend, one frontend), but two or more **refuse the check by name** rather than guess which
server the component meant, because guessing would point it at the wrong one and grade
whatever came back. A binding resolves directly against that name — booted → its port;
declared in `packages:` but never booted → refused, carrying the boot's own failure reason; a
name this artifact never declared at all → refused by name, never a silent fallback to the
ambiguous-count guess. This is what makes a two-package stack (a Flask backend plus a React
frontend, say) gradable at all: before this field, any render check in a stack booting more
than one package was refused outright.

Unlike an `http` check's `package:`, this one is **optional** — a `dom-assert`/`css-assert`
with no `package:` still resolves the S1 case (one booted package) exactly as before, and a
`package`-shaped artifact (never batch-booting anything up front) reads the field, if present,
as ignored: nothing was booted to resolve it against, so the component's `fetch` refuses every
request exactly as it does with no binding at all.

**Zero or one `function` check per project.** `params:` / `returns:` are artifact-level
singletons, so a second function check would have nowhere to declare its own types; a
project may equally have **none** (graded entirely by `dom-assert` / `css-assert` /
`build`), in which case `params:` and `returns:` are optional and the function structural
floor — the 6-public / 3-final case counts — does not apply to it.

**Solved = every check green.**

#### Binding cases to a check

A ` ```json ` fence in `## Tests` / `## Final Tests` carries a `check=<name>` attribute, and
**every** fence in the section is read (not just the first):

````markdown
## Tests

```json check="catalogue filter"
[ { "input": { "minStock": 3 }, "expected": ["keyboard"] } ]
```

```json check=alternates
[ { "input": {}, "expected": "X" } ]
```
````

The public/final split is therefore **per check**: a check's cases are its `## Tests` fences
followed by its `## Final Tests` ones, and *Run Tests* grades only the public leading slice.

Quote a name containing spaces. A bare fence with no attribute binds to the **sole** check
when the artifact has exactly one — the common single-check project — and warns otherwise
rather than guessing. A fence naming a check that does not exist warns too. Public fences
bind before final ones, so a check's cases keep public-then-final order.

This widened fence applies to **every** artifact, `function` included: a second json fence
under `## Tests`, previously ignored in silence, is now appended to the suite, and one
malformed fence costs only its own cases.

### 9.3 `packages:` (`leetcodeType: package` · `stack`)

A body config fence (§2.5), canonically placed before `## Files`:

````markdown
```yaml leetcode
test:
  runtime: local          # 'docker' reserved
packages:
  - name: api
    dir: server
    install: ["pip", "install", "-r", "requirements.txt"]
    start:   ["uvicorn", "main:app", "--host", "127.0.0.1", "--port", "${PORT}"]
    ready:   "Uvicorn running"
    exposeAs:
      VITE_API_URL: "http://127.0.0.1:${PORT}"
  - name: web
    dir: client
    install: ["npm", "ci"]
    start:   ["npm", "run", "dev", "--", "--port", "${PORT}"]
    ready:   "ready in"
    envFile: .env.local     # written role:hidden — the solver never sees it
    dependsOn: [api]
```
````

> **`packages:` is parsed and reached now.** `parseLeetCode` calls
> `parsePackages`, a parsed artifact carries a `packages` field, and an `http`
> check resolves its `package:` name against that list before booting (§9.2).
> `packages-parser.helpers.ts` reads `name`, `dir`, `install`, `start`, `ready`,
> `exposeAs` and `dependsOn`. **`envFile` is not among them** — it appears in the
> worked example above and in the vault's own `fastapi-react.md`, and the parser
> drops it silently, without even the near-miss warning an unknown key would
> earn. Treat it as documented-but-unparsed until the boot task reads it.
>
> **Boot ordering, `install` and `exposeAs` are wired (VSX-178).** `bootStack`
> installs every package, boots them in `dependsOn` order — backends, then
> frontend builds, then frontends — computes each package's `exposeAs` values
> from the **already-assigned** ports of its dependencies, and tears the whole
> set down in one `finally`, registering every group so a session-level cleanup
> finds them. It has its own **verify rules** as of VSX-181 (`stack.rules.ts`), so a malformed
> stack now fails by name rather than borrowing a `package`'s message. It **is** graded end to
> end: the vault's `stack` artifacts verify green with their overlays and red without, each red
> a real response served by a booted server.
> It remains skipped by the default vault sweep (§6.1) and graded only under
> `LEET_STACK_E2E=1` — not because anything is unimplemented, but because installing several
> ecosystems and booting several servers turns a seconds-long offline gate into a minutes-long
> networked one, and a gate nobody runs protects nothing.
>
> **Libraries reach a booted process** (VSX-227). Whatever `libs:` resolved for this run is
> composed into every package's environment for **both** its `install` and its `start` —
> `VIRTUAL_ENV` and a `<venv>/bin` `PATH` prefix for pip, `NODE_PATH` for pnpm, `CLASSPATH`
> for maven, `CARGO_TARGET_DIR` for cargo (§9.4's table) — so a FastAPI or Spring Boot package
> boots with its dependencies. The run's own `node_modules/.bin` is prepended to `PATH` too,
> so a framework CLI named in `start:` (`vite`, `next`) resolves the version this exercise
> declared rather than whatever the machine happens to have.
>
> **An artifact cannot name any of those variables in `exposeAs`.** The reserved set is
> *derived* from the same table that composes them, so it cannot drift out of step, and the
> composition assigns the library values first, the artifact's `exposeAs` entries second
> (allowlist-filtered), and `PORT` last. See §9.2.
>
> The `services:` → `packages:` rename landed with the vault write rather than
> with the parser so that the vault is written exactly once. A check's binding
> field was renamed with it, one wave later: `service: api` became
> `package: api`, because a check field the grammar does not know is dropped —
> which would have read as an artifact declaring fewer checks than it does.

- `install` / `start` are **argv arrays**, never command strings.
- `${PORT}` is the **only** substitution, and it is templated into **every** argv element —
  the command at index 0 included, so `start: ["./boot-${PORT}.sh", …]` resolves to a real
  executable name rather than spawning a file literally called `boot-${PORT}.sh` (VSX-178;
  before it, only the elements *after* the command were substituted).
  **`PORT` is also set in the child's environment**, so `process.env.PORT` and the
  substituted argv agree and either channel is correct. The child is otherwise spawned with
  an environment composed by **allowlist**, not inherited wholesale: the harness spreads its
  own environment, injects `PORT` as its own literal, and then assigns only those `exposeAs`
  names that pass validation. `PORT` itself is therefore a **reserved** `exposeAs` name — an
  artifact naming it is refused at parse time, because an artifact-supplied value would
  otherwise overwrite the port the harness just assigned and the server would bind somewhere
  the cases never reach. The port is one the OS assigned (bind `:0`, read it back), never a
  guess.
- `exposeAs` maps *variable name → value template*; the author names what their framework
  wants (`VITE_*`, `NEXT_PUBLIC_*`) — **the extension encodes no framework knowledge**.
  Both halves are constrained, and narrowly: the **name** must match `^[A-Z][A-Z0-9_]*$` and
  miss a denylist of loader/interpreter variables (`LD_PRELOAD`, `DYLD_*`, `NODE_OPTIONS`,
  `JDK_JAVA_OPTIONS`, …) **plus every variable this extension itself sets as the library seam**
  (`NODE_PATH`, `VIRTUAL_ENV`, `CLASSPATH`, `CARGO_TARGET_DIR` — read from `libEnvVars`, not
  re-listed); the **value** must be exactly a loopback URL template,
  `http://127.0.0.1:${PORT}` with an optional path. So `API_PORT: "${PORT}"` and any
  `localhost` spelling are refused. The name rule alone would be a denylist over an open
  namespace; the value rule is what makes a future gap in it inert.
- `dependsOn` orders the boot; a cycle is a parse error, as is a duplicate `name` or more
  than eight entries.

**Trust class, stated plainly:** a `packages:`-declaring artifact executes declared commands and
package scripts from the `.md` — arbitrary code by design, the same trust class as running
the solver's own candidate locally. The argv rules and the per-registry spec grammars bound the
*shape* of what runs; they do not make artifact-authored code safe.

### 9.4 `libs:` — third-party libraries, per registry

A body config fence (§2.5), canonically placed before `# Setup` (a `function`
artifact) or before `## Files` (a `package` / `stack`) — next to the code it
installs for:

````markdown
```yaml leetcode
libs:
  python: ["fastapi>=0.115,<0.116", uvicorn>=0.32]
  typescript: [react@^19.0.0, vite@^7.0.0]
```
````

Both the block form above and the inline form parse. **`libs:` is read for every
`test.type`** — a `function` exercise can want numpy exactly as a `project` can.

**Quote any spec containing a comma.** A comma separates entries in the inline form, so
`python: [numpy>=2,<3]` declares *two* entries — `numpy>=2` and a `<3` that is refused —
which silently installs an unbounded numpy. Write `["numpy>=2,<3"]`, or use the block form,
where no quoting is needed:

````markdown
```yaml leetcode
libs:
  python:
    - numpy>=2,<3
```
````

#### The language key picks the registry

`LANGUAGES[lang].ecosystem` maps each runnable language to the toolchain that resolves its
libraries — the ids are **tool** names (`pnpm`, `pip`, `cargo`, `maven`), because that is what
gets invoked; the npm *registry* is still where `pnpm`'s packages come from. `ecosystemFor`
`ecosystemFor` resolves the key the same way a fence info-string is resolved (`py` → python,
`tsx` → typescript). A key naming no runnable language is **dropped with a warning**.

| Language(s) | Registry | Installed by |
|---|---|---|
| `javascript`, `typescript` (and their `*react` display ids) | `pnpm` | `pnpm add --dir` — packages from registry.npmjs.org; `npm` itself is never invoked |
| `python` | PyPI | a **venv** in the cache, then `<venv>/bin/python3 -m pip install` |
| `rust` | crates.io | a generated `Cargo.toml`, then `cargo fetch` + a pre-warm build |
| `java` | Maven Central | a generated `pom.xml`, then `mvn dependency:copy-dependencies` |

A missing toolchain is a named message, not a stack: *"mvn not found — install Maven to run
library-backed Java exercises"*.

#### Specs are ecosystem-native, and bounded

An author writes what that registry expects. Each grammar is a **subset** of what the CLI
accepts, parsed into fields — never waved through by one pattern:

| Registry | Write | Fields |
|---|---|---|
| `pnpm` | `react@^19.0.0`, `@types/node@^20` | name (scope included), range |
| pip | `numpy`, `"numpy>=2,<3"`, `requests[socks]==2.32.3` | name, extras, predicates |
| cargo | `serde_json@1.0`, `serde@^1+derive+std` | name, req, features |
| maven | `com.google.guava:guava:33.3.1-jre` (`:packaging:classifier` optional) | the coordinate segments |

The maven version is the **published** one, qualifier included: guava ships `33.3.1-jre` and
`33.3.1-android`, never a bare `33.3.1`. The grammar cannot know that — every segment is a
plain identifier to it — so a version that does not exist passes validation and fails at
resolve time, where the installer now reports Maven's own `was not found` line.

An entry its registry's grammar refuses is **dropped with a warning** before it can reach an
install subprocess; the rest of that language's list still installs.

**Refused everywhere**, because each fetches from or reads a location the artifact chose, and
a name-shape grammar can say nothing about a URL:

- `pnpm` `file:` · `link:` · `git+…` · `workspace:` protocol specs
- pip direct references (`name @ url`), VCS URLs, environment markers (`;`), `-r file` forms
- cargo inline TOML — `git =`, `path =`, `registry =`, `default-features = false`
- maven repository or mirror overrides, and the non-reproducible `LATEST` / `RELEASE`
- in all four: `..` anywhere, a leading `-` (flag injection), whitespace, shell metacharacters

Cargo's `+` is this grammar's feature separator, so semver build metadata (`1.0.0+build`) is
not supported — one character cannot mean both.

#### How a run consumes them

Installed **once** into a shared cache under `os.tmpdir()`, keyed on
`sha256(ecosystem + sorted specs)` — the ecosystem is part of the key, so a pip `react` and an
npm `react` can never be served the same directory. Two artifacts declaring the same set share
one install; a different version is a different key.

The cache reaches a run through **environment variables only**. Every `compile` / `run`
command stays a fixed literal, so no cache path is ever interpolated into a command line:

| Language | Variable(s) | Command change |
|---|---|---|
| python | `PATH` prefix `<dir>/bin`, `VIRTUAL_ENV` | none — `python3 runner.py` resolves the venv interpreter |
| javascript / typescript | `NODE_PATH=<dir>/node_modules` | none |
| java | `CLASSPATH=<dir>/jars/*` + `:.` | the run drops `-cp .`, which would override `CLASSPATH` |
| rust | `CARGO_TARGET_DIR=<dir>/target` | **yes** — a Cargo project replaces the bare `rustc` path |

`NODE_PATH` serves CJS `require` only, which is what the `function` envs' `vm` sandbox uses;
a bare ESM `import` needs the `project` type, whose run directory gets a real `node_modules`
of symlinks instead.

A `project` resolves **one environment per registry** in a single grading run, so a
FastAPI-plus-React exercise gets a venv *and* a node environment. A `build` check sees its
declared toolchains first on `PATH` — `<venv>/bin`, then the run's own `node_modules/.bin`,
then the inherited `PATH` — so `argv: ["pytest", "-q"]` resolves what the exercise declared.

#### Trust class

Installing `libs:` fetches, and may execute, third-party code: a PyPI `setup.py`, a cargo
`build.rs`, a maven plugin. That is **arbitrary code**, the same trust class as running a
solver's candidate locally. The grammars bound the *shape of a name*; they say nothing about
what the package does once fetched. Treat an artifact's `libs:` the way you would treat its
`build` argv — as code you are choosing to run.

**Node is the exception, in the safe direction.** pnpm 10+ refuses a dependency's
`preinstall`/`install`/`postinstall` outright, and **no allowlist ships**, so an artifact
cannot obtain code execution merely by declaring an npm package. This is stricter than the
npm installer it replaced, which ran them all. Two consequences worth knowing:

- `--config.strict-dep-builds=false` **is** passed, because pnpm 11 makes an ignored build
  script a non-zero exit; without it a perfectly usable install is reported as `install failed`.
  The flag changes the *exit status*, never whether a script runs.
- A package that genuinely needs a build step installs quietly incomplete. esbuild — the
  harness's own toolchain, and the obvious candidate — is unaffected, because its platform
  binary arrives as an optional dependency rather than a postinstall download. If some future
  lib does need one, the fix is a hardcoded `--allow-build=<pkg>` list in the installer,
  **never** one read from an artifact.

---

## 10. Wire shape *(engine; W3, 2026-11-04)*

The JSON the engine sends for a parsed exercise. Rust owner: `mda-core/src/leetcode/model/` (`Serialize` only, D-41);
`LeetCodeExercise::default()` is the one authority for every undeclared value (D-40). Field names are camelCase as TS's
`ParsedLeetCode` (`src/types/leetcode.types.ts`), with three deviations the extension's exporter normalises (D-24):

- **`warnings` is always an array** of `{code, params}` (TS: `string[]`, `undefined` when clean, `[]` for projects).
- **`unimplementedKinds`** (string list) is added: the check kinds an environment does not implement yet (fed by
  `leetcode.checks.reserved_kind`, deduplicated, first-seen order).
- **Optionals are absent** when undeclared, as TS; a required field is always present.

Maps (`TestCase.input`, object case values, `functions`, `libs`, `exposeAs`) are JSON objects **in declared order**
(clients print them as written). Case values are JSON: `null`, booleans, integers (`5`, never `5.0`), floats, strings,
lists, objects; NaN and infinities serialize as `null` (as `JSON.stringify`). A duplicate key in the source keeps its first
position with the last value (as a JS object); the parser (W4) builds maps that way, so a key is never emitted twice.

### 10.1 `LeetCodeExercise`

Required = always on the wire. Undeclared value = `LeetCodeExercise::default()` (single authority; W4 starts from it).

| JSON name | Type | Req | Undeclared value | Source of default |
|---|---|---|---|---|
| `title` | string | yes | `""` | spec:185, TS parser.service:91 |
| `leetcodeType` | `function`\|`package`\|`stack` | yes | `"function"` (derived) | spec:225, §2.3 |
| `difficulty` | `easy`\|`medium`\|`hard` | yes | `"easy"` | spec:186 |
| `functionName` | string | yes | `""` | spec:457, TS parser.service:94 |
| `functions` | map lang -> name | no | absent | spec:458 |
| `algorithm` | string | no | absent | spec:188 |
| `status` | `unsolved`\|`attempted`\|`solved` | yes | `"unsolved"` | spec:187 |
| `params` | `ParamDef[]` | yes | `[]` | spec:459 |
| `returns` | string | yes | `""` | spec:460, TS parser.service:99 |
| `description` | string | yes | `""` | TS parser.service:100 |
| `examples` | `{input, output}[]` (strings) | yes | `[]` | spec §3.2 |
| `tests` | `TestCase[]` | yes | `[]` | spec §3.3 |
| `finalTests` | `TestCase[]` | yes | `[]` (raw, no public fallback) | spec §3.3 |
| `test` | `TestConfig` | yes | `{type:"call", timeoutMs:5000}` | spec:354-355, TS constants:234,274 |
| `setups` | `ExerciseSetup[]` | yes | `[]` | spec §3.4 |
| `practice` | `PracticeConfig` | yes | `{options:["noCompletion","noAiAgents"], timeLimitMinutes:0, locked:false}` | spec:421-423 |
| `solutions` | `LeetCodeSolution[]` | yes | `[]` | spec §3.5 |
| `attempts` | `Attempt[]` | yes | `[]` | spec §3.6 |
| `tags` | string[] | yes | `[]` | spec:189 |
| `files` | `FileSpec[]` | no | absent | spec §9.1 |
| `libs` | map lang -> string[] | no | absent | spec §9.4 |
| `solutionFiles` | `FileSpec[]` | no | absent | spec §9.1b |
| `checks` | `ProjectCheck[]` | no | absent | spec §9.2 |
| `packages` | `PackageSpec[]` | no | absent (TS: empty list => absent; W4 must keep that) | spec §9.3 |
| `program` | `ProgramConfig` | no | absent | spec §2.5.5 |
| `warnings` | `Warning[]` (`{code, params}`) | yes | `[]` (TS: `string[]`, undefined when clean; now always an array, D-24) | D-24 |
| `unimplementedKinds` | string[] | yes | `[]` | TS project-parser.helpers `unimplementedKinds` (readonly string[]) |

### 10.2 Nested types

| Type | JSON fields (optional marked ?) | Enum value sets |
|---|---|---|
| `ParamDef` | `name`, `type` | - |
| `Example` | `input`, `output` | - |
| `TestConfig` | `type`, `timeoutMs` (no `rawType`) | `TestTypeId`: `call` `program` `http` `build` `dom-assert` `css-assert` `class` `in-place` |
| `PracticeConfig` | `options`, `timeLimitMinutes` (fence key `timeLimit`), `locked` | `PracticeOptionId`: `noCompletion` `noAiAgents` `noSnippets` `noParameterHints` |
| `ProgramConfig` | `channel`, `entry`?, `flags`? | `channel`: `argv` `flags` `stdin` |
| `TestCase` | `input` (name -> value), `expected` | values are JSON (`CaseValue`: null, bool, int, float, string, list, object) |
| `ExerciseSetup` | `language`, `code` | - |
| `LeetCodeSolution` | `language`, `label`?, `code`, `solvedAt`?, `duration`? | - |
| `Attempt` | `language`, `at`, `duration`, `passed`, `bigO`?, `confidence`?, `code` | `confidence` free string (TS: high/medium/low, not enforced) |
| `AttemptEntry` | `at`, `duration`, `passed`, `bigO`?, `confidence`?, `code` | - |
| `LeetCodeSummary` | `title`, `leetcodeType`?, `difficulty`, `status`, `algorithm`?, `tags` | - |
| `FileSpec` | `path`, `language`, `role`, `content` | `role`: `editable` `readonly` `hidden` |
| `LibSpec` | map lang -> string[] | - |
| `PackageSpec` | `name`, `dir`, `install`, `start`, `ready`?, `exposeAs`? (map), `dependsOn`? | - |
| `RenderStep` | tag `op`: `click{selector}` `change{selector,value}` `text{selector}` `count{selector}` `attr{selector,name}` `style{selector,property}` | - |

### 10.3 `ProjectCheck`

Internally tagged on **`kind`** (as TS); common fields `name`, `cases`, `publicCount` sit beside the tag.

| `kind` | extra fields |
|---|---|
| `call` | `file`, `function` |
| `build` | `argv`, `dir`? |
| `dom-assert` | `file`, `package`? |
| `css-assert` | `file`, `package`? |
| `http` | `package` |

### 10.4 Warning codes

One code per TS warning site (37 sites, 36 codes: two sites share one message). Params are strings; a list is `", "`-joined
(D-36); no `line` param. TS prose `reason` values become tokens: `empty`, `absolute`, `parent_segment`, `reserved_segment`
(`program.entry_rejected`, `packages.dir_rejected`). The guarded list is `mda_core::error::LEETCODE_WARNING_CODES`;
PROTOCOL §6 lists them. TS files: PP = `services/project-parser.helpers.ts`, PK = `packages-parser.helpers.ts`, CB =
`leetcode-config-blocks.helpers.ts`, PG = `program-config.helpers.ts`, LP = `leetcode-parser.service.ts`, LT =
`leetcode-type.helpers.ts` (extension at `84c437d`). English is the extension's message; `{p}` names a param.

| # | TS site | Code | Params | English (`{p}` = param) |
|---|---|---|---|---|
| W01 | LP:184 | `leetcode.frontmatter.ignored_key` | `key` | frontmatter '{key}:' is ignored - move it into a 'yaml leetcode' body fence |
| W02 | LT:103 | `leetcode.type.unknown` | `declared`, `derived` | leetcodeType: unknown value '{declared}' - derived '{derived}' from test.type instead |
| W03 | CB:233 | `leetcode.config.indented` | `line` (the first content line) | config fence: line not at column 0 (kept verbatim) - '{line}' |
| W04 | CB:269 | `leetcode.config.key_in_frontmatter` | `key` | config fence: '{key}:' belongs in frontmatter - the fence wins here, but the picker and the status writer read frontmatter, so the two will disagree |
| W05 | CB:283 | `leetcode.config.duplicate_key` | `key`, `count`, `winner` (`first`/`last` as TS names it) | config: '{key}' is declared in {count} fences - the {winner} occurrence wins |
| W06 | CB:325 | `leetcode.config.unterminated_fence` | - | config fence: unterminated ```yaml leetcode - no config read from it |
| W07 | CB:347 | `leetcode.config.hidden_by_open_fence` | - | config: an unterminated ``` fence earlier in the document may hide config fences after it |
| W08 | PG:98 | `leetcode.program.no_channel` | - | program: no 'channel' declared - defaulted to 'argv' |
| W09 | PG:109 | `leetcode.program.unknown_channel` | `channel` | program: unknown channel '{channel}' - defaulted to 'argv' |
| W10 | PG:117 | `leetcode.program.entry_rejected` | `entry`, `reason` | program: entry '{entry}' rejected - {reason} |
| W11 | PP:306 | `leetcode.files.unknown_role` | `role` | ## Files: unknown role '{role}' - treated as editable |
| W12 | PP:348 | `leetcode.libs.no_registry` | `language` | libs: '{language}' has no package registry here - these are skipped |
| W13 | PP:355 | `leetcode.libs.rejected` | `specs` (`", "`-joined, each unquoted), `ecosystem` | libs: rejected {specs} - not an installable {ecosystem} spec |
| W14 | PP:466 | `leetcode.checks.not_a_list` | `got` | checks: expected a list, got '{got}' - no checks parsed |
| W15 | PP:502 | `leetcode.checks.reserved_kind` | `name`, `kind` | checks: '{name}' declares kind '{kind}', which no environment implements yet - dropped |
| W16 | PP:507 and PP:552 | `leetcode.checks.unknown_kind` | `name`, `kind` | checks: '{name}' has unknown kind '{kind}' - dropped |
| W17 | PP:517 | `leetcode.checks.http_needs_package` | `name` | checks: http check '{name}' needs a package - dropped |
| W18 | PP:526 | `leetcode.checks.build_needs_argv` | `name` | checks: build check '{name}' needs argv as a JSON array of strings - dropped |
| W19 | PP:535 | `leetcode.checks.needs_file` | `name` | checks: '{name}' needs a file - dropped |
| W20 | PP:541 | `leetcode.checks.call_needs_function` | `name` | checks: call check '{name}' needs a function - dropped |
| W21 | PP:595 | `leetcode.checks.fence_unbound` | - | ## Tests: a json fence names no check, and this project has several - cases ignored |
| W22 | PP:600 | `leetcode.checks.fence_unknown_check` | `name` | ## Tests: fence binds to unknown check '{name}' - cases ignored |
| W23 | PP:635 | `leetcode.config.unknown_key` | `key`, `suggestion` | unknown config key '{key}' - did you mean '{suggestion}'? |
| W24 | PK:174 | `leetcode.packages.not_a_list` | `got` | packages: expected a list, got '{got}' - no packages parsed |
| W25 | PK:216 | `leetcode.packages.missing_name` | - | packages: entry missing 'name' - dropped |
| W26 | PK:220 | `leetcode.packages.dir_rejected` | `name`, `dir`, `reason` | packages: '{name}' dir '{dir}' rejected - {reason} |
| W27 | PK:243 | `leetcode.packages.missing_field` | `name`, `field` (`install`/`start`) | packages: '{name}' needs {field} - dropped |
| W28 | PK:248 | `leetcode.packages.argv_not_array` | `name`, `field` | packages: '{name}' {field} must be a JSON array of strings, not a command string - dropped |
| W29 | PK:252 | `leetcode.packages.argv_empty` | `name`, `field` | packages: '{name}' {field} must not be an empty argv - dropped |
| W30 | PK:257 | `leetcode.packages.argv_substitution` | `name`, `field`, `element` | packages: '{name}' {field} element '{element}' uses a substitution other than ${PORT} - dropped |
| W31 | PK:303 | `leetcode.packages.expose_name_invalid` | `name` | packages: exposeAs name '{name}' must match ^[A-Z][A-Z0-9_]*$ - dropped |
| W32 | PK:307 | `leetcode.packages.expose_name_reserved` | `name` | packages: exposeAs '{name}' is a reserved loader/interpreter variable name - dropped |
| W33 | PK:311 | `leetcode.packages.expose_value_not_loopback` | `name`, `value` | packages: exposeAs '{name}' value '{value}' must be a loopback URL template - dropped |
| W34 | PK:187 (via PP:206) | `leetcode.packages.too_many` | `count`, `max` (8, `MAX_PACKAGES`) | packages: {count} entries declared, more than the {max}-entry cap - whole block refused |
| W35 | PK:199 (via PP:206) | `leetcode.packages.duplicate_name` | `name` | packages: duplicate name '{name}' - every package name must be unique |
| W36 | PK:204 (via PP:206) | `leetcode.packages.cycle` | `path` (`", "`-joined package names; TS joins with ' -> ') | packages: dependsOn cycle detected: {path} |

**Parser rules carried by this table** (W4 reproduces them, never "fixes" them):

- W34-W36 are the "hard parse errors" of `parsePackages`: the whole `packages:` block is dropped (TS returns `ok:false`, the project parser pushes the message and carries on with `packages` empty). They are warnings on the wire, never an error (the parser never fails, D-24).
- `reason` tokens for W10 and W26 (TS prose -> token; `unsafeEntryReason` PG:129-142, `unsafeDirReason` PK:281): `empty`, `absolute` ("must be relative to the run directory"), `parent_segment` ("must not contain a '..' segment"), `reserved_segment` ("'node_modules' is a reserved path segment"). Four values, shared.
- W15 also feeds `unimplementedKinds` (kind pushed via `onReservedKind`, deduplicated, insertion order).
- W23 near-miss algorithm (PP:627-637): for each frontmatter-or-fence line matching `^(\w+):` (column 0, merged config text), skip if the key is in `KNOWN_KEYS`; else take the **first** known key, in array order, whose Levenshtein distance (iterative single-row, unit cost for insert/delete/substitute) is <= 2 (not the closest). Array order = `artifactType, leetcodeType, title, difficulty, status, algorithm, tags` (`CANONICAL_FRONTMATTER_ORDER`, via the `RETAINED_FM_KEYS` set) then `function, functions, params, returns, test, practice, libs, checks, packages, program` (`BODY_SET_KEYS`). No match => silent (custom keys stay silent). Short keys (<= 2 chars) match almost anything; W4 must reproduce that, not fix it.
- W01: the key set comes from `legacyFrontmatterKeys(fmRaw)` (body-set keys found in frontmatter, D4 hard cut): `function functions params returns test practice libs checks packages program`.
- W05 fires only for keys in `DUPLICATE_KEY_WINNER` (CB:60): `last` for function, functions, params, returns, test, practice, tags; `first` for libs, checks, packages. `count` >= 2 fences.
- TS gives no warning for: unknown `difficulty`/`status` values (silent default), unknown `test.type`, out-of-range `timeoutMs`, unknown practice option ids (all silent in TS; D-24 forbids adding any).
