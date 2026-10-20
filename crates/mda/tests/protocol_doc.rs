//! Doc-sync guard: `spec/PROTOCOL.md` tables must stay set-equal to the registry, `ALL_CODES`,
//! `RENDER_WARNING_CODES`, the op-case directories and the serve constants.
#![allow(
    clippy::panic,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::indexing_slicing
)] // reason: tests fail loudly

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use mda_ops::error::{ALL_CODES, RENDER_WARNING_CODES};

type Params = BTreeMap<String, (BTreeSet<String>, BTreeSet<String>)>;

fn spec_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec")
}

fn doc() -> String {
    fs::read_to_string(spec_dir().join("PROTOCOL.md")).expect("spec/PROTOCOL.md")
}

fn cases_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/op_cases")
}

/// Sorted `.toml` files in `dir` (empty if it does not exist).
fn case_files(dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = fs::read_dir(dir)
        .map(|rd| rd.filter_map(|e| Some(e.ok()?.path())).collect())
        .unwrap_or_default();
    v.retain(|p| p.extension().is_some_and(|x| x == "toml"));
    v.sort();
    v
}

/// The body of the `## ` section whose heading line is exactly `heading`, up to the next `## `.
fn section<'a>(md: &'a str, heading: &str) -> Option<&'a str> {
    let mut start = None;
    let mut pos = 0;
    for line in md.split_inclusive('\n') {
        let t = line.trim_end();
        if let Some(s) = start {
            if t.starts_with("## ") {
                return Some(&md[s..pos]);
            }
        } else if t == heading {
            start = Some(pos + line.len());
        }
        pos += line.len();
    }
    start.map(|s| &md[s..])
}

/// Table rows (`| \`` lines) outside code fences.
fn rows(sec: &str) -> Vec<&str> {
    let mut fenced = false;
    let mut out = Vec::new();
    for line in sec.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        } else if !fenced && line.starts_with("| `") {
            out.push(line);
        }
    }
    out
}

