//! T3b.1: `safe_rel_path`, link extraction, plans, candidates, `is_index`.
#![allow(clippy::unwrap_used)] // reason: test file

mod common;
use common::fuzz_strings;
use mda_core::model::Frontmatter;
use mda_core::multi_index::{
    RelPathError as E, RewriteError, dest_candidates, extract_links, is_index, plan_links,
    resolve_link, rewrite_links, safe_rel_path,
};
use mda_core::registry::ArtifactType;
use std::time::{Duration, Instant};

#[test]
fn safe_rel_path_table() {
    let table: &[(&str, Result<&str, E>)] = &[
        ("dir_2/subdir1/Button.md", Ok("dir_2/subdir1/Button.md")),
        ("./dir//sub/file.md", Ok("dir/sub/file.md")),
        ("../etc/passwd", Err(E::ParentSegment)),
        ("dir/../../etc/passwd", Err(E::ParentSegment)),
        ("/etc/passwd", Err(E::Absolute)),
        ("C:\\Windows\\System32\\evil.md", Err(E::DriveOrScheme)),
        ("C:\\Windows\\evil", Err(E::DriveOrScheme)),
        ("C:\\evil", Err(E::DriveOrScheme)),
        ("file:///etc/passwd", Err(E::DriveOrScheme)),
        ("dir_2\\subdir1\\Button.md", Err(E::Backslash)),
        ("dir\\sub\\file", Err(E::Backslash)),
        ("dir\\sub", Err(E::Backslash)),
        ("dir/evil\u{0}.md", Err(E::ControlChar)),
        ("dir/evil\u{0}", Err(E::ControlChar)),
        ("..%2f..%2fetc%2fpasswd", Ok("..%2f..%2fetc%2fpasswd")),
        ("..%2f..%2fetc", Ok("..%2f..%2fetc")),
        ("", Err(E::Empty)),
        (".", Err(E::Empty)),
        ("/", Err(E::Absolute)),
        ("./", Err(E::Empty)),
        ("\u{85}a", Err(E::ControlChar)),
        ("a\u{7f}", Err(E::ControlChar)),
        ("\u{0}:", Err(E::ControlChar)),
        (":\\\\", Err(E::DriveOrScheme)),
        ("\\\\/x", Err(E::Backslash)),
        ("/..", Err(E::Absolute)),
        ("a//./b/", Ok("a/b")),
    ];
    for (input, want) in table {
        assert_eq!(safe_rel_path(input), want.map(str::to_owned), "{input:?}");
    }
}

