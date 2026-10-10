//! The `vks` emitter (spec §9.5, D-7): YAML only, never the legacy `KEY=value` body (D-8), plus
//! [`check_vars`], the model-side semantic checks (§9.1 key grammar, §9.7 limits, C0) a client's
//! values pass before anything is emitted (W-10).

use std::collections::HashSet;

use super::legacy::is_control;
use super::yaml::{check_key, is_dash, line_construct};
use super::{
    MAX_DEPTH, MAX_LIST_ITEMS, MAX_MAP_KEYS, MAX_NODES, VksList, VksRecord, VksValue, limit_error,
};
use crate::error::{
    EmitReason, Limit, Unrepresentable, VKS_CONTROL_CHAR, VKS_DUPLICATE_KEY, VarsError,
};
use crate::model::ParsedVar;
use crate::parse::text::js_trim;

/// How one string is written; chosen by [`form`], the only place the §9.5 rules live.
enum Form {
    Empty,
    Plain,
    Single,
    Double,
    Block,
}

/// §9.5, first rule that applies. `item`: a list item, where a block scalar does not exist.
fn form(s: &str, item: bool) -> Result<Form, EmitReason> {
    if s.contains("```") {
        return Err(EmitReason::Backticks);
    }
    if s.ends_with("\n\n") {
        return Err(EmitReason::TrailingNewlines);
    }
    if s.is_empty() {
        return Ok(Form::Empty);
    }
    if s.contains('\n') {
        // The reader turns a whitespace-only line into "" and an all-newline string into an empty
        // block, so those go double-quoted, where every character survives.
        let blockable = !item
            && s.chars().any(|c| c != '\n')
            && s.split('\n')
                .all(|l| l.is_empty() || !js_trim(l).is_empty());
        return Ok(if blockable { Form::Block } else { Form::Double });
    }
    Ok(if fits_plain(s, item) && !special(s) {
        Form::Plain
    } else {
        Form::Single
    })
}

/// What `yaml::scalar` accepts and returns unchanged (plus the item checks for a list item).
fn fits_plain(s: &str, item: bool) -> bool {
    let bad_start = s.starts_with(['\'', '"', '#', ']', '}', '%', '@', '`', ',', '?'])
        || s.starts_with("- ")
        || s == "[]"
        || line_construct(s).is_some();
    let bad_edge = s.starts_with([' ', '\t']) || s.ends_with([' ', '\t']);
    let bad_inside = s.contains(": ") || s.contains(" #") || s.ends_with(':');
    let bad_item = item && (is_dash(s) || s.starts_with('#'));
    !(bad_start || bad_edge || bad_inside || bad_item)
}

/// YAML words and numerals a YAML reader would not take for a string; quoted to keep the file portable.
fn special(s: &str) -> bool {
    ["true", "false", "yes", "no", "on", "off", "null", "~"]
        .iter()
        .any(|w| s.eq_ignore_ascii_case(w))
        || s.parse::<f64>().is_ok()
}

