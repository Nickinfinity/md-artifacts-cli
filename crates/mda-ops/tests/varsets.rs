//! `varsets.*` / `render.text` ops: size and cost gates, containment sink, ranking edge cases.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // reason: tests

use std::fs;
use std::path::PathBuf;

use mda_core::render::Values;
use mda_core::vks::VksValue;
use mda_ops::artifact::{MAX_ARTIFACT_BYTES, ReadRequest, read};
use mda_ops::render::{TextRequest, text};
use mda_ops::varsets::{MatchRequest, SaveRequest, match_, save};
use mda_ops::{Ctx, Root};

fn vault(name: &str, files: &[(&str, String)]) -> (Ctx, PathBuf) {
    let dir = std::env::temp_dir().join(format!("mda-vsets-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    for d in ["Snippets", "Variables"] {
        fs::create_dir_all(dir.join(d)).unwrap();
    }
    for (p, c) in files {
        let f = dir.join(p);
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(f, c).unwrap();
    }
    (Ctx::new(Some(Root::new(&dir).unwrap())), dir)
}

fn vars_file(title: &str, tags: &str, body: &str) -> String {
    format!("---\nartifactType: Variables\ntitle: {title}\n{tags}---\n\n{body}")
}

fn snippet(tags: &str) -> String {
    format!("---\nartifactType: Snippet\ntitle: S\n{tags}---\n\n```sh\necho <VK-a>\n```\n")
}

fn m(path: &str) -> MatchRequest {
    MatchRequest {
        path: path.into(),
        block: None,
    }
}

fn text_req(text: String, values: Values) -> TextRequest {
    TextRequest { text, values }
}

#[test]
fn text_over_max_is_too_large() {
    let (c, _) = vault("big", &[]);
    let size = usize::try_from(MAX_ARTIFACT_BYTES).unwrap() + 1;
    let e = text(&c, text_req("a".repeat(size), Values::new())).unwrap_err();
    assert_eq!(e.code, "file.too_large");
    assert_eq!(e.params["path"], "");
    assert_eq!(e.params["size"], size.to_string());
    assert_eq!(e.params["max"], MAX_ARTIFACT_BYTES.to_string());
}

#[test]
fn text_output_limit_is_a_code() {
    let (c, _) = vault("lim", &[]);
    let mut v = Values::new();
    v.insert("VK-a".into(), VksValue::Str("a".repeat(2048)));
    let e = text(&c, text_req("<VK-a>".repeat(1000), v)).unwrap_err();
    assert_eq!(e.code, "render.limit");
    assert_eq!(e.params["limit"], "output_bytes");
}

#[cfg(unix)]
#[test]
fn save_through_symlinked_dir_writes_nothing_outside() {
    let (c, dir) = vault("link", &[]);
    let out = std::env::temp_dir().join(format!("mda-vsets-{}-out", std::process::id()));
    let _ = fs::remove_dir_all(&out);
    fs::create_dir_all(&out).unwrap();
    std::os::unix::fs::symlink(&out, dir.join("Variables/sub")).unwrap();
    let req: SaveRequest = serde_json::from_value(
        serde_json::json!({"title": "X", "dir": "Variables/sub", "values": {"VK-a": "b"}}),
    )
    .unwrap();
    let e = save(&c, req).unwrap_err();
    assert_eq!(e.code, "path.outside_root");
    assert_eq!(fs::read_dir(&out).unwrap().count(), 0);
}

#[test]
fn saved_file_reads_back() {
    let (c, _) = vault("rb", &[]);
    let req: SaveRequest =
        serde_json::from_value(serde_json::json!({"title": "Rb", "values": {"VK-a": "b"}}))
            .unwrap();
    let w = save(&c, req).unwrap();
    let r = read(&c, ReadRequest { path: w.path }).unwrap();
    assert_eq!(r.hash, w.hash);
    assert_eq!(r.artifact.vars.len(), 1);
    assert_eq!(r.artifact.vars[0].name, "VK-a");
}

#[test]
fn ties_break_by_path_then_index() {
    let body = "## One\n\n```yaml VKs\nVK-a: x\n```\n\n## Two\n\n```yaml VKs\nVK-a: y\n```\n";
    let (c, _) = vault(
        "tie",
        &[
            ("Snippets/s.md", snippet("")),
            ("Variables/b.md", vars_file("B", "", body)),
            ("Variables/a.md", vars_file("A", "", body)),
        ],
    );
    let r = match_(&c, m("Snippets/s.md")).unwrap();
    let got: Vec<(&str, usize)> = r
        .matches
        .iter()
        .map(|x| (r.paths[x.path_index].as_str(), x.subset.index))
        .collect();
    assert_eq!(
        got,
        [
            ("Variables/a.md", 0),
            ("Variables/a.md", 1),
            ("Variables/b.md", 0),
            ("Variables/b.md", 1)
        ]
    );
}

#[test]
fn one_bad_block_skips_the_whole_file() {
    let body = "## Ok\n\n```yaml VKs\nVK-a: x\n```\n\n## Bad\n\n```yaml VKs\nVK-a: x # note\n```\n";
    let (c, _) = vault(
        "bad",
        &[
            ("Snippets/s.md", snippet("")),
            ("Variables/m.md", vars_file("M", "", body)),
        ],
    );
    let r = match_(&c, m("Snippets/s.md")).unwrap();
    assert!(r.matches.is_empty() && r.paths.is_empty());
}

#[test]
fn block_list_tags_count() {
    let (c, _) = vault(
        "tags",
        &[
            ("Snippets/s.md", snippet("tags:\n  - api\n")),
            (
                "Variables/t.md",
                vars_file("T", "tags: [api]\n", "```yaml VKs\nVK-a: x\n```\n"),
            ),
        ],
    );
    let r = match_(&c, m("Snippets/s.md")).unwrap();
    assert_eq!(r.matches[0].score.tag_matches, 1);
}

#[test]
fn profile_is_built_once_per_call() {
    let sets: String = (0..10_000)
        .map(|i| format!("## S{i}\n\n```yaml VKs\nVK-v{i}: x\n```\n\n"))
        .collect();
    let tokens: String = (0..80_000).map(|i| format!("<VK-n{i}>")).collect();
    let snip = format!("---\nartifactType: Snippet\ntitle: S\n---\n\n```sh\n{tokens}\n```\n");
    let (c, _) = vault(
        "perf",
        &[
            ("Snippets/s.md", snip),
            ("Variables/big.md", vars_file("Big", "", &sets)),
        ],
    );
    let t = std::time::Instant::now();
    let r = match_(&c, m("Snippets/s.md")).unwrap();
    assert_eq!(r.matches.len(), 10_000);
    assert!(t.elapsed().as_secs() < 3, "{:?}", t.elapsed());
}

fn copy_dir(src: &std::path::Path, dst: &std::path::Path) {
    fs::create_dir_all(dst).unwrap();
    for e in fs::read_dir(src).unwrap() {
        let e = e.unwrap();
        let to = dst.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &to);
        } else {
            fs::copy(e.path(), to).unwrap();
        }
    }
}

