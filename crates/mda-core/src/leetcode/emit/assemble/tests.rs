use serde_json::{Value, json};

use super::super::assemble;
use crate::leetcode::{Block, BlockKind as K, LeetCodeExercise, layout, parse, write_guard};

/// Every block kind of a function exercise: block-list tags, a comment in the frontmatter and in a
/// fence, two config fences, shared and labelled language headings, solved meta, attempts.
const RICH: &str = "---
artifactType: LeetCode
leetcodeType: function
title: Two Sum
difficulty: easy
status: attempted
# a note
algorithm: hash-map
tags:
  - leetcode
  - arrays
---

Given an array of numbers.

```yaml leetcode
# the signature
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
input: nums = [2,7], target = 9
output: [0,1]
```

```example
input: nums = [3,3], target = 6
output: [0,1]
```

```yaml leetcode
test:
  timeoutMs: 8000
practice:
  locked: true
```

## Tests

```json
[{\"input\":{\"nums\":[2,7],\"target\":9},\"expected\":[0,1]}]
```

```yaml
- input:
    nums: [3, 3]
    target: 6
  expected: [0, 1]
```

## Final Tests

```json
[{\"input\":{\"nums\":[1,5],\"target\":6},\"expected\":[0,1]}]
```

# Setup

## python

```python
def two_sum(nums, target): pass
```

## javascript

```javascript
function twoSum(nums, target) {}
```

# Solutions

## python

### Hash Map

