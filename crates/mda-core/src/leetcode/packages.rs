//! `packages:` config block (TS `packages-parser.helpers.ts`, W4 T4.4; spec §9.3): a stack's boot
//! list. Argv arrays only, `${PORT}` the sole substitution, `exposeAs` names and values validated.
//! The hard errors (cap, duplicate name, cycle) drop the whole block with a warning (W34-W36).

use std::collections::{HashMap, HashSet};

use super::{
    OrderedMap, PackageSpec,
    config::{block_lines, indent_of, is_ws, kv_line, unquote_pair},
    libs::LIB_SEAM_VARS,
    program::{unsafe_path_reason, warning},
    safe_json::safe_json,
};
use crate::parse::text::js_trim;
use crate::render::Warning;

#[cfg(test)]
mod tests;

/// Hard cap on `packages:` entries (S7): bounds concurrent processes and installs.
pub(crate) const MAX_PACKAGES: usize = 8;

/// Loader/interpreter variables an `exposeAs` name must never be; `DYLD_*` is checked separately.
const EXPOSE_DENYLIST: &[&str] = &[
    "PORT",
    "PATH",
    "NODE_OPTIONS",
    "NODE_EXTRA_CA_CERTS",
    "NODE_REPL_EXTERNAL_MODULE",
    "PYTHONPATH",
    "PYTHONSTARTUP",
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "LD_AUDIT",
    "JAVA_TOOL_OPTIONS",
    "JDK_JAVA_OPTIONS",
    "RUSTFLAGS",
];

/// `^[A-Z][A-Z0-9_]*$`: structurally excludes `__proto__`.
fn expose_name_ok(n: &str) -> bool {
    let mut cs = n.chars();
    cs.next().is_some_and(|c| c.is_ascii_uppercase())
        && cs.all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

fn is_reserved_expose_name(n: &str) -> bool {
    EXPOSE_DENYLIST.contains(&n) || LIB_SEAM_VARS.contains(&n) || n.starts_with("DYLD_")
}

/// Whether `name` may be set as a booted child's environment variable (TS `isSafeExposeName`).
#[cfg(test)] // TS runtime-only (install/boot); the parse path never calls it
pub(crate) fn is_safe_expose_name(name: &str) -> bool {
    expose_name_ok(name) && !is_reserved_expose_name(name)
}

/// `^http://127.0.0.1:${PORT}(/\S*)?$`: a loopback URL, `${PORT}` its only variable part.
fn expose_value_ok(v: &str) -> bool {
    v.strip_prefix("http://127.0.0.1:${PORT}")
        .is_some_and(|rest| rest.is_empty() || (rest.starts_with('/') && !rest.chars().any(is_ws)))
}

/// Every `${...}` in `v` is exactly `${PORT}` (TS `/\$\{([^}]*)\}/g`).
fn only_port_placeholder(v: &str) -> bool {
    let mut rest = v;
    while let Some(i) = rest.find("${") {
        let after = &rest[i + 2..];
        let Some(j) = after.find('}') else {
            return true;
        };
        if &after[..j] != "PORT" {
            return false;
        }
        rest = &after[j + 1..];
    }
    true
}

/// Parse the `packages:` block of merged config lines; empty when absent or dropped.
pub(crate) fn parse_packages(lines: &[&str], warnings: &mut Vec<Warning>) -> Vec<PackageSpec> {
    let Some((start, header)) = lines
        .iter()
        .enumerate()
        .find(|(_, l)| js_trim(l).starts_with("packages:"))
    else {
        return vec![];
    };
    let got = js_trim(js_trim(header).trim_start_matches("packages:"));
    if !got.is_empty() {
        warnings.push(warning("leetcode.packages.not_a_list", &[("got", got)]));
        return vec![];
    }
    let groups = split_entries(&block_lines(lines, start));
    if groups.len() > MAX_PACKAGES {
        let (count, max) = (groups.len().to_string(), MAX_PACKAGES.to_string());
        warnings.push(warning(
            "leetcode.packages.too_many",
            &[("count", &count), ("max", &max)],
        ));
        return vec![];
    }
    let packages: Vec<PackageSpec> = groups
        .iter()
        .filter_map(|g| build_package(g, warnings))
        .collect();
    let mut seen: Vec<&str> = vec![];
    if let Some(name) = packages.iter().map(|p| p.name.as_str()).find(|n| {
        let dup = seen.contains(n);
        seen.push(n);
        dup
    }) {
        warnings.push(warning(
            "leetcode.packages.duplicate_name",
            &[("name", name)],
        ));
        return vec![];
    }
    if let Some(cycle) = find_cycle(&packages) {
        warnings.push(warning(
            "leetcode.packages.cycle",
            &[("path", &cycle.join(", "))],
        ));
        return vec![];
    }
    packages
}

/// One raw-line group per `- ` entry, at the block's own (smallest) dash indent.
fn split_entries<'a>(lines: &[&'a str]) -> Vec<Vec<&'a str>> {
    let is_dash = |l: &str| js_trim(l).starts_with("- ");
    let Some(entry_indent) = lines
        .iter()
        .filter(|l| is_dash(l))
        .map(|l| indent_of(l))
        .min()
    else {
        return vec![];
    };
    let mut groups: Vec<Vec<&str>> = vec![];
    for line in lines {
        if indent_of(line) == entry_indent && is_dash(line) {
            groups.push(vec![line]);
        } else if let Some(g) = groups.last_mut() {
            g.push(line);
        }
    }
    groups
}

