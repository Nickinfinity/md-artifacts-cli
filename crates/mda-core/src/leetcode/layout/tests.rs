use super::*;
use crate::leetcode::{BlockKind as K, LayoutField as F, MAX_LAYOUT_BLOCKS};

fn concat(blocks: &[Block]) -> String {
    blocks.iter().map(|b| b.raw.as_str()).collect()
}

fn ok(src: &str) -> Vec<Block> {
    let blocks = layout(src).unwrap();
    assert_eq!(concat(&blocks), src, "concat(raw) must equal the input");
    blocks
}

fn fields(blocks: &[Block], kind: K) -> Vec<Vec<F>> {
    blocks
        .iter()
        .filter(|b| b.kind == kind)
        .map(|b| b.fields.clone())
        .collect()
}

fn md_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            md_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "md") {
            out.push(p);
        }
    }
}

#[test]
fn concat_equals_input_for_every_conformance_and_hostile_input() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/leetcode");
    let mut files = vec![];
    md_files(&root, &mut files);
    assert!(files.len() > 50, "found only {} inputs", files.len());
    for f in files {
        let src = std::fs::read_to_string(&f).unwrap_or_else(|e| panic!("{}: {e}", f.display()));
        match layout(&src) {
            Ok(blocks) => assert_eq!(concat(&blocks), src, "{}", f.display()),
            Err(e) => panic!("{} refused: {e:?}", f.display()),
        }
    }
}

#[test]
fn odd_inputs_cover_every_byte() {
    for s in [
        "",
        "---",
        "---\n",
        "---x\nrest\n",
        "---\r\na: b\r\n---\r\n",
        "```",
        "\r\n```\r\n",
        "é```\n",
        "```yaml leetcode\nreturns: int\n",
        "```yaml leetcode\n```",
        "<!-- x",
        "# Solutions\n<!-- x\n",
        "---\n---\n",
        "\u{feff}---\na: b\n---\n",
        "a\u{2028}b\n```\nx",
    ] {
        ok(s);
    }
}

const FULL: &str = "---
artifactType: LeetCode
title: Two Sum
difficulty: Easy
status: solved
algorithm: hash
leetcodeType: function
tags:
  - a
  - b
---

Describe it.

```yaml leetcode
function: twoSum
# a comment
params:
  nums: int[]
returns: int
test:
  mode: x
libs:
  python: [numpy]
```

## Examples

```example
input: x
output: y
```

## Tests

```json
[{\"input\":{\"a\":1},\"expected\":1},{\"input\":{\"a\":2},\"expected\":2}]
```

## Final Tests

```json
[{\"input\":{\"a\":3},\"expected\":3}]
```

# Setup

## Python

```python
def f(): pass
```

# Solutions

## Python