#[test]
fn resolve_link_cases() {
    assert_eq!(
        resolve_link("dir_2/subdir1/Button").unwrap(),
        "dir_2/subdir1/Button.md"
    );
    assert_eq!(
        resolve_link("dir/Button.test.md").unwrap(),
        "dir/Button.test.md"
    );
    assert_eq!(resolve_link("dir/Button.MD").unwrap(), "dir/Button.MD");
    assert_eq!(
        resolve_link("..%2f..%2fetc%2fpasswd").unwrap(),
        "..%2f..%2fetc%2fpasswd.md"
    );
    for bad in [
        "../escape",
        "/etc/passwd",
        "C:\\evil",
        "dir\\sub",
        "dir/evil\u{0}",
        "file:///etc/passwd",
    ] {
        assert!(resolve_link(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn extract_links_cases() {
    let cases: &[(&str, &[&str])] = &[
        (
            "1. [[dir_2/subdir1/Button]]\n2. [T](dir_2/subdir1/Button.test.md)",
            &["dir_2/subdir1/Button", "dir_2/subdir1/Button.test.md"],
        ),
        ("[[dir/Button|Button component]]", &["dir/Button"]),
        ("[[dir/Button#Usage]]", &["dir/Button"]),
        ("[[dir/Button#Usage|see usage]]", &["dir/Button"]),
        ("[[dir/a]]\n[[dir/a]]", &["dir/a", "dir/a"]),
        ("Scaffolds a component.\n[[dir/a]]\nDone.", &["dir/a"]),
        ("![alt](dir/pic.png)\n[[dir/a]]", &["dir/a"]),
        ("![[dir/preview]]\n[[dir/a]]", &["dir/a"]),
        ("![[dir/pic.png]]", &[]),
        ("[[dir/a[1]]]", &[]),
        ("[T](dir/a(1).md)", &[]),
        ("[[dir/a\nb]]", &[]),
        ("", &[]),
        ("!![[n]]", &[]),
        ("![a](u)", &[]),
        ("[[a]]![[b]][[c]]", &["a", "c"]),
        ("![T]([[x]])", &["x"]),
        ("[[ a ]]", &["a"]),
        ("[T]( x.md )", &["x.md"]),
        ("[[a#h|b]]", &["a"]),
        ("[[\u{feff}a]]", &["a"]),
    ];
    for (body, want) in cases {
        assert_eq!(extract_links(body), *want, "{body:?}");
    }
}

#[test]
fn floods_are_linear() {
    for unit in ["[[", "![", "![](", "[](", "[[[["] {
        let body = unit.repeat((1 << 20) / unit.len());
        let t = Instant::now();
        assert!(extract_links(&body).is_empty(), "{unit:?}");
        assert!(
            t.elapsed() < Duration::from_secs(1),
            "{unit:?} {:?}",
            t.elapsed()
        );
    }
    let t = Instant::now();
    assert_eq!(extract_links(&"[[a]]".repeat(100_000)).len(), 100_000);
    assert!(t.elapsed() < Duration::from_secs(5));
    let t = Instant::now();
    assert!(extract_links(&"![[a]]".repeat(100_000)).is_empty());
    assert!(t.elapsed() < Duration::from_secs(5));
}

#[test]
fn plan_links_cases() {
    let body = "1. [[dir_2/subdir1/Button]]\n2. [T](dir_2/subdir1/Button.test.md)\n3. [[../escape]]\n4. [[dir_1/barrel]]";
    let p = plan_links(body);
    let raws: Vec<_> = p.steps.iter().map(|s| s.raw.as_str()).collect();
    assert_eq!(
        raws,
        [
            "dir_2/subdir1/Button",
            "dir_2/subdir1/Button.test.md",
            "dir_1/barrel"
        ]
    );
    let rels: Vec<_> = p.steps.iter().map(|s| s.rel_path.as_str()).collect();
    assert_eq!(
        rels,
        [
            "dir_2/subdir1/Button.md",
            "dir_2/subdir1/Button.test.md",
            "dir_1/barrel.md"
        ]
    );
    let dirs: Vec<_> = p.steps.iter().map(|s| s.rel_dir.as_str()).collect();
    assert_eq!(dirs, ["dir_2/subdir1", "dir_2/subdir1", "dir_1"]);
    assert_eq!(p.rejected, vec![("../escape".to_owned(), E::ParentSegment)]);
    assert_eq!(plan_links("[[Button]]").steps[0].rel_dir, "");
    let d = plan_links("[[dir/a]]\n[[dir/a]]");
    assert_eq!(d.steps.len(), 2);
    assert_eq!(d.steps[1].rel_path, "dir/a.md");
    assert_eq!(plan_links("just prose, no links"), Default::default());
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_owned()).collect()
}

#[test]
fn dest_candidates_cases() {
    assert_eq!(
        dest_candidates("src/app", "dir_2/subdir1", &[]),
        s(&["src/app/dir_2/subdir1"])
    );
    assert_eq!(dest_candidates("", "", &[]), s(&[""]));
    assert_eq!(
        dest_candidates(
            "src/app",
            "dir_2/subdir1",
            &s(&["src/components", "packages/ui/src"])
        ),
        s(&["src/app/dir_2/subdir1", "src/components", "packages/ui/src"])
    );
    assert_eq!(
        dest_candidates(
            "src/app",
            "dir_2/subdir1",
            &s(&["src/app/dir_2/subdir1", "src/components"])
        ),
        s(&["src/app/dir_2/subdir1", "src/components"])
    );
    assert_eq!(
        dest_candidates("", "", &s(&["../escape", "src/components"])),
        s(&["", "src/components"])
    );
    for bad in [
        "../escape",
        "/etc/passwd",
        "C:\\evil",
        "dir\\sub",
        "dir/evil\u{0}",
        "file:///etc/passwd",
    ] {
        assert_eq!(dest_candidates("", "", &s(&[bad])), s(&[""]), "{bad:?}");
    }
    assert_eq!(
        dest_candidates("", "", &s(&["..%2f..%2fetc"])),
        s(&["", "..%2f..%2fetc"])
    );
    assert_eq!(dest_candidates("", "", &s(&["./a//b"])), s(&["", "a/b"]));
}

#[test]
fn is_index_cases() {
    let fm = |t, i: Option<bool>| {
        let mut f = Frontmatter::new(t);
        f.index = i;
        f
    };
    assert!(is_index(&fm(ArtifactType::Template, Some(true))));
    assert!(is_index(&fm(ArtifactType::AIAgentsConfig, Some(true))));
    assert!(!is_index(&fm(ArtifactType::Snippet, Some(true))));
    assert!(!is_index(&fm(ArtifactType::Command, Some(true))));
    assert!(!is_index(&fm(ArtifactType::Template, None)));
    assert!(!is_index(&fm(ArtifactType::Template, Some(false))));
}

#[test]
fn fuzz_invariants() {
    for x in fuzz_strings(0x1DE7, 5_000, 40) {
        if let Ok(p) = safe_rel_path(&x) {
            assert!(
                !p.starts_with('/') && !p.contains(['\\', ':']) && !p.chars().any(char::is_control),
                "{x:?}"
            );
            assert!(
                p.split('/').all(|seg| !seg.is_empty() && seg != ".."),
                "{x:?}"
            );
        }
        let _ = extract_links(&x);
        let _ = plan_links(&x);
    }
}

// ---- rewrite_links (spec 14.3, D-37) ----

fn rw(body: &str, from: &str, to: &str) -> String {
    rewrite_links(body, from, to).unwrap()
}

#[test]
fn rewrite_keeps_anchor_and_alias() {
    assert_eq!(
        rewrite_links("[[sub/a#h|t]]", "sub/a.md", "sub/b.md").unwrap(),
        "[[sub/b#h|t]]"
    );
    assert_eq!(rw("[[ a # h | t ]]", "a.md", "b.md"), "[[ b # h | t ]]");
    // The `#` after the alias bar is alias text, not an anchor.
    assert_eq!(rw("[[a#h]]", "a.md", "b.md"), "[[b#h]]");
    assert_eq!(rw("[[a|t]]", "a.md", "b.md"), "[[b|t]]");
    assert_eq!(rw("[[a|t#h]]", "a.md", "b.md"), "[[b|t#h]]");
    assert_eq!(rw("[[ |t]] [[a]]", "a.md", "b.md"), "[[ |t]] [[b]]");
}

#[test]
fn rewrite_markdown_and_md_suffix() {
    assert_eq!(
        rw("[Label](sub/a.md)", "sub/a.md", "sub/b.md"),
        "[Label](sub/b.md)"
    );
    assert_eq!(rw("[L]( a.md )", "a.md", "b.md"), "[L]( b.md )");
    assert_eq!(rw("[[a]]", "a.md", "b.md"), "[[b]]");
    assert_eq!(rw("[[a.md]]", "a.md", "b.md"), "[[b.md]]");
    assert_eq!(rw("[[a.MD]]", "a.md", "b.md"), "[[b.md]]");
    assert_eq!(rw("[[./sub//a]]", "sub/a.md", "sub/b.md"), "[[sub/b]]");
}

#[test]
fn rewrite_all_matches_and_case_insensitive() {
    assert_eq!(rw("[[a]] x [[a]]", "a.md", "b.md"), "[[b]] x [[b]]");
    assert_eq!(rw("[[SUB/A]]", "sub/a.md", "sub/b.md"), "[[sub/b]]");
}

#[test]
fn rewrite_leaves_everything_else() {
    for body in [
        "![[a]]",
        "![x](a.md)",
        "[x](a.md \"t\")",
        "[x](a.md#h)",
        "[[../a]] [[c]] [[/a]]",
        "[[\u{e9}]]",
    ] {
        assert_eq!(rw(body, "a.md", "b.md"), body, "{body}");
    }
    assert_eq!(rw("![T]([[x]])", "a.md", "b.md"), "![T]([[x]])");
    assert_eq!(rw("![T]([[x]])", "x.md", "y.md"), "![T]([[y]])");
}

#[test]
fn rewrite_unlinkable_target() {
    for to in ["a#b.md", "a|b.md", "a[b.md", "a]b.md", "a(b.md", "a)b.md"] {
        assert_eq!(
            rewrite_links("[[a]]", "a.md", to),
            Err(RewriteError::Unlinkable),
            "{to}"
        );
    }
    // Not its own normal form: the guard would compare a different path.
    for to in ["b.md.md", "s//b.md", "./b.md", "b"] {
        assert_eq!(
            rewrite_links("[[a]]", "a.md", to),
            Err(RewriteError::Unlinkable),
            "{to}"
        );
    }
    // No matching link: nothing to spell, so no check.
    assert_eq!(
        rewrite_links("[[c]]", "a.md", "a#b.md"),
        Ok("[[c]]".to_owned())
    );
}

#[test]
fn rewrite_is_linear() {
    let body = "[[a]] ".repeat(100_000);
    let t = Instant::now();
    let out = rw(&body, "a.md", "bb.md");
    assert!(t.elapsed() < Duration::from_secs(5));
    assert_eq!(out, "[[bb]] ".repeat(100_000));
}