/// TS `first.replace(/^(\s*)-\s/, '$1  ')`: the dash becomes space, the indent before it stays.
fn blank_dash(first: &str) -> String {
    let body = first.trim_start_matches(is_ws);
    let indent = &first[..first.len() - body.len()];
    match body.strip_prefix('-').and_then(|r| {
        let mut cs = r.chars();
        cs.next().filter(|&c| is_ws(c)).map(|c| &r[c.len_utf8()..])
    }) {
        Some(rest) => format!("{indent}  {rest}"),
        None => first.to_owned(),
    }
}

/// Flat top-level fields of one entry, plus its validated `exposeAs` pairs.
fn parse_entry_lines(
    group: &[&str],
    warnings: &mut Vec<Warning>,
) -> (HashMap<String, String>, Vec<(String, String)>) {
    let flat: Vec<String> = group
        .iter()
        .enumerate()
        .map(|(i, l)| {
            if i == 0 {
                blank_dash(l)
            } else {
                (*l).to_owned()
            }
        })
        .collect();
    let mut expose = vec![];
    let mut kept: Vec<&str> = vec![];
    let mut i = 0;
    while let Some(line) = flat.get(i).map(String::as_str) {
        let is_header = js_trim(line)
            .strip_prefix("exposeAs:")
            .is_some_and(|r| js_trim(r).is_empty());
        if !is_header {
            kept.push(line);
            i += 1;
            continue;
        }
        let header_indent = indent_of(line);
        while let Some(next) = flat.get(i + 1).filter(|l| indent_of(l) > header_indent) {
            i += 1;
            read_expose_entry(next, warnings, &mut expose);
        }
        i += 1;
    }
    let mut fields = HashMap::new();
    for line in kept {
        if let Some((k, v)) = kv_line(js_trim(line))
            && matches!(
                k,
                "name" | "dir" | "install" | "start" | "ready" | "dependsOn"
            )
        {
            fields.insert(k.to_owned(), js_trim(v).to_owned());
        }
    }
    (fields, expose)
}

/// One `NAME: "value"` line of an `exposeAs:` block, dropped by name when unsafe (S9, SEC2).
fn read_expose_entry(line: &str, warnings: &mut Vec<Warning>, out: &mut Vec<(String, String)>) {
    let Some((key, val)) = kv_line(js_trim(line)) else {
        return;
    };
    let value = unquote_pair(js_trim(val));
    if !expose_name_ok(key) {
        warnings.push(warning(
            "leetcode.packages.expose_name_invalid",
            &[("name", key)],
        ));
    } else if is_reserved_expose_name(key) {
        warnings.push(warning(
            "leetcode.packages.expose_name_reserved",
            &[("name", key)],
        ));
    } else if !expose_value_ok(value) {
        warnings.push(warning(
            "leetcode.packages.expose_value_not_loopback",
            &[("name", key), ("value", value)],
        ));
    } else {
        out.push((key.to_owned(), value.to_owned()));
    }
}

