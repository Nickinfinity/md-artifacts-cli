//! Canonical project content (W7 T7.4): the whole `test:` group (scalars + `checks`, Q-76), `libs`,
//! `packages`, `program`, and `## Files` path fences. Each group is written, then read back through
//! its own parser and compared: a value the parser would drop or change is refused here, never
//! discovered by the write guard. Values quote only through `quote` where the reader is
//! `unquote_pair`; `program` reads bare (`kv_line` + `js_trim`), so its values go out as they are and
//! the read-back refuses what cannot survive (a comma in a flag, an edge quote, a leading space).

use serde::Serialize;
use serde_json::Value;

use super::quote::pair_quoted;
use crate::error::NotSerializableReason as Why;
use crate::leetcode::fm_blocks::parse_test_block;
use crate::leetcode::{
    FileRole, FileSpec, LeetCodeExercise, ProjectCheck, TestConfig, checks::parse_checks,
    packages::parse_packages, program::parse_program, project::parse_lib_declarations,
    project::parse_project_artifact,
};

#[cfg(test)]
mod tests;

type Group = Result<Option<String>, Why>;

/// A value through `pair_quoted`; a line break inside it has no form that reads back.
fn q(s: &str) -> Result<String, Why> {
    pair_quoted(s).ok_or(Why::Other)
}

/// The serde wire spelling of a unit enum (`program`, `flags`, ...).
fn wire(v: &impl Serialize) -> Result<String, Why> {
    match serde_json::to_value(v) {
        Ok(Value::String(s)) => Ok(s),
        _ => Err(Why::Other),
    }
}

/// A JSON array of strings on one line (the `argv` readers go through `safe_json`).
fn json_array(v: &[String]) -> Result<String, Why> {
    serde_json::to_string(v).map_err(|_| Why::Other)
}

/// The lines the parsers see: `\n`-split, a `\r` before it dropped.
fn lines_of(text: &str) -> Vec<&str> {
    text.split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect()
}

/// A key group is only ever one fence's lines: a backtick run in a value could close it.
fn fence_free(text: &str) -> Result<(), Why> {
    if text.contains("```") {
        return Err(Why::Fence);
    }
    Ok(())
}

/// Checks as the model carries them minus the cases (`## Tests` fences bind those, not config).
fn without_cases(cs: &[ProjectCheck]) -> Vec<Value> {
    cs.iter()
        .map(|c| {
            let mut v = serde_json::to_value(c).unwrap_or(Value::Null);
            if let Some(o) = v.as_object_mut() {
                o.remove("cases");
                o.remove("publicCount");
            }
            v
        })
        .collect()
}

fn check_lines(c: &ProjectCheck, out: &mut String) -> Result<(), Why> {
    let (kind, name, mut fields): (&str, &str, Vec<(&str, String)>) = match c {
        ProjectCheck::Call(c) => (
            "call",
            &c.base.name,
            vec![("file", q(&c.file)?), ("function", q(&c.function)?)],
        ),
        ProjectCheck::Build(c) => {
            let mut f = vec![("argv", json_array(&c.argv)?)];
            if let Some(d) = &c.dir {
                f.push(("dir", q(d)?));
            }
            ("build", &c.base.name, f)
        }
        ProjectCheck::DomAssert(c) => ("dom-assert", &c.base.name, file_pkg(&c.file, &c.package)?),
        ProjectCheck::CssAssert(c) => ("css-assert", &c.base.name, file_pkg(&c.file, &c.package)?),
        ProjectCheck::Http(c) => ("http", &c.base.name, vec![("package", q(&c.package)?)]),
    };
    out.push_str(&format!("    - name: {}\n      kind: {kind}\n", q(name)?));
    for (k, v) in fields.drain(..) {
        out.push_str(&format!("      {k}: {v}\n"));
    }
    Ok(())
}

fn file_pkg<'a>(file: &str, package: &Option<String>) -> Result<Vec<(&'a str, String)>, Why> {
    let mut f = vec![("file", q(file)?)];
    if let Some(p) = package {
        f.push(("package", q(p)?));
    }
    Ok(f)
}

