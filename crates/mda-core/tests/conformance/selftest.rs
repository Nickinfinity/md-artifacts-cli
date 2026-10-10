//! Harness self-tests: temp manifests exercising each kind handler and the walker.
#![allow(clippy::unwrap_used, clippy::expect_used)] // test file: a failing test is the point

use super::*;

fn tmp_with(name: &str, manifest: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let d = std::env::temp_dir().join(format!("mda-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("manifest.toml"), manifest).unwrap();
    for (f, bytes) in files {
        std::fs::create_dir_all(d.join(f).parent().unwrap()).unwrap();
        std::fs::write(d.join(f), bytes).unwrap();
    }
    d
}

fn tmp(name: &str, manifest: &str, files: &[&str]) -> PathBuf {
    let files: Vec<(&str, &[u8])> = files.iter().map(|f| (*f, &b"x"[..])).collect();
    tmp_with(name, manifest, &files)
}

fn err_of(d: &Path) -> String {
    let e = check(d).unwrap_err().join("\n");
    std::fs::remove_dir_all(d).unwrap();
    e
}

fn case(id: &str, kind: &str, input: &str) -> String {
    format!("[[cases]]\nid = \"{id}\"\nkind = \"{kind}\"\ninput = \"{input}\"\n")
}

#[test]
fn unknown_kind_names_id() {
    let d = tmp(
        "bogus",
        &case("c-bogus", "bogus", "parse/a.md"),
        &["parse/a.md"],
    );
    assert!(err_of(&d).contains("c-bogus"));
}

#[test]
fn missing_input_names_path() {
    let d = tmp("missing", &case("c1", "bogus", "parse/gone.md"), &[]);
    assert!(err_of(&d).contains("parse/gone.md"));
}

#[test]
fn unlisted_file_named() {
    let d = tmp("unlisted", "", &["parse/stray.md"]);
    assert!(err_of(&d).contains("stray.md"));
}

const RMD: &[u8] = b"---\nartifactType: Snippet\n---\n\n```text\nHi <VK-a> <VK-b>\n```\n";

fn render_case(extra: &str) -> String {
    format!("{}{extra}", case("r1", "render", "render/a.md"))
}

/// Expected `Rendered` JSON for `RMD` with a=X, from the real engine so the test hand-writes no model.
fn rendered_json(block: Option<usize>) -> String {
    let p = mda_core::parse::parse_from_content(&mda_core::parse::decode(RMD), "Snippets/a.md");
    let v = serde_json::from_str(r#"{"VK-a":"X"}"#).unwrap();
    let r = mda_core::render::render_artifact(&p, block, None, &v).unwrap();
    serde_json::to_string(&r).unwrap()
}

fn render_dir(name: &str, m: &str, exp: &str, values: &str) -> PathBuf {
    tmp_with(
        name,
        m,
        &[
            ("render/a.md", RMD),
            ("render/a.json", exp.as_bytes()),
            ("render/a.values.json", values.as_bytes()),
        ],
    )
}

const RCASE: &str =
    "path = \"Snippets/a.md\"\nexpected = \"render/a.json\"\nvalues = \"render/a.values.json\"\n";

#[test]
fn render_case_matches() {
    let d = render_dir(
        "rok",
        &render_case(RCASE),
        &rendered_json(None),
        r#"{"VK-a":"X"}"#,
    );
    assert_eq!(check(&d), Ok(()));
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn render_diff_names_id_and_pointer() {
    // warnings are part of the compared JSON: output alone would hide a regression there
    let mut v: serde_json::Value = serde_json::from_str(&rendered_json(None)).unwrap();
    v["warnings"] = serde_json::json!([]);
    let d = render_dir(
        "rdiff",
        &render_case(RCASE),
        &v.to_string(),
        r#"{"VK-a":"X"}"#,
    );
    let e = err_of(&d);
    assert!(e.contains("r1") && e.contains("/warnings"), "{e}");
}

#[test]
fn render_case_without_path_names_id() {
    let m = render_case("expected = \"render/a.json\"\n");
    let d = render_dir("rnopath", &m, "{}", "{}");
    assert!(err_of(&d).contains("r1: render needs path and expected"));
}

#[test]
fn render_bad_values_names_id() {
    // CR in a value: check_values refuses (vks.control_char), not the renderer
    let d = render_dir("rbad", &render_case(RCASE), "{}", "{\"VK-a\":\"a\\rb\"}");
    let e = err_of(&d);
    assert!(e.contains("r1: values: vks.control_char"), "{e}");
}

#[test]
fn render_block_refusal_names_id() {
    let m = render_case(&format!("{RCASE}block = 3\n"));
    let d = render_dir("rblk", &m, "{}", "{}");
    assert!(err_of(&d).contains("r1: refused"));
}

const MODEL: &str = r#"{"artifactType":"Snippet","title":"Hi","blocks":[{"code":"x"}]}"#;
const MODEL_MD: &str = "---\nartifactType: Snippet\ntitle: Hi\n---\n\n```\nx\n```\n";

fn ser_case(model: &str, md: &str, name: &str) -> PathBuf {
    let m = format!(
        "{}expected = \"serialize/a.md\"\n",
        case("s1", "serialize", "serialize/a.model.json")
    );
    tmp_with(
        name,
        &m,
        &[
            ("serialize/a.model.json", model.as_bytes()),
            ("serialize/a.md", md.as_bytes()),
        ],
    )
}

#[test]
fn serialize_case_matches() {
    let d = ser_case(MODEL, MODEL_MD, "sok");
    assert_eq!(check(&d), Ok(()));
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn serialize_diff_names_id_and_line() {
    let d = ser_case(
        MODEL,
        "---\nartifactType: Snippet\ntitle: Wrong\n---\n\n```\nx\n```\n",
        "sdiff",
    );
    let e = err_of(&d);
    assert!(e.contains("s1: line 3: got") && e.contains("Wrong"), "{e}");
}

#[test]
fn serialize_unknown_model_field_names_id() {
    let d = ser_case(r#"{"artifactType":"Snippet","bogus":1}"#, MODEL_MD, "sunk");
    let e = err_of(&d);
    assert!(e.contains("s1") && e.contains("bogus"), "{e}");
}

#[test]
fn serialize_refusal_names_id() {
    let d = ser_case(
        r#"{"artifactType":"Template","blocks":[{},{}]}"#,
        MODEL_MD,
        "sref",
    );
    assert!(err_of(&d).contains("s1: refused"));
}

#[test]
fn serialize_case_without_expected_names_id() {
    let m = case("s1", "serialize", "serialize/a.model.json");
    let d = tmp_with(
        "snoexp",
        &m,
        &[("serialize/a.model.json", MODEL.as_bytes())],
    );
    assert!(err_of(&d).contains("s1: serialize needs expected"));
}

const MD: &[u8] = b"---\ntitle: Hi\n---\n";

fn parse_case(extra: &str) -> String {
    format!("{}{extra}", case("p1", "parse", "parse/a.md"))
}

/// Expected JSON for `MD`, taken from the real parser so the test needs no hand-written model.
fn expected_json(bytes: &[u8]) -> String {
    let p = mda_core::parse::parse_from_content(&mda_core::parse::decode(bytes), "a.md");
    serde_json::to_string(&p).unwrap()
}

#[test]
fn parse_case_matches() {
    let m = parse_case("path = \"a.md\"\nexpected = \"parse/a.json\"\n");
    let exp = expected_json(MD);
    let d = tmp_with(
        "pok",
        &m,
        &[("parse/a.md", MD), ("parse/a.json", exp.as_bytes())],
    );
    assert_eq!(check(&d), Ok(()));
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn parse_case_diff_names_pointer() {
    let m = parse_case("path = \"a.md\"\nexpected = \"parse/a.json\"\n");
    let mut v: serde_json::Value = serde_json::from_str(&expected_json(MD)).unwrap();
    v["frontmatter"]["title"] = "Wrong".into();
    let d = tmp_with(
        "pdiff",
        &m,
        &[
            ("parse/a.md", MD),
            ("parse/a.json", v.to_string().as_bytes()),
        ],
    );
    let e = err_of(&d);
    assert!(e.contains("p1") && e.contains("/frontmatter/title"), "{e}");
}

#[test]
fn parse_case_without_path_names_id() {
    let m = parse_case("expected = \"parse/a.json\"\n");
    let d = tmp_with(
        "pnopath",
        &m,
        &[("parse/a.md", MD), ("parse/a.json", b"{}")],
    );
    assert!(err_of(&d).contains("p1: parse needs path and expected"));
}

#[test]
fn parse_case_decodes_bom() {
    let bom = [&b"\xEF\xBB\xBF"[..], MD].concat();
    let m = parse_case("path = \"a.md\"\nexpected = \"parse/a.json\"\n");
    let exp = expected_json(MD); // no-BOM parse: BOM must have been stripped
    let d = tmp_with(
        "pbom",
        &m,
        &[("parse/a.md", &bom), ("parse/a.json", exp.as_bytes())],
    );
    assert_eq!(check(&d), Ok(()));
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn duplicate_id_fails() {
    let m = format!(
        "{}{}",
        case("dup", "bogus", "parse/a.md"),
        case("dup", "bogus", "parse/a.md")
    );
    let d = tmp("dup", &m, &["parse/a.md"]);
    assert!(err_of(&d).contains("dup: duplicate id"));
}

#[test]
fn typo_field_is_rejected() {
    let m = format!("{}expectd = \"x\"\n", case("c1", "parse", "parse/a.md"));
    let d = tmp("typo", &m, &["parse/a.md"]);
    assert!(err_of(&d).contains("expectd"));
}