fn build_package(group: &[&str], warnings: &mut Vec<Warning>) -> Option<PackageSpec> {
    let (fields, expose) = parse_entry_lines(group, warnings);
    let get = |k: &str| fields.get(k).map(String::as_str);
    let name = unquote_pair(get("name").unwrap_or(""));
    if name.is_empty() {
        warnings.push(warning("leetcode.packages.missing_name", &[]));
        return None;
    }
    let dir = unquote_pair(get("dir").unwrap_or(""));
    if let Some(reason) = unsafe_path_reason(dir) {
        warnings.push(warning(
            "leetcode.packages.dir_rejected",
            &[("name", name), ("dir", dir), ("reason", reason)],
        ));
        return None;
    }
    let install = parse_argv(get("install"), name, "install", warnings)?;
    let start = parse_argv(get("start"), name, "start", warnings)?;
    let depends_on = inline_names(get("dependsOn").unwrap_or(""));
    Some(PackageSpec {
        name: name.to_owned(),
        dir: dir.to_owned(),
        install,
        start,
        ready: get("ready").map(|r| unquote_pair(r).to_owned()),
        expose_as: (!expose.is_empty()).then(|| OrderedMap::from_pairs(expose)),
        depends_on: (!depends_on.is_empty()).then_some(depends_on),
    })
}

/// An `install:` / `start:` value: a JSON array of strings, nothing else (S5).
fn parse_argv(
    raw: Option<&str>,
    name: &str,
    field: &str,
    warnings: &mut Vec<Warning>,
) -> Option<Vec<String>> {
    let both = [("name", name), ("field", field)];
    let Some(raw) = raw else {
        warnings.push(warning("leetcode.packages.missing_field", &both));
        return None;
    };
    let argv: Option<Vec<String>> = safe_json(raw)
        .and_then(|v| v.as_array().cloned())
        .and_then(|a| a.iter().map(|e| e.as_str().map(str::to_owned)).collect());
    let Some(argv) = argv else {
        warnings.push(warning("leetcode.packages.argv_not_array", &both));
        return None;
    };
    if argv.is_empty() {
        warnings.push(warning("leetcode.packages.argv_empty", &both));
        return None;
    }
    if let Some(bad) = argv.iter().find(|e| !only_port_placeholder(e)) {
        warnings.push(warning(
            "leetcode.packages.argv_substitution",
            &[("name", name), ("field", field), ("element", bad)],
        ));
        return None;
    }
    Some(argv)
}

/// `dependsOn: [api, cache]` -> names; anything else -> `[]`.
fn inline_names(raw: &str) -> Vec<String> {
    let v = js_trim(raw);
    let Some(inner) = v.strip_prefix('[') else {
        return vec![];
    };
    let inner = inner.strip_suffix(']').unwrap_or(inner);
    inner
        .split(',')
        .map(|p| unquote_pair(p).to_owned())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Depth-first cycle search over `dependsOn`; names are unique here and at most [`MAX_PACKAGES`].
fn find_cycle(packages: &[PackageSpec]) -> Option<Vec<String>> {
    fn visit<'a>(
        name: &'a str,
        deps: &HashMap<&'a str, &'a [String]>,
        done: &mut HashSet<&'a str>,
        chain: &mut Vec<&'a str>,
    ) -> Option<Vec<String>> {
        if done.contains(name) {
            return None;
        }
        if let Some(at) = chain.iter().position(|c| *c == name) {
            let mut c: Vec<String> = chain
                .get(at..)
                .unwrap_or(&[])
                .iter()
                .map(|s| (*s).to_owned())
                .collect();
            c.push(name.to_owned());
            return Some(c);
        }
        chain.push(name);
        for dep in deps.get(name).copied().unwrap_or(&[]) {
            if let Some(found) = visit(dep, deps, done, chain) {
                return Some(found);
            }
        }
        chain.pop();
        done.insert(name);
        None
    }
    let deps: HashMap<&str, &[String]> = packages
        .iter()
        .map(|p| (p.name.as_str(), p.depends_on.as_deref().unwrap_or(&[])))
        .collect();
    let (mut done, mut chain) = (HashSet::new(), vec![]);
    packages
        .iter()
        .find_map(|p| visit(&p.name, &deps, &mut done, &mut chain))
}