/// Every backticked span of `s`.
fn spans(s: &str) -> Vec<String> {
    s.split('`')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

fn cells(row: &str) -> Vec<&str> {
    row.trim()
        .trim_matches('|')
        .split('|')
        .map(str::trim)
        .collect()
}

/// First backticked span of each row: the row's name. Prose mentions never count.
fn row_names(sec: &str) -> Vec<String> {
    rows(sec)
        .iter()
        .filter_map(|r| spans(cells(r)[0]).into_iter().next())
        .collect()
}

/// Per row: (required, optional) param keys of cell 2. A trailing `?` inside the span = optional.
fn row_params(sec: &str) -> Params {
    let mut out = Params::new();
    for r in rows(sec) {
        let c = cells(r);
        let Some(name) = spans(c[0]).into_iter().next() else {
            continue;
        };
        let (mut req, mut opt) = (BTreeSet::new(), BTreeSet::new());
        for s in spans(c.get(1).copied().unwrap_or("")) {
            match s.strip_suffix('?') {
                Some(k) => opt.insert(k.to_string()),
                None => req.insert(s),
            };
        }
        out.insert(name, (req, opt));
    }
    out
}

/// Cells 1 and 2 of each row: the first backticked span, or the bare cell (`none`).
fn row_cells(sec: &str) -> Vec<(String, String)> {
    let first = |c: &str| spans(c).into_iter().next().unwrap_or_else(|| c.to_string());
    rows(sec)
        .iter()
        .map(|r| {
            let c = cells(r);
            (first(c[0]), first(c.get(1).copied().unwrap_or("")))
        })
        .collect()
}

/// `missing|extra|duplicate <n>` lines, sorted. Empty = the doc matches the code.
fn diff(doc: &[String], real: &[String]) -> Vec<String> {
    let (d, r): (BTreeSet<_>, BTreeSet<_>) = (doc.iter().collect(), real.iter().collect());
    let mut out: Vec<String> = r.difference(&d).map(|n| format!("missing {n}")).collect();
    out.extend(d.difference(&r).map(|n| format!("extra {n}")));
    out.extend(
        d.iter()
            .filter(|n| doc.iter().filter(|x| x == *n).count() > 1)
            .map(|n| format!("duplicate {n}")),
    );
    out.sort();
    out
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(ToString::to_string).collect()
}

fn op_names() -> Vec<String> {
    mda_ops::ops().iter().map(|o| o.name.to_string()).collect()
}

fn sec(md: &str, h: &str) -> String {
    section(md, h)
        .unwrap_or_else(|| panic!("missing {h}"))
        .to_string()
}

/// `Some(problem)` when `keys` do not fit the documented params of `code`.
fn param_problem(table: &Params, code: &str, keys: &BTreeSet<String>) -> Option<String> {
    let Some((req, opt)) = table.get(code) else {
        return Some(format!("{code}: not in the doc"));
    };
    let allowed: BTreeSet<_> = req.union(opt).cloned().collect();
    (!req.is_subset(keys) || !keys.is_subset(&allowed))
        .then(|| format!("{code}: keys {keys:?} vs required {req:?} optional {opt:?}"))
}

#[test]
fn prose_mention_is_not_a_row() {
    let mut md = String::from("## 3. Methods\n\nsee `system.ops` here\n\n");
    for n in op_names().iter().filter(|n| *n != "system.ops") {
        md.push_str(&format!("| `{n}` | x |\n"));
    }
    md.push_str("\n## 4. Errors\n");
    let got = row_names(&sec(&md, "## 3. Methods"));
    assert_eq!(got.len(), op_names().len() - 1);
    assert_eq!(diff(&got, &op_names()), ["missing system.ops"]);
}

#[test]
fn fenced_rows_do_not_count() {
    let md = "## 3. Methods\n```\n| `a.b` | x |\n```\n| `c.d` | y |\n## 4. Errors\n| `e.f` | z |\n";
    assert_eq!(row_names(&sec(md, "## 3. Methods")), ["c.d"]);
}

#[test]
fn optional_marker_inside_backticks() {
    let p = row_params("| `x.y` | `a`, `b?` |\n");
    assert_eq!(p["x.y"].0, BTreeSet::from(["a".to_string()]));
    assert_eq!(p["x.y"].1, BTreeSet::from(["b".to_string()]));
    assert!(row_params("| `x.y` | — |\n")["x.y"].0.is_empty());
}

#[test]
fn diff_reports_extra_and_duplicate() {
    let got = diff(&strings(&["a", "a", "z"]), &strings(&["a", "b"]));
    assert_eq!(got, ["duplicate a", "extra z", "missing b"]);
}

#[test]
fn methods_match_registry() {
    let md = doc();
    assert_eq!(
        diff(&row_names(&sec(&md, "## 3. Methods")), &op_names()),
        Vec::<String>::new()
    );
}

#[test]
fn codes_match_all_codes() {
    let md = doc();
    let real = strings(ALL_CODES);
    assert_eq!(
        diff(&row_names(&sec(&md, "## 5. Error codes")), &real),
        Vec::<String>::new()
    );
}

#[test]
fn warnings_match() {
    let md = doc();
    let real = strings(RENDER_WARNING_CODES);
    assert_eq!(
        diff(&row_names(&sec(&md, "## 6. Warning codes")), &real),
        Vec::<String>::new()
    );
}

#[test]
fn method_rows_link_case_dirs() {
    let md = doc();
    for row in rows(&sec(&md, "## 3. Methods")) {
        let op = &spans(cells(row)[0])[0];
        let text = format!("crates/mda/tests/op_cases/{op}/");
        let c1 = cells(row)[1];
        assert!(
            c1.starts_with(&format!("[`{text}`](../{text})")),
            "{op}: bad link {c1}"
        );
        assert!(
            spec_dir().join("..").join(&text).is_dir(),
            "{op}: no case dir"
        );
    }
}

#[test]
fn header_pins() {
    let md = doc();
    assert!(md.contains(&format!("**Protocol version:** `{}`", mda_ops::PROTOCOL)));
    assert!(md.contains(&format!(
        "**Max line:** `{}` bytes",
        mda::serve::MAX_LINE_BYTES
    )));
}

#[test]
fn param_check_flags_missing_and_extra_keys() {
    let t = row_params("| `a.b` | `x`, `y?` |\n");
    let k = |v: &[&str]| v.iter().map(ToString::to_string).collect::<BTreeSet<_>>();
    assert_eq!(param_problem(&t, "a.b", &k(&["x"])), None);
    assert_eq!(param_problem(&t, "a.b", &k(&["x", "y"])), None);
    assert!(param_problem(&t, "a.b", &k(&["y"])).is_some());
    assert!(param_problem(&t, "a.b", &k(&["x", "z"])).is_some());
    assert!(param_problem(&t, "c.d", &k(&[])).is_some());
}

#[test]
fn error_params_match_doc() {
    let md = doc();
    let table = row_params(&sec(&md, "## 5. Error codes"));
    let mut bad = Vec::new();
    for dir in fs::read_dir(cases_dir()).expect("op_cases").flatten() {
        for f in case_files(&dir.path()) {
            let v: toml::Value = toml::from_str(&fs::read_to_string(&f).unwrap()).unwrap();
            let Some(p) = v.get("expect_error_params").and_then(|p| p.as_table()) else {
                continue;
            };
            let code = v["expect_error"].as_str().expect("expect_error");
            let keys = p.keys().cloned().collect();
            bad.extend(param_problem(&table, code, &keys).map(|m| format!("{}: {m}", f.display())));
        }
    }
    assert!(bad.is_empty(), "{bad:#?}");
}

#[test]
fn client_notes_target_ops() {
    let md = doc();
    let ops = op_names();
    let got = row_cells(&sec(&md, "## 11. Client notes"));
    assert!(!got.is_empty());
    for (name, target) in got {
        assert!(
            target == "none" || ops.contains(&target),
            "{name} -> {target}"
        );
    }
}
