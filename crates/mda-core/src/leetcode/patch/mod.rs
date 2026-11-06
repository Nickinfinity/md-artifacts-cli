//! Pure, byte-surgical outcome patches of an exercise file (W6 D-27; TS `frontmatter-patcher`,
//! `attempts-writer`, `insertSolvedMeta`). Every function takes the decoded text and returns the
//! new text or a refusal; nothing here touches I/O, so an `Err` can never reach a writer. Inserted
//! text uses the file's own EOL.

use regex::Regex;

use super::bounds::{SectionBounds, section_bounds};
use super::config::{CANONICAL_ORDER, top_level_key};
use super::{AttemptEntry, LeetCodeStatus, resolve_lang_id};
use crate::error::{BadField, NotSerializableReason as Why};
use crate::migrate::frontmatter::lines;
use crate::parse::body_range;
use crate::parse::text::{compile, js_trim, js_trim_end};

#[cfg(test)]
mod tests;

/// The file's EOL: CRLF when its first line ends so, else LF.
fn eol_of(raw: &str) -> &'static str {
    match raw.find('\n') {
        Some(i) if raw.get(..i).is_some_and(|h| h.ends_with('\r')) => "\r\n",
        _ => "\n",
    }
}

/// `(heading, boundary)` section search: `# <name>` line to the next top-level `# ` line.
fn section(raw: &str, name: &str) -> Option<SectionBounds> {
    let heading: Regex = compile(&format!(r"(?m)^# {name}[ \t]*\r?$"));
    section_bounds(raw, &heading, &|l: &str| l.starts_with("# "))
}

/// Replace `status:` in place (every occurrence, as TS `gm`), or insert it at its canonical index
/// (before the first top-level key that outranks it, else at the end of the frontmatter). Every
/// other byte is kept. No closed frontmatter block: `Err(Frontmatter)`.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{LeetCodeStatus, patch_status};
/// let raw = "---\ndifficulty: easy\ntags: [a]\n---\n";
/// assert_eq!(
///     patch_status(raw, LeetCodeStatus::Solved).unwrap(),
///     "---\ndifficulty: easy\nstatus: solved\ntags: [a]\n---\n"
/// );
/// ```
pub fn patch_status(raw: &str, status: LeetCodeStatus) -> Result<String, Why> {
    let eol = eol_of(raw);
    let line = match status {
        LeetCodeStatus::Unsolved => "status: unsolved",
        LeetCodeStatus::Attempted => "status: attempted",
        LeetCodeStatus::Solved => "status: solved",
    };
    // `---\n---` is no block for the parser, but TS writes the line into it.
    let opener = format!("---{eol}");
    // The closer must be exactly `---` (TS `/^---$/m`): `----`, `---x`, `--- ` fall through.
    if let Some(rest) = raw.strip_prefix(opener.as_str()).filter(|r| {
        r.strip_prefix("---")
            .is_some_and(|t| t.is_empty() || t.starts_with(['\n', '\r']))
    }) {
        return Ok(format!("{opener}{line}{eol}{rest}"));
    }
    let range = body_range(raw).ok_or(Why::Frontmatter)?;
    let fm = lines(raw, range.clone());
    let mut out = raw.to_owned();
    let present: Vec<_> = fm
        .iter()
        .filter(|(_, l)| top_level_key(l) == Some("status"))
        .collect();
    if !present.is_empty() {
        // Back to front so earlier offsets stay valid.
        for (at, l) in present.into_iter().rev() {
            out.replace_range(*at..*at + l.len(), line);
        }
        return Ok(out);
    }
    let target = CANONICAL_ORDER.iter().position(|k| *k == "status");
    let before = fm.iter().find(|(_, l)| {
        let rank = top_level_key(l).and_then(|k| CANONICAL_ORDER.iter().position(|c| *c == k));
        rank > target
    });
    match before {
        Some((at, _)) => out.insert_str(*at, &format!("{line}{eol}")),
        None => out.insert_str(range.end, &format!("{eol}{line}")),
    }
    Ok(out)
}

