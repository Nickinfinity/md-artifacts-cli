//! W7c (spec §7.6): `serialize(from_parsed(parse(f)))` writes `f` back byte for byte when `f` is in
//! the canonical layout — flags with their names, notes and `***` chrome, bare bodies, `paths` —
//! and anything the format cannot carry is refused by the guard, never written lossy.
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)] // reason: assertion helpers outside #[test] fns; a panic is the failure signal

use mda_core::error::FieldReason;
use mda_core::model::{ArtifactModel, PayloadKind};
use mda_core::parse::parse_from_content;
use mda_core::serialize::{SerializeError, from_parsed, serialize};
use serde_json::{Value, json};

const MAX: usize = 1 << 20;

fn round_trip(src: &str, path: &str) -> String {
    serialize(&from_parsed(&parse_from_content(src, path)), MAX).unwrap()
}

fn refused(v: Value) -> (String, FieldReason) {
    let m: ArtifactModel = serde_json::from_value(v).unwrap();
    match serialize(&m, MAX) {
        Err(SerializeError::Field { field, reason }) => (field, reason),
        other => panic!("expected Field, got {other:?}"),
    }
}

const FLAGGED: &str = "---
artifactType: AIPrompt
title: Code Review Request
tags:
  - review
---

Notes to myself, outside the flags.
Second line.

%%oa:start%%
***
Review `<VK-branch>` against <VK-base>.

```bash
git diff <VK-base>
```

## Not a heading here
***
%%oa:end%%

More notes, after.

vars:
```vks
VK-base: main
```
";

#[test]
fn flagged_file_round_trips_byte_for_byte() {
    // The payload holds a fence and a `## ` line: both refused before W7c, when it was re-fenced.
    assert_eq!(round_trip(FLAGGED, "AIPrompts/r.md"), FLAGGED);
}

#[test]
fn read_reports_the_layout() {
    let p = parse_from_content(FLAGGED, "AIPrompts/r.md");
    assert_eq!(p.layout.payload, PayloadKind::Flags);
    assert_eq!(
        p.layout.regions[0].notes,
        "Notes to myself, outside the flags.\nSecond line."
    );
    assert!(p.layout.regions[0].rule);
    assert_eq!(p.layout.notes, "More notes, after.");
    let v = serde_json::to_value(&p).unwrap();
    assert_eq!(v["layout"]["payload"], "flags");
}

const MULTI: &str = "---
artifactType: AIPrompt
title: Refactor
---

Intro notes.

%%oa:start Scope%%
***
Map <VK-area>.
***
%%oa:end%%

Between the regions.

%%oa:start Plan%%
Plan for <VK-area>.
%%oa:end%%

vars:
```vks
VK-area: src
```
";

#[test]
fn named_regions_and_notes_between_them_round_trip() {
    assert_eq!(round_trip(MULTI, "AIPrompts/m.md"), MULTI);
    let p = parse_from_content(MULTI, "AIPrompts/m.md");
    assert_eq!(p.blocks[1].heading, "Plan");
    assert_eq!(p.layout.regions[1].notes, "Between the regions.");
    assert!(!p.layout.regions[1].rule);
}

#[test]
fn single_region_keeps_its_name_and_an_explicit_language() {
    let src = "---\nartifactType: AIAgentsConfig\nlanguage: python\ntarget: run.py\n---\n\n%%oa:start Main%%\nprint(1)\n%%oa:end%%\n";
    assert_eq!(round_trip(src, "AIAgentsConf/a.md"), src);
}

#[test]
fn hand_written_layout_converges_in_one_rewrite() {
    // Unterminated region, no blank lines, defaults fence before the region: one canonical rewrite,
    // after which the file is a fixed point and nothing the reader saw is gone.
    let src = "---\nartifactType: AIPrompt\n---\nnote\nvars:\n\n```vks\nVK-a: 1\n```\n%%oa:start%%\nhi <VK-a>";
    let once = round_trip(src, "AIPrompts/h.md");
    assert_eq!(
        once,
        "---\nartifactType: AIPrompt\n---\n\nnote\n\n%%oa:start%%\nhi <VK-a>\n%%oa:end%%\n\nvars:\n```vks\nVK-a: '1'\n```\n"
    );
    assert_eq!(round_trip(&once, "AIPrompts/h.md"), once);
}

#[test]
fn bare_body_and_paths_round_trip() {
    let src = "---
artifactType: AIAgentsConfig
title: Harness
index: true
paths:
  - agents
  - docs/agents
tags:
  - ai-config
---

Run me to scaffold every agent.

1. [[claude/CLAUDE]]
2. [Codex](codex/AGENTS.md)
";
    assert_eq!(round_trip(src, "AIAgentsConf/i.md"), src);
    let m = from_parsed(&parse_from_content(src, "AIAgentsConf/i.md"));
    assert_eq!(m.paths, ["agents", "docs/agents"]);
    assert_eq!(m.layout.payload, PayloadKind::Bare);
}

#[test]
fn a_client_model_without_regions_gets_default_ones() {
    let m: ArtifactModel = serde_json::from_value(json!({"artifactType":"AIPrompt",
        "layout":{"payload":"flags"},"blocks":[{"code":"# Hi"}]}))
    .unwrap();
    assert_eq!(
        serialize(&m, MAX).unwrap(),
        "---\nartifactType: AIPrompt\n---\n\n%%oa:start%%\n# Hi\n%%oa:end%%\n"
    );
}

#[test]
fn notes_holding_a_start_flag_are_refused() {
    let r = refused(json!({"artifactType":"AIPrompt","blocks":[{"code":"x"}],
        "layout":{"payload":"flags","regions":[{"name":"","notes":"%%oa:start%%"}]}}));
    assert_eq!(r.1, FieldReason::RoundTrip);
}

#[test]
fn payload_holding_an_end_flag_or_a_rule_line_is_refused() {
    for code in ["a\n%%oa:end%%\nb", "a\n***\nb"] {
        let r = refused(json!({"artifactType":"AIPrompt","blocks":[{"code":code}],
            "layout":{"payload":"flags"}}));
        assert_eq!(
            r,
            ("blocks/0/code".to_owned(), FieldReason::RoundTrip),
            "{code}"
        );
    }
}

#[test]
fn blocks_disagreeing_on_a_default_are_refused() {
    // The reader gives every region every default (§7.3), so two values for one name cannot be written.
    let r = refused(
        json!({"artifactType":"AIPrompt","layout":{"payload":"flags"},"blocks":[
        {"heading":"A","code":"<VK-a>","vars":[{"name":"VK-a","value":"1"}]},
        {"heading":"B","code":"<VK-a>","vars":[{"name":"VK-a","value":"2"}]}]}),
    );
    assert_eq!(r.1, FieldReason::RoundTrip);
}

#[test]
fn layout_the_type_cannot_carry_is_refused() {
    // Bare is whole-file only; flags are not a Variables shape; notes need flags.
    for v in [
        json!({"artifactType":"Snippet","layout":{"payload":"bare"},"blocks":[{"code":"x"}]}),
        json!({"artifactType":"Variables","layout":{"payload":"flags"},"blocks":[]}),
        json!({"artifactType":"Snippet","layout":{"notes":"n"},"blocks":[{"code":"x"}]}),
    ] {
        assert_eq!(refused(v.clone()).1, FieldReason::RoundTrip, "{v}");
    }
}