fn single(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn double(s: &str) -> String {
    let mut out = String::from('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Emit the entry `key: value` at indent `n` (the key's column; a list item's first field is
/// positioned by the caller). Recursion depth is bounded by the value's own depth, which
/// [`check_vars`] limits and serde bounds on the wire.
fn entry(out: &mut String, n: usize, key: &str, v: &VksValue) -> Result<(), EmitReason> {
    let pad = " ".repeat(n);
    match v {
        VksValue::Str(s) => match form(s, false)? {
            Form::Empty => out.push_str(&format!("{pad}{key}:\n")),
            Form::Plain => out.push_str(&format!("{pad}{key}: {s}\n")),
            Form::Single => out.push_str(&format!("{pad}{key}: {}\n", single(s))),
            Form::Double => out.push_str(&format!("{pad}{key}: {}\n", double(s))),
            Form::Block => {
                let keep = s.ends_with('\n');
                out.push_str(&format!("{pad}{key}: {}\n", if keep { "|" } else { "|-" }));
                let text = s.strip_suffix('\n').unwrap_or(s);
                for l in text.split('\n') {
                    if !l.is_empty() {
                        out.push_str(&format!("{pad}  {l}"));
                    }
                    out.push('\n');
                }
            }
        },
        VksValue::List(l) if list_is_empty(l) => out.push_str(&format!("{pad}{key}: []\n")),
        VksValue::List(VksList::Strings(items)) => {
            out.push_str(&format!("{pad}{key}:\n"));
            for i in items {
                let text = match form(i, true)? {
                    Form::Empty => "''".to_owned(),
                    Form::Plain => i.clone(),
                    Form::Single => single(i),
                    Form::Double | Form::Block => double(i),
                };
                out.push_str(&format!("{pad}  - {text}\n"));
            }
        }
        VksValue::List(VksList::Records(recs)) => {
            out.push_str(&format!("{pad}{key}:\n"));
            for r in recs {
                // Compact record: the first field rides on the dash line, the rest align under it.
                let mut body = String::new();
                record(&mut body, n + 4, r)?;
                out.push_str(&format!("{pad}  - {}", body.get(n + 4..).unwrap_or("")));
            }
        }
        VksValue::Record(r) => {
            out.push_str(&format!("{pad}{key}:\n"));
            record(out, n + 2, r)?;
        }
    }
    Ok(())
}

fn list_is_empty(l: &VksList) -> bool {
    match l {
        VksList::Strings(v) => v.is_empty(),
        VksList::Records(v) => v.is_empty(),
    }
}

fn record(out: &mut String, n: usize, r: &VksRecord) -> Result<(), EmitReason> {
    if r.0.is_empty() {
        return Err(EmitReason::EmptyRecord);
    }
    r.0.iter().try_for_each(|(k, v)| entry(out, n, k, v))
}

/// Emit a ```` ```vks ```` fence body (without the fences): one entry per var, key order kept,
/// every line ending `\n`. Call [`check_vars`] first (the serializer does): this assumes valid
/// keys and in-limit values.
///
/// # Examples
///
/// ```
/// use mda_core::{model::ParsedVar, vks::emit_body};
/// assert_eq!(emit_body(&[ParsedVar::text("VK-a", "x")]).unwrap(), "VK-a: x\n");
/// ```
pub fn emit_body(vars: &[ParsedVar]) -> Result<String, Unrepresentable> {
    let mut out = String::new();
    for v in vars {
        entry(&mut out, 0, &v.name, &v.value).map_err(|reason| Unrepresentable {
            var: v.name.clone(),
            reason,
        })?;
    }
    Ok(out)
}

/// Check a model's vars before emission: top-level count, key grammar, duplicates, §9.7 limits
/// (each before the descent it bounds), control characters. Errors carry `line = 0` and `var`.
///
/// # Examples
///
/// ```
/// use mda_core::{model::ParsedVar, vks::check_vars};
/// assert!(check_vars(&[ParsedVar::text("VK-a", "x")]).is_ok());
/// assert!(check_vars(&[ParsedVar::text("a.b", "x")]).is_err());
/// ```
pub fn check_vars(vars: &[ParsedVar]) -> Result<(), VarsError> {
    if let Some(over) = vars.get(MAX_MAP_KEYS) {
        return Err(limit_error(Limit::MapKeys, MAX_MAP_KEYS, 0).with("var", &over.name));
    }
    let mut nodes = 0;
    let mut seen = HashSet::new();
    for v in vars {
        let at = |e: VarsError| e.with("var", &v.name);
        check_key(&v.name, true, 0).map_err(at)?;
        if !seen.insert(v.name.as_str()) {
            return Err(at(VarsError::new(VKS_DUPLICATE_KEY, 0).with("key", &v.name)));
        }
        walk(&v.value, 1, &mut nodes).map_err(at)?;
    }
    Ok(())
}

fn walk(v: &VksValue, depth: usize, nodes: &mut usize) -> Result<(), VarsError> {
    count(nodes)?;
    match v {
        VksValue::Str(s) => text(s),
        VksValue::List(l) => {
            depth_ok(depth)?;
            match l {
                VksList::Strings(items) => {
                    list_ok(items.len())?;
                    items
                        .iter()
                        .try_for_each(|i| count(nodes).and_then(|()| text(i)))
                }
                VksList::Records(recs) => {
                    list_ok(recs.len())?;
                    recs.iter().try_for_each(|r| {
                        count(nodes)?;
                        fields(r, depth + 1, nodes)
                    })
                }
            }
        }
        VksValue::Record(r) => fields(r, depth, nodes),
    }
}

fn fields(r: &VksRecord, depth: usize, nodes: &mut usize) -> Result<(), VarsError> {
    depth_ok(depth)?;
    if r.0.len() > MAX_MAP_KEYS {
        return Err(limit_error(Limit::MapKeys, MAX_MAP_KEYS, 0));
    }
    let mut seen = HashSet::new();
    for (k, v) in &r.0 {
        check_key(k, false, 0)?;
        if !seen.insert(k.as_str()) {
            return Err(VarsError::new(VKS_DUPLICATE_KEY, 0).with("key", k));
        }
        walk(v, depth + 1, nodes)?;
    }
    Ok(())
}

fn count(nodes: &mut usize) -> Result<(), VarsError> {
    if *nodes >= MAX_NODES {
        return Err(limit_error(Limit::Nodes, MAX_NODES, 0));
    }
    *nodes += 1;
    Ok(())
}

fn depth_ok(depth: usize) -> Result<(), VarsError> {
    if depth > MAX_DEPTH {
        return Err(limit_error(Limit::Depth, MAX_DEPTH, 0));
    }
    Ok(())
}

fn list_ok(len: usize) -> Result<(), VarsError> {
    if len > MAX_LIST_ITEMS {
        return Err(limit_error(Limit::ListItems, MAX_LIST_ITEMS, 0));
    }
    Ok(())
}

/// `\n` is content; every other control, and the JS line separators `js_lines` splits on, is not.
fn text(s: &str) -> Result<(), VarsError> {
    if s.chars()
        .any(|c| (is_control(c) && c != '\n') || c == '\u{2028}' || c == '\u{2029}')
    {
        return Err(VarsError::new(VKS_CONTROL_CHAR, 0));
    }
    Ok(())
}
