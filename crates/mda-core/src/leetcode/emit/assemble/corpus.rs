//! Whole-corpus and hostile-input checks for the assembler (T7.5a): every conformance input
//! round-trips byte for byte with its layout and canonically without one.

use super::super::assemble;
use super::tests::rich;
use crate::leetcode::{Block, BlockKind as K, MAX_LAYOUT_BLOCKS, layout, parse, write_guard};

fn conformance_sources() -> Vec<(String, String)> {
    fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "md") {
                out.push(p);
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/leetcode");
    let mut files = vec![];
    walk(&root, &mut files);
    files.sort();
    assert!(files.len() > 50);
    files
        .iter()
        .map(|f| (f.display().to_string(), std::fs::read_to_string(f).unwrap()))
        .collect()
}

#[test]
fn round_trip_identity_over_every_conformance_input() {
    for (name, src) in conformance_sources() {
        let blocks = layout(&src).unwrap();
        let x = parse(&src);
        assert_eq!(
            assemble(&x, Some(&blocks)).as_deref(),
            Ok(src.as_str()),
            "{name}"
        );
        assert_eq!(write_guard(&x, &src), Ok(()), "{name}");
    }
}

/// Inputs whose value has no canonical file: an empty or invalid title (nothing to put in
/// frontmatter), a description that holds an unclosed fence, and a bare `## Tests` fence beside
/// several checks plus an unknown `check=` (the binding is not in the model).
const UNREPRESENTABLE: [&str; 9] = [
    "lc-config-blocks-hidden-by-open-fence",
    "lc-config-blocks-unterminated",
    "lc-golden-bom",
    "lc-golden-empty-file",
    "lc-golden-no-frontmatter",
    "lc-hostile-control-chars",
    "lc-hostile-frontmatter-64k-plus-1",
    "lc-parser-u2028-title",
    "lc-project-parser-fence-binding",
];

#[test]
fn canonical_round_trip_without_layout() {
    let mut refused = vec![];
    for (name, src) in conformance_sources() {
        let x = parse(&src);
        let stem = name
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".md")
            .to_owned();
        match assemble(&x, None).map(|out| write_guard(&x, &out)) {
            Ok(Ok(())) => {}
            Ok(Err(e)) => refused.push(format!("{stem}: guard {e:?}")),
            Err(e) => refused.push(format!("{stem}: emit {e:?}")),
        }
    }
    let stems: Vec<&str> = refused.iter().filter_map(|r| r.split(':').next()).collect();
    assert_eq!(stems, UNREPRESENTABLE, "{}", refused.join("\n"));
}

#[test]
fn hostile_layouts_never_panic_and_stay_bounded() {
    let (x, _) = rich();
    let junk = |raw: &str, kind| Block {
        raw: raw.into(),
        kind,
        fields: vec![],
    };
    let kinds = [
        K::FmOpen,
        K::FmKey,
        K::FmClose,
        K::Heading,
        K::ConfigOpen,
        K::ConfigKey,
        K::ConfigComment,
        K::ConfigClose,
        K::CaseFence,
        K::CodeFence,
        K::FileFence,
        K::Comment,
        K::Prose,
    ];
    let raws = [
        "",
        "\n",
        "```",
        "### \n",
        "# Attempts\n",
        "key",
        "\u{0}",
        "---\n",
    ];
    let mix: Vec<Block> = kinds
        .iter()
        .flat_map(|k| raws.iter().map(move |r| junk(r, *k)))
        .collect();
    let _ = assemble(&x, Some(&mix));
    let big: Vec<Block> = (0..MAX_LAYOUT_BLOCKS).map(|_| junk("", K::Prose)).collect();
    let _ = assemble(&x, Some(&big));
    let mut over = big;
    over.push(junk("", K::Prose));
    assert!(assemble(&x, Some(&over)).is_err());
    // fields that point past every list
    let wild = Block {
        raw: "x\n".into(),
        kind: K::CodeFence,
        fields: vec![
            crate::leetcode::LayoutField::Solutions(99),
            crate::leetcode::LayoutField::Tests(7),
        ],
    };
    let _ = assemble(&x, Some(&[wild]));
    let out = assemble(&x, None).unwrap();
    assert!(out.len() < 100_000);
}

#[test]
fn many_blocks_for_one_item_emit_once() {
    use crate::leetcode::LayoutField as F;
    let (x, _) = rich();
    let mut y = x.clone();
    y.solutions[0].code = "changed".into();
    let blocks: Vec<Block> = (0..3000)
        .map(|_| Block {
            raw: "```python\nx\n```\n".into(),
            kind: K::CodeFence,
            fields: vec![F::Solutions(0)],
        })
        .collect();
    super::items::EMITS.with(|c| c.set(0));
    let _ = assemble(&y, Some(&blocks));
    assert!(super::items::EMITS.with(std::cell::Cell::get) <= 1);
}