<!-- meta: {\"runtime\":1} -->
```python
x
```

# Attempts

## Python

<!-- attempt: {\"at\":\"t\",\"duration\":\"1s\",\"passed\":true} -->
```python
y
```
";

#[test]
fn frontmatter_blocks_are_key_plus_continuation() {
    let b = ok(FULL);
    assert_eq!(b[0].kind, K::FmOpen);
    assert_eq!(
        fields(&b, K::FmKey),
        vec![
            vec![],
            vec![F::Title],
            vec![F::Difficulty],
            vec![F::Status],
            vec![F::Algorithm],
            vec![F::LeetcodeType],
            vec![F::Tags]
        ]
    );
    let tags = b.iter().find(|b| b.fields == [F::Tags]).unwrap();
    assert_eq!(tags.raw, "tags:\n  - a\n  - b\n");
    assert_eq!(fields(&b, K::FmClose).len(), 1);
}

#[test]
fn config_fence_is_open_keys_comment_close() {
    let b = ok(FULL);
    assert_eq!(fields(&b, K::ConfigOpen), vec![Vec::<F>::new()]);
    assert_eq!(
        fields(&b, K::ConfigKey),
        vec![
            vec![F::FunctionName],
            vec![F::Params],
            vec![F::Returns],
            vec![F::Test, F::Checks],
            vec![F::Libs]
        ]
    );
    assert_eq!(fields(&b, K::ConfigComment), vec![Vec::<F>::new()]);
    assert_eq!(fields(&b, K::ConfigClose).len(), 1);
    let params = b.iter().find(|b| b.fields == [F::Params]).unwrap();
    assert_eq!(params.raw, "params:\n  nums: int[]\n");
}

#[test]
fn body_blocks_carry_their_fields() {
    let b = ok(FULL);
    assert_eq!(
        fields(&b, K::CaseFence),
        vec![
            vec![F::Examples(0)],
            vec![F::Tests(0), F::Tests(1)],
            vec![F::FinalTests(0)]
        ]
    );
    assert_eq!(
        fields(&b, K::CodeFence),
        vec![
            vec![F::Setups(0)],
            vec![F::Solutions(0)],
            vec![F::Attempts(0)]
        ]
    );
    assert_eq!(
        fields(&b, K::Comment),
        vec![vec![F::Solutions(0)], vec![F::Attempts(0)]]
    );
    assert!(fields(&b, K::Heading).iter().all(Vec::is_empty));
    assert_eq!(fields(&b, K::Heading).len(), 9);
    let prose = b.iter().find(|b| b.raw == "\nDescribe it.\n\n").unwrap();
    assert_eq!(
        (prose.kind, prose.fields.clone()),
        (K::Prose, vec![F::Description])
    );
}

#[test]
fn project_fences_are_file_fences() {
    let src = "---\nleetcodeType: package\n---\n\n## Files\n\n```ts path=a.ts role=hidden\nx\n```\n\n```ts\nprose\n```\n\n# Solutions\n\n```ts path=a.ts\ny\n```\n";
    let b = ok(src);
    assert_eq!(
        fields(&b, K::FileFence),
        vec![vec![F::Files(0)], vec![F::SolutionFiles(0)]]
    );
    assert_eq!(fields(&b, K::CodeFence), vec![Vec::<F>::new()]);
}

#[test]
fn a_second_tests_section_and_second_setup_fence_feed_nothing() {
    let src = "## Tests\n```json\n[{\"input\":{},\"expected\":1}]\n```\n## Tests\n```json\n[{\"input\":{},\"expected\":2}]\n```\n# Setup\n## Py\n```py\na\n```\n```py\nb\n```\n";
    let b = ok(src);
    assert_eq!(fields(&b, K::CaseFence), vec![vec![F::Tests(0)]]);
    assert_eq!(
        fields(&b, K::CodeFence),
        vec![vec![], vec![F::Setups(0)], vec![]]
    );
}

#[test]
fn oversized_frontmatter_is_covered_but_maps_nothing() {
    let mut src = String::from("---\n");
    src.push_str(&"title: xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\n".repeat(1700));
    src.push_str("---\nbody\n");
    let b = ok(&src);
    assert!(fields(&b, K::FmKey).iter().all(Vec::is_empty));
}

#[test]
fn block_cap_is_enforced_before_building() {
    // Exactly the cap passes, one more is refused; 1 MiB of fence lines is refused.
    let at_cap = "```\n```\n".repeat(MAX_LAYOUT_BLOCKS);
    assert_eq!(ok(&at_cap).len(), MAX_LAYOUT_BLOCKS);
    let over = "```\n```\n".repeat(MAX_LAYOUT_BLOCKS + 1);
    assert_eq!(layout(&over), Err(LayoutRefusal::TooManyBlocks));
    assert_eq!(
        layout(&"```\n".repeat(1 << 18)),
        Err(LayoutRefusal::TooManyBlocks)
    );
}

#[test]
fn solutions_index_like_the_tree() {
    let src = "# Solutions\n\n```python\norphan\n```\n\n## Python\n\n### A\n\n```python\na1\n```\n\n```python\na2\n```\n\n### B\n\n```python\nb\n```\n\n## Java\n\n```java\nj\n```\n";
    let b = ok(src);
    assert_eq!(
        fields(&b, K::CodeFence),
        vec![
            vec![],
            vec![F::Solutions(0)],
            vec![],
            vec![F::Solutions(1)],
            vec![F::Solutions(2)]
        ]
    );
}

#[test]
fn attempts_index_only_valid_commented_fences() {
    let ok_c = "<!-- attempt: {\"at\":\"t\",\"duration\":\"1s\",\"passed\":true} -->";
    let src = format!(
        "# Attempts\n\n## Python\n\n```python\nplain\n```\n\n<!-- attempt: {{}} -->\n```python\nbad\n```\n\n{ok_c}\n```python\nreal\n```\n"
    );
    let b = ok(&src);
    assert_eq!(
        fields(&b, K::CodeFence),
        vec![vec![], vec![], vec![F::Attempts(0)]]
    );
    assert_eq!(fields(&b, K::Comment), vec![vec![], vec![F::Attempts(0)]]);
}

#[test]
fn lead_fences_feed_the_description() {
    let b = ok("intro\n\n```text\nx\n```\n\nmore\n# Setup\n");
    assert_eq!(fields(&b, K::CodeFence), vec![vec![F::Description]]);
}

#[test]
fn body_set_keys_in_frontmatter_feed_nothing() {
    let b = ok("---\nfunction: f\ntitle: T\n---\n");
    assert_eq!(fields(&b, K::FmKey), vec![vec![], vec![F::Title]]);
}

#[test]
fn path_attribute_follows_the_project_parser() {
    let src = "## Files\n\n```ts path=\"abc\nz\n```\n\n```ts path=\"\" path=x\ny\n```\n";
    let b = ok(src);
    let kinds: Vec<K> = b
        .iter()
        .map(|b| b.kind)
        .filter(|k| matches!(k, K::FileFence | K::CodeFence))
        .collect();
    assert_eq!(kinds, [K::FileFence, K::CodeFence]);
}