// H5b.2 cross-op: save → list → match ranks it first → apply → render.text; hashes agree.
#[test]
fn save_list_match_apply_render_agree() {
    let src =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../mda/tests/fixtures/vault_vars");
    let dir = std::env::temp_dir().join(format!("mda-varsets-cross-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    copy_dir(&src, &dir);
    let ctx = Ctx::new(Some(Root::new(&dir.canonicalize().unwrap()).unwrap()));
    let req: SaveRequest = serde_json::from_value(serde_json::json!(
        {"title": "Api Local", "values": {"VK-host": "h", "VK-path": "p"}, "tags": ["api"]}))
    .unwrap();
    let w = save(&ctx, req).unwrap();
    assert_eq!(w.path, "Variables/api-local.md");
    assert_eq!(
        w.hash,
        read(
            &ctx,
            ReadRequest {
                path: w.path.clone()
            }
        )
        .unwrap()
        .hash
    );
    let l = mda_ops::vars::list(&ctx, mda_ops::vars::ListRequest {}).unwrap();
    let f = l.files.iter().find(|f| f.path == w.path).unwrap();
    assert_eq!(serde_json::to_value(f.shape).unwrap(), "flat");
    let r = match_(&ctx, m("Snippets/api.md")).unwrap();
    let top = &r.matches[0];
    assert_eq!(r.paths[top.path_index], w.path);
    assert_eq!(top.score.match_ratio, 0.6666666666666666);
    assert_eq!(top.score.tag_matches, 1);
    let a = mda_ops::varsets::apply(
        &ctx,
        serde_json::from_value(serde_json::json!({"path": w.path, "values": {}})).unwrap(),
    )
    .unwrap();
    assert_eq!(a.values.get("VK-host"), Some(&VksValue::Str("h".into())));
    assert_eq!(a.values.get("VK-path"), Some(&VksValue::Str("p".into())));
    let out = text(
        &ctx,
        TextRequest {
            text: "<VK-host>/<VK-path>".into(),
            values: a.values,
        },
    )
    .unwrap();
    assert_eq!(out.output, "h/p");
    fs::remove_dir_all(dir).unwrap();
}