/// A `<!-- meta|attempt: … -->` payload string must not close the comment.
fn comment_safe(parts: &[&str], why: Why) -> Result<(), Why> {
    if parts.iter().any(|p| p.contains("-->")) {
        Err(why)
    } else {
        Ok(())
    }
}

/// `json` of a string (escaped by serde, so `"` and `\` cannot break the object).
fn js(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_default()
}

/// The fence-opening lines of `body` as `(absolute offset, line without EOL)`, skipping lines
/// inside a fence (a plain ``` toggle, as `boundary_outside_fence`).
fn fence_openers<'a>(raw: &'a str, b: &SectionBounds) -> Vec<(usize, &'a str)> {
    let mut fenced = false;
    let mut out = vec![];
    for (at, l) in lines(raw, b.heading_end..b.body_end) {
        if l.starts_with("```") {
            if !fenced {
                out.push((at, l));
            }
            fenced = !fenced;
        }
    }
    out
}

/// Write `<!-- meta: { "solved_at": …, "duration": … } -->` above the language's first bare
/// fence inside `# Solutions`, replacing a meta line already directly above it (Q-56). An empty
/// `duration` writes nothing (as TS). No such fence: `Err(Meta)`; `-->` in a value: `Err(Meta)`.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::insert_solved_meta;
/// let raw = "# Solutions\n```python\nx\n```\n";
/// let out = insert_solved_meta(raw, "py", "2026-11-05T10:00:00Z", "5m").unwrap();
/// assert_eq!(out, "# Solutions\n<!-- meta: { \"solved_at\": \"2026-11-05T10:00:00Z\", \"duration\": \"5m\" } -->\n```python\nx\n```\n");
/// ```
pub fn insert_solved_meta(
    raw: &str,
    lang: &str,
    solved_at: &str,
    duration: &str,
) -> Result<String, Why> {
    if duration.is_empty() {
        return Ok(raw.to_owned());
    }
    comment_safe(&[solved_at, duration], Why::Meta)?;
    let lang = validate_language(lang).map_err(|_| Why::Other)?;
    let b = section(raw, "Solutions").ok_or(Why::Meta)?;
    let bare = |l: &str| {
        l.strip_prefix("```").is_some_and(|i| {
            !i.is_empty() && !i.contains(char::is_whitespace) && resolve_lang_id(i) == lang
        })
    };
    let (at, _) = fence_openers(raw, &b)
        .into_iter()
        .find(|(_, l)| bare(l))
        .ok_or(Why::Meta)?;
    let meta = format!(
        "<!-- meta: {{ \"solved_at\": {}, \"duration\": {} }} -->",
        js(solved_at),
        js(duration)
    );
    let eol = eol_of(raw);
    // A meta line directly above the fence (inside the section) is replaced, never stacked.
    let prev = lines(raw, b.heading_end..at).pop().filter(|(_, l)| {
        let l = js_trim(l);
        l.starts_with("<!-- meta:") && l.ends_with("-->")
    });
    let mut out = raw.to_owned();
    match prev {
        Some((p, l)) => out.replace_range(p..p + l.len(), &meta),
        None => out.insert_str(at, &format!("{meta}{eol}")),
    }
    Ok(out)
}

/// Blank line before `text` unless `before` ends in one (or is empty); a newline after it unless
/// `after` starts with one (or is empty).
fn splice_in(raw: &str, at: usize, text: &str, eol: &str) -> String {
    let (before, after) = raw.split_at(at);
    let lead = if before.is_empty() || before.ends_with("\n\n") || before.ends_with("\n\r\n") {
        ""
    } else if before.ends_with('\n') {
        eol
    } else {
        &format!("{eol}{eol}")
    };
    let trail = if after.is_empty() || after.starts_with(['\n', '\r']) {
        ""
    } else {
        eol
    };
    format!("{before}{lead}{text}{trail}{after}")
}

/// `(start, entries_start, canonical language)` of each `## <Language>` line of a section body,
/// skipping lines inside a fence (deviates from TS, which would split an attempt's code).
fn lang_headings(body: &str) -> Vec<(usize, usize, String)> {
    let (mut fenced, mut at, mut out) = (false, 0, vec![]);
    for l in body.split_inclusive('\n') {
        let text = l
            .strip_suffix('\n')
            .map(|t| t.strip_suffix('\r').unwrap_or(t));
        if l.starts_with("```") {
            fenced = !fenced;
        } else if let (false, Some(t)) = (fenced, text)
            && let Some(name) = t.strip_prefix("## ").filter(|n| !n.is_empty())
        {
            out.push((at, at + l.len(), resolve_lang_id(js_trim(name))));
        }
        at += l.len();
    }
    out
}