/// The `test:` group (header, scalars, `checks`), `None` when the value is the default.
///
/// # Examples
///
/// ```
/// let _ = mda_core::leetcode::emit::test_group(&mda_core::leetcode::LeetCodeExercise::default());
/// ```
pub fn test_group(x: &LeetCodeExercise) -> Group {
    let d = TestConfig::default();
    let mut body = String::new();
    if x.test.r#type != d.r#type {
        body.push_str(&format!("  type: {}\n", wire(&x.test.r#type)?));
    }
    if x.test.timeout_ms != d.timeout_ms {
        body.push_str(&format!("  timeoutMs: {}\n", x.test.timeout_ms));
    }
    let checks = x.checks.as_deref().unwrap_or(&[]);
    if !checks.is_empty() {
        body.push_str("  checks:\n");
        for c in checks {
            check_lines(c, &mut body)?;
        }
    }
    if body.is_empty() {
        return Ok(None);
    }
    let text = format!("test:\n{body}");
    fence_free(&text)?;
    let lines = lines_of(&text);
    let same = parse_test_block(&lines, 0).0.test == x.test
        && without_cases(&parse_checks(&lines, &mut vec![], &mut vec![])) == without_cases(checks);
    same.then_some(Some(text)).ok_or(Why::Other)
}

/// The `libs:` group, `None` when absent.
///
/// # Examples
///
/// ```
/// let _ = mda_core::leetcode::emit::libs_group(&mda_core::leetcode::LeetCodeExercise::default());
/// ```
pub fn libs_group(x: &LeetCodeExercise) -> Group {
    let Some(libs) = x.libs.as_ref().filter(|l| !l.0.is_empty()) else {
        return Ok(None);
    };
    let mut text = String::from("libs:\n");
    for (lang, specs) in &libs.0 {
        word(lang)?;
        text.push_str(&format!("  {lang}:\n"));
        for s in specs {
            text.push_str(&format!("    - {}\n", q(s)?));
        }
    }
    fence_free(&text)?;
    let got = parse_lib_declarations(&lines_of(&text), &mut vec![]);
    (got == *libs).then_some(Some(text)).ok_or(Why::Other)
}

/// A key written bare: no whitespace, colon or backtick that could end it or open another line.
fn word(s: &str) -> Result<(), Why> {
    if s.is_empty() || s.contains(|c: char| c.is_whitespace() || c == ':' || c == '`') {
        return Err(Why::Other);
    }
    Ok(())
}

/// The `packages:` group, `None` when absent.
///
/// # Examples
///
/// ```
/// let _ = mda_core::leetcode::emit::packages_group(&mda_core::leetcode::LeetCodeExercise::default());
/// ```
pub fn packages_group(x: &LeetCodeExercise) -> Group {
    let Some(pkgs) = x.packages.as_deref().filter(|p| !p.is_empty()) else {
        return Ok(None);
    };
    let mut t = String::from("packages:\n");
    for p in pkgs {
        t.push_str(&format!(
            "  - name: {}\n    dir: {}\n    install: {}\n    start: {}\n",
            q(&p.name)?,
            q(&p.dir)?,
            json_array(&p.install)?,
            json_array(&p.start)?
        ));
        if let Some(r) = &p.ready {
            t.push_str(&format!("    ready: {}\n", q(r)?));
        }
        if let Some(d) = &p.depends_on {
            let names: Result<Vec<_>, _> = d.iter().map(|n| q(n)).collect();
            t.push_str(&format!("    dependsOn: [{}]\n", names?.join(", ")));
        }
        if let Some(e) = &p.expose_as {
            t.push_str("    exposeAs:\n");
            for (k, v) in &e.0 {
                word(k)?;
                t.push_str(&format!("      {k}: {}\n", q(v)?));
            }
        }
    }
    fence_free(&t)?;
    (parse_packages(&lines_of(&t), &mut vec![]) == pkgs)
        .then_some(Some(t))
        .ok_or(Why::Other)
}

/// The `program:` group, `None` when absent.
///
/// # Examples
///
/// ```
/// let _ = mda_core::leetcode::emit::program_group(&mda_core::leetcode::LeetCodeExercise::default());
/// ```
pub fn program_group(x: &LeetCodeExercise) -> Group {
    let Some(p) = &x.program else {
        return Ok(None);
    };
    let mut t = format!("program:\n  channel: {}\n", wire(&p.channel)?);
    if let Some(e) = &p.entry {
        t.push_str(&format!("  entry: {e}\n"));
    }
    if let Some(f) = &p.flags {
        t.push_str(&format!("  flags: [{}]\n", f.join(", ")));
    }
    fence_free(&t)?;
    let got = parse_program(&lines_of(&t), &mut vec![]);
    (got.as_ref() == Some(p))
        .then_some(Some(t))
        .ok_or(Why::Other)
}

/// One `## Files` path fence (```` ```<lang> path=… role=… ````).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{FileRole, FileSpec};
/// let f = FileSpec { path: "a.py".into(), language: "python".into(), role: FileRole::Editable, content: "x\n".into() };
/// assert_eq!(mda_core::leetcode::emit::file_fence(&f).unwrap(), "```python path=a.py\nx\n```\n");
/// ```
pub fn file_fence(f: &FileSpec) -> Result<String, Why> {
    // The reader takes everything up to the first backtick run, so the closer must follow a line
    // end the content already owns; content with no final line end is not safely carried (the
    // line-start fence toggle of section bounds and layout would see the fence still open).
    if f.content.contains("```") {
        return Err(Why::Fence);
    }
    if !f.content.is_empty() && !f.content.ends_with('\n') {
        return Err(Why::Other);
    }
    let path = super::quote::attr_quoted(&f.path).ok_or(Why::Other)?;
    let role = match f.role {
        FileRole::Editable => "",
        FileRole::Readonly => " role=readonly",
        FileRole::Hidden => " role=hidden",
    };
    let text = format!("```{} path={path}{role}\n{}```\n", f.language, f.content);
    let body = format!("## Files\n\n{text}");
    let back = parse_project_artifact("", &body).files;
    (back.len() == 1 && back.first() == Some(f))
        .then_some(text)
        .ok_or(Why::Other)
}