<!-- meta: { \"solved_at\": \"2026-01-01\", \"duration\": \"5m\" } -->
```python
def two_sum(nums, target): return [0, 1]
```

## java
```java
class S {}
```

# Attempts

## python
<!-- attempt: {\"at\":\"2026-01-02\",\"duration\":\"1m\",\"passed\":false} -->
```python
x
```
";

pub(super) fn rich() -> (LeetCodeExercise, Vec<Block>) {
    let x = parse(RICH);
    assert!(x.warnings.is_empty(), "{:?}", x.warnings);
    assert_eq!(
        (
            x.examples.len(),
            x.tests.len(),
            x.final_tests.len(),
            x.setups.len()
        ),
        (2, 2, 1, 2)
    );
    assert_eq!(
        (x.solutions.len(), x.attempts.len(), x.tags.len()),
        (2, 1, 2)
    );
    (x, layout(RICH).unwrap())
}

fn edit(x: &LeetCodeExercise, f: impl FnOnce(&mut Value)) -> LeetCodeExercise {
    let mut v = serde_json::to_value(x).unwrap();
    f(&mut v);
    serde_json::from_value(v).unwrap()
}

fn set(v: &mut Value, ptr: &str, to: Value) {
    *v.pointer_mut(ptr).unwrap_or_else(|| panic!("no {ptr}")) = to;
}

fn key_of(f: crate::leetcode::LayoutField) -> String {
    serde_json::to_value(f)
        .map(|v| match v {
            Value::Object(m) => m.keys().next().cloned().unwrap_or_default(),
            Value::String(s) => s,
            _ => String::new(),
        })
        .unwrap_or_default()
}

/// Top-level field → (JSON pointer, new value): one edit per field of the model.
const MUTATIONS: [(&str, &str, &str); 17] = [
    ("title", "/title", "\"Changed\""),
    ("difficulty", "/difficulty", "\"hard\""),
    ("status", "/status", "\"solved\""),
    ("algorithm", "/algorithm", "\"dp\""),
    ("tags", "/tags/0", "\"newtag\""),
    ("functionName", "/functionName", "\"twoSum2\""),
    ("params", "/params/0/name", "\"numbers\""),
    ("returns", "/returns", "\"int\""),
    ("description", "/description", "\"New description.\""),
    ("examples", "/examples/0/output", "\"[9]\""),
    ("tests", "/tests/0/expected", "[9, 9]"),
    ("finalTests", "/finalTests/0/expected", "[9, 9]"),
    ("test", "/test/timeoutMs", "7000"),
    ("practice", "/practice/locked", "false"),
    ("setups", "/setups/0/code", "\"x = 1\""),
    ("solutions", "/solutions/1/code", "\"class Changed {}\""),
    ("attempts", "/attempts/0/code", "\"changed\""),
];

/// No top-level key of the frontmatter appears twice.
fn assert_unique_fm_keys(out: &str) {
    let fm: Vec<&str> = out.lines().skip(1).take_while(|l| *l != "---").collect();
    let mut seen = std::collections::HashSet::new();
    for l in fm {
        if let Some(k) = crate::leetcode::config::top_level_key(l) {
            assert!(seen.insert(k), "{k} twice:\n{out}");
        }
    }
}

/// `out` holds every unmapped block's raw text, in order.
fn assert_unmapped_survive(key: &str, blocks: &[Block], out: &str) {
    let fm = ["title", "difficulty", "status", "algorithm", "tags"].contains(&key);
    let mut at = 0;
    for b in blocks {
        let mapped = b.fields.iter().any(|f| key_of(*f) == key);
        let whole = fm && matches!(b.kind, K::FmOpen | K::FmKey | K::FmClose);
        let section = key == "attempts" && b.kind == K::Heading;
        if mapped || whole || section {
            continue;
        }
        let found = out.get(at..).and_then(|r| r.find(&b.raw));
        let Some(i) = found else {
            panic!("{key}: block {:?} {:?} lost or moved\n{out}", b.kind, b.raw);
        };
        at += i + b.raw.len();
    }
}

#[test]
fn mutate_one_field_and_only_its_blocks_change() {
    let (x, blocks) = rich();
    assert_eq!(assemble(&x, Some(&blocks)).unwrap(), RICH);
    for (key, ptr, to) in MUTATIONS {
        let y = edit(&x, |v| set(v, ptr, serde_json::from_str(to).unwrap()));
        let out = assemble(&y, Some(&blocks)).unwrap_or_else(|e| panic!("{key}: refused {e:?}"));
        assert_ne!(out, RICH, "{key}: nothing changed");
        assert_eq!(write_guard(&y, &out), Ok(()), "{key}\n{out}");
        assert_unmapped_survive(key, &blocks, &out);
        assert_unique_fm_keys(&out);
    }
}

#[test]
fn every_field_has_a_mutation() {
    let (x, _) = rich();
    let v = serde_json::to_value(&x).unwrap();
    for k in v.as_object().unwrap().keys() {
        let skip = [
            "warnings",
            "unimplementedKinds",
            "leetcodeType",
            "functions",
        ];
        assert!(
            skip.contains(&k.as_str()) || MUTATIONS.iter().any(|m| m.0 == k),
            "no mutation for {k}"
        );
    }
}

#[test]
fn growth_appends_and_shrink_drops_the_tail() {
    let (x, blocks) = rich();
    let grown = edit(&x, |v| {
        let push = |v: &mut Value, p: &str, item: Value| {
            v.pointer_mut(p).unwrap().as_array_mut().unwrap().push(item)
        };
        push(v, "/examples", json!({"input": "n = 1", "output": "1"}));
        push(
            v,
            "/tests",
            json!({"input": {"nums": [1, 1], "target": 2}, "expected": [0, 1]}),
        );
        push(
            v,
            "/setups",
            json!({"language": "go", "code": "package main"}),
        );
        push(
            v,
            "/solutions",
            json!({"language": "go", "code": "package main"}),
        );
    });
    let out = assemble(&grown, Some(&blocks)).unwrap();
    assert_eq!(write_guard(&grown, &out), Ok(()), "{out}");
    assert!(
        out.contains("n = 1") && out.contains("package main"),
        "{out}"
    );
    // shrink: drop the second example, the second test fence, the java solution and the setup of js
    let small = edit(&x, |v| {
        v["examples"].as_array_mut().unwrap().pop();
        v["tests"].as_array_mut().unwrap().pop();
        v["setups"].as_array_mut().unwrap().pop();
        v["solutions"].as_array_mut().unwrap().pop();
    });
    let out = assemble(&small, Some(&blocks)).unwrap();
    assert_eq!(write_guard(&small, &out), Ok(()), "{out}");
    assert!(
        !out.contains("## java") && !out.contains("## javascript"),
        "{out}"
    );
    assert!(out.contains("## python") && out.contains("### Hash Map"));
}

#[test]
fn config_keys_new_and_back_to_default() {
    let (x, blocks) = rich();
    // a key whose group fence exists joins it; one with no fence opens a canonical fence (Q-81)
    let mut y = x.clone();
    y.returns = "int".into();
    y.test.timeout_ms = 1000;
    y.libs = None;
    let out = assemble(&y, Some(&blocks)).unwrap();
    assert_eq!(write_guard(&y, &out), Ok(()), "{out}");
    // test + practice fence loses `practice` when it is back to its default, keeps `test`
    let mut z = x.clone();
    z.practice = Default::default();
    let out = assemble(&z, Some(&blocks)).unwrap();
    assert_eq!(write_guard(&z, &out), Ok(()), "{out}");
    assert!(!out.contains("practice:") && out.contains("timeoutMs: 8000"));
    // every key of that fence back to default: the fence goes
    let mut w = z.clone();
    w.test = Default::default();
    let out = assemble(&w, Some(&blocks)).unwrap();
    assert_eq!(write_guard(&w, &out), Ok(()), "{out}");
    assert_eq!(out.matches("```yaml leetcode").count(), 1, "{out}");
    // a key with no fence anywhere: new canonical fence
    let bare = "---\ntitle: T\n---\n\nProse.\n\n## Examples\n";
    let mut v = parse(bare);
    v.function_name = "f".into();
    let out = assemble(&v, Some(&layout(bare).unwrap())).unwrap();
    assert_eq!(write_guard(&v, &out), Ok(()), "{out}");
    assert!(out.contains("```yaml leetcode\nfunction: f\n```"));
}

#[test]
fn attempts_are_rebuilt_whole() {
    let (x, blocks) = rich();
    let more = edit(&x, |v| {
        let a = json!({"language": "python", "code": "y", "at": "2026-02-02", "duration": "2m", "passed": true});
        v["attempts"].as_array_mut().unwrap().insert(0, a);
    });
    let out = assemble(&more, Some(&blocks)).unwrap();
    assert_eq!(write_guard(&more, &out), Ok(()), "{out}");
    let none = edit(&x, |v| v["attempts"] = json!([]));
    let out = assemble(&none, Some(&blocks)).unwrap();
    assert_eq!(write_guard(&none, &out), Ok(()), "{out}");
    assert!(!out.contains("# Attempts"));
}

const PKG: &str = "---
artifactType: LeetCode
leetcodeType: package
title: Pkg
difficulty: easy
status: unsolved
tags: [leetcode]
---

Build it.

```yaml leetcode
libs:
  javascript: [lodash]
```

```yaml leetcode
packages:
  - name: api
    dir: server
    install: [\"pip\", \"install\"]
    start: [\"uvicorn\", \"main:app\"]
```

```yaml leetcode
program:
  channel: argv
  entry: main.py
```

```yaml leetcode
test:
  timeoutMs: 10000
  checks:
    - name: cat filter
      kind: call
      file: src/lib/cat.ts
      function: f
```

## Files

```typescript path=src/lib/cat.ts role=editable
export function f() {}
```

```css path=src/app.css role=readonly
body {}
```

## Tests

```json check=\"cat filter\"
[{\"input\": {\"n\": 1}, \"expected\": [1]}]
```

# Solutions

```typescript path=src/lib/cat.ts
export function f() { return [1]; }
```
";

#[test]
fn package_exercise_mapping() {
    let x = parse(PKG);
    assert!(x.warnings.is_empty(), "{:?}", x.warnings);
    let blocks = layout(PKG).unwrap();
    assert_eq!(assemble(&x, Some(&blocks)).unwrap(), PKG);
    let muts: [(&str, &str, &str); 7] = [
        ("files", "/files/1/content", "\"a { }\\n\""),
        (
            "solutionFiles",
            "/solutionFiles/0/content",
            "\"export const f = 1;\\n\"",
        ),
        ("libs", "/libs/javascript/0", "\"left-pad\""),
        ("packages", "/packages/0/dir", "\"srv\""),
        ("program", "/program/entry", "\"app.py\""),
        ("tests", "/tests/0/expected", "[7]"),
        ("test", "/test/timeoutMs", "20000"),
    ];
    for (key, ptr, to) in muts {
        let mut v = serde_json::to_value(&x).unwrap();
        let Some(slot) = v.pointer_mut(ptr) else {
            panic!("{key}: no {ptr} in {v}");
        };
        *slot = serde_json::from_str(to).unwrap();
        if key == "tests" {
            // the bound check carries the same case (its cases follow the fence)
            set(&mut v, "/checks/0/cases/0/expected", json!([7]));
        }
        let y: LeetCodeExercise = serde_json::from_value(v).unwrap();
        let out = assemble(&y, Some(&blocks)).unwrap_or_else(|e| panic!("{key}: refused {e:?}"));
        assert_ne!(out, PKG, "{key}");
        assert_eq!(write_guard(&y, &out), Ok(()), "{key}\n{out}");
        assert_unmapped_survive(key, &blocks, &out);
    }
}

#[test]
fn custom_frontmatter_keys_survive_a_frontmatter_change() {
    let src = "---\nartifactType: LeetCode\ntitle: T\naliases: [x]\nstatus: unsolved\ncreated: 2026-01-01\nsource:\n  - a\n---\n\nBody.\n";
    let x = parse(src);
    let mut y = x.clone();
    y.status = crate::leetcode::LeetCodeStatus::Solved;
    let out = assemble(&y, Some(&layout(src).unwrap())).unwrap();
    assert_eq!(write_guard(&y, &out), Ok(()));
    assert!(out.contains("status: solved\n"), "{out}");
    for k in [
        "aliases: [x]\n",
        "created: 2026-01-01\n",
        "source:\n  - a\n",
    ] {
        assert!(out.contains(k), "lost {k:?}:\n{out}");
    }
    assert!(out.contains("---\n\nBody.\n"), "{out}");
}

#[test]
fn a_status_change_keeps_one_line_per_canonical_key() {
    let src = "---\nartifactType: LeetCode\nleetcodeType: function\ntitle: T\ndifficulty: easy\nstatus: attempted\ntags:\n  - a\n  - b\naliases: [x]\n---\n\nBody.\n";
    let x = parse(src);
    let mut y = x.clone();
    y.status = crate::leetcode::LeetCodeStatus::Solved;
    let out = assemble(&y, Some(&layout(src).unwrap())).unwrap();
    assert_eq!(write_guard(&y, &out), Ok(()));
    assert_unique_fm_keys(&out);
    for k in [
        "artifactType",
        "leetcodeType",
        "title",
        "difficulty",
        "status",
        "tags",
    ] {
        assert_eq!(
            out.lines()
                .filter(|l| l.starts_with(&format!("{k}:")))
                .count(),
            1,
            "{k}\n{out}"
        );
    }
    assert!(out.contains("aliases: [x]\n"));
}