/// Prepend one attempt (newest first) under `# Attempts` / `## <Language>`, creating either when
/// absent (`# Attempts` after `# Solutions`, else at the end). Code holding a ``` run: `Err(Fence)`;
/// `-->` in a comment field: `Err(Meta)`.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{AttemptEntry, prepend_attempt};
/// let a = AttemptEntry { at: "t".into(), duration: "1m".into(), passed: true, big_o: None,
///     confidence: None, code: "x".into() };
/// let out = prepend_attempt("# Solutions\n", "python", &a).unwrap();
/// assert_eq!(out, "# Solutions\n\n# Attempts\n\n## python\n<!-- attempt: {\"at\":\"t\",\"duration\":\"1m\",\"passed\":true} -->\n```python\nx\n```\n");
/// ```
pub fn prepend_attempt(raw: &str, lang: &str, a: &AttemptEntry) -> Result<String, Why> {
    if a.code.contains("```") {
        return Err(Why::Fence);
    }
    let opt = |o: &Option<String>| o.as_deref().unwrap_or("").to_owned();
    comment_safe(
        &[&a.at, &a.duration, &opt(&a.big_o), &opt(&a.confidence)],
        Why::Meta,
    )?;
    let lang = validate_language(lang).map_err(|_| Why::Other)?;
    let eol = eol_of(raw);
    let mut meta = format!(
        "{{\"at\":{},\"duration\":{},\"passed\":{}",
        js(&a.at),
        js(&a.duration),
        a.passed
    );
    if let Some(v) = &a.big_o {
        meta += &format!(",\"bigO\":{}", js(v));
    }
    if let Some(v) = &a.confidence {
        meta += &format!(",\"confidence\":{}", js(v));
    }
    let code = js_trim_end(&a.code)
        .replace("\r\n", "\n")
        .replace('\n', eol);
    let block = format!("<!-- attempt: {meta}}} -->{eol}```{lang}{eol}{code}{eol}```{eol}");

    let Some(sec) = section(raw, "Attempts") else {
        let at = section(raw, "Solutions").map_or(raw.len(), |s| s.body_end);
        return Ok(splice_in(
            raw,
            at,
            &format!("# Attempts{eol}{eol}## {lang}{eol}{block}"),
            eol,
        ));
    };
    let body = raw.get(sec.heading_end..sec.body_end).ok_or(Why::Other)?;
    let heads = lang_headings(body);
    let Some(i) = heads.iter().position(|h| h.2 == lang) else {
        return Ok(splice_in(
            raw,
            sec.body_end,
            &format!("## {lang}{eol}{block}"),
            eol,
        ));
    };
    let end = heads.get(i + 1).map_or(body.len(), |h| h.0);
    let entries = heads.get(i).ok_or(Why::Other)?.1;
    let glue = if js_trim(body.get(entries..end).unwrap_or("")).is_empty() {
        ""
    } else {
        eol
    };
    let mut out = raw.to_owned();
    out.insert_str(sec.heading_end + entries, &format!("{block}{glue}"));
    Ok(out)
}

/// The request `language`, resolved, if it is `[A-Za-z0-9_]{1,32}` (Q-57): a newline, backtick or
/// `#` would become a heading or fence info.
///
/// # Examples
///
/// ```
/// use mda_core::{error::BadField, leetcode::validate_language};
/// assert_eq!(validate_language("py").unwrap(), "python");
/// assert_eq!(validate_language("c++").unwrap(), "cpp");
/// assert_eq!(validate_language("a\n# Setup").unwrap_err(), BadField::Language);
/// ```
pub fn validate_language(lang: &str) -> Result<String, BadField> {
    let id = resolve_lang_id(lang);
    let ok =
        (1..=32).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if ok { Ok(id) } else { Err(BadField::Language) }
}
