//! Library spec grammars (TS `services/libs/lib-spec.helpers.ts`, `lib-ecosystem.ts`, W4 T4.6).
//! `lib-env.helpers` is not ported (runtime env code): only its seam variable names are pinned.

use super::resolve_lang_id;

#[cfg(test)]
mod tests;

/// Longest raw spec, in chars (Rust-only cap, Q-37/Q-39); longer is `leetcode.libs.rejected` upstream.
#[allow(dead_code)] // reason: seam, used by T4.3
pub(crate) const MAX_SPEC_CHARS: usize = 10_000;

/// Registries a `libs:` key can resolve to (TS `LIB_ECOSYSTEMS`).
#[allow(dead_code)] // reason: seam, used by T4.3
pub(crate) const LIB_ECOSYSTEMS: [&str; 4] = ["pnpm", "pip", "cargo", "maven"];

/// Env vars the library seam exports (TS `packages-parser.ts:114`); pinned so a TS change cannot drift silently.
#[allow(dead_code)] // reason: seam, used by T4.3
pub(crate) const LIB_SEAM_VARS: [&str; 4] =
    ["NODE_PATH", "VIRTUAL_ENV", "CLASSPATH", "CARGO_TARGET_DIR"];

/// Which toolchain resolves a `libs:` key.
#[allow(dead_code)] // reason: seam, used by T4.3 / T4.4
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LibEcosystem {
    Pnpm,
    Pip,
    Cargo,
    Maven,
}

/// One validated spec, split into the fields its manifest is rendered from.
#[allow(dead_code)] // reason: seam, used by T4.3 / T4.4
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ParsedLibSpec {
    Pnpm {
        name: String,
        range: Option<String>,
    },
    Pip {
        name: String,
        extras: Vec<String>,
        predicates: Vec<String>,
    },
    Cargo {
        name: String,
        req: Option<String>,
        features: Vec<String>,
    },
    Maven {
        group_id: String,
        artifact_id: String,
        version: String,
        packaging: Option<String>,
        classifier: Option<String>,
    },
}

/// Why a spec was refused. Codes only: the W13 warning carries `specs` + `ecosystem`, not this.
#[allow(dead_code)] // reason: seam, used by T4.3 / T4.4
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SpecRefusal {
    /// Over [`MAX_SPEC_CHARS`] chars.
    TooLong,
    /// Contains `..`.
    Traversal,
    /// Package / crate / requirement name is not a plain identifier.
    Name,
    /// npm range is not plain.
    Range,
    /// pip `[` without `]`.
    UnterminatedExtras,
    /// pip extra is not a plain identifier.
    Extra,
    /// pip predicate outside the grammar.
    Predicate,
    /// cargo requirement is not plain.
    Req,
    /// cargo feature is not a plain identifier.
    Feature,
    /// maven coordinate has the wrong number of segments.
    Coordinate,
    /// maven segment is not a plain identifier.
    Segment,
    /// maven `LATEST` / `RELEASE`.
    Floating,
}

type Parsed = Result<ParsedLibSpec, SpecRefusal>;

/// The registry serving a `libs:` language key: alias-resolved, display ids folded onto their runnable pair.
#[allow(dead_code)] // reason: seam, used by T4.3
pub(crate) fn ecosystem_for(language_key: &str) -> Option<LibEcosystem> {
    match resolve_lang_id(language_key).as_str() {
        "java" => Some(LibEcosystem::Maven),
        "python" => Some(LibEcosystem::Pip),
        "javascript" | "javascriptreact" | "typescript" | "typescriptreact" => {
            Some(LibEcosystem::Pnpm)
        }
        "rust" => Some(LibEcosystem::Cargo),
        _ => None,
    }
}

#[allow(dead_code)] // reason: seam, used by T4.3
fn all(s: &str, ok: impl Fn(char) -> bool) -> bool {
    s.chars().all(ok)
}

/// `^[A-Za-z0-9][A-Za-z0-9<rest>]*$` — ASCII only, so a leading `-`, `.` or `/` never matches.
#[allow(dead_code)] // reason: seam, used by T4.3
fn ident(s: &str, rest: &str) -> bool {
    let mut it = s.chars();
    it.next().is_some_and(|c| c.is_ascii_alphanumeric())
        && all(it.as_str(), |c| {
            c.is_ascii_alphanumeric() || rest.contains(c)
        })
}

#[allow(dead_code)] // reason: seam, used by T4.3
fn pnpm(raw: &str) -> Parsed {
    let (name, range) = match raw.rsplit_once('@') {
        Some((n, r)) if !n.is_empty() => (n, Some(r)),
        _ => (raw, None),
    };
    if !ident(name.strip_prefix('@').unwrap_or(name), "._/-") {
        return Err(SpecRefusal::Name);
    }
    if let Some(r) = range
        && (r.is_empty() || !all(r, |c| c.is_ascii_alphanumeric() || ".^~*+-".contains(c)))
    {
        return Err(SpecRefusal::Range);
    }
    Ok(ParsedLibSpec::Pnpm {
        name: name.to_owned(),
        range: range.map(str::to_owned),
    })
}

#[allow(dead_code)] // reason: seam, used by T4.3
fn pip_pred(p: &str) -> bool {
    let Some(v) = ["==", ">=", "<=", "~=", "!=", "<", ">"]
        .iter()
        .find_map(|o| p.strip_prefix(*o))
    else {
        return false;
    };
    v.chars().next().is_some_and(|c| c.is_ascii_digit())
        && all(v, |c| c.is_ascii_alphanumeric() || ".*!-".contains(c))
}

#[allow(dead_code)] // reason: seam, used by T4.3
fn pip(raw: &str) -> Parsed {
    let (head, extras, rest) = match raw.split_once('[') {
        None => (raw, "", ""),
        Some((head, after)) => match after.split_once(']') {
            None => return Err(SpecRefusal::UnterminatedExtras),
            Some((extras, rest)) => (head, extras, rest),
        },
    };
    let (name, head_preds) = head
        .find(['<', '>', '=', '!', '~'])
        .map_or((head, ""), |i| head.split_at(i));
    if !ident(name, "._-") {
        return Err(SpecRefusal::Name);
    }
    let split = |t: &str| -> Vec<String> {
        if t.is_empty() {
            vec![]
        } else {
            t.split(',').map(str::to_owned).collect()
        }
    };
    let extras = split(extras);
    if extras.iter().any(|e| !ident(e, "._-")) {
        return Err(SpecRefusal::Extra);
    }
    let predicates = split(&format!("{head_preds}{rest}"));
    if predicates.iter().any(|p| !pip_pred(p)) {
        return Err(SpecRefusal::Predicate);
    }
    Ok(ParsedLibSpec::Pip {
        name: name.to_owned(),
        extras,
        predicates,
    })
}

#[allow(dead_code)] // reason: seam, used by T4.3
fn cargo_req(r: &str) -> bool {
    let lead = r
        .chars()
        .take(2)
        .take_while(|c| "^~<>=".contains(*c))
        .count();
    let (_, v) = r.split_at(lead); // lead chars are ASCII
    v.chars().next().is_some_and(|c| c.is_ascii_digit())
        && all(v, |c| c.is_ascii_alphanumeric() || ".*-".contains(c))
}

#[allow(dead_code)] // reason: seam, used by T4.3
fn cargo_feature(f: &str) -> bool {
    let mut it = f.chars();
    it.next()
        .is_some_and(|c| matches!(c, 'a'..='z' | '0'..='9'))
        && all(
            it.as_str(),
            |c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '-'),
        )
}

#[allow(dead_code)] // reason: seam, used by T4.3
fn cargo(raw: &str) -> Parsed {
    let (name, tail) = match raw.split_once('@') {
        Some((n, t)) => (n, Some(t)),
        None => (raw, None),
    };
    if !ident(name, "_-") {
        return Err(SpecRefusal::Name);
    }
    let Some(tail) = tail else {
        return Ok(ParsedLibSpec::Cargo {
            name: name.to_owned(),
            req: None,
            features: vec![],
        });
    };
    let mut parts = tail.split('+');
    let req = parts.next().unwrap_or_default();
    if !cargo_req(req) {
        return Err(SpecRefusal::Req);
    }
    let features: Vec<String> = parts.map(str::to_owned).collect();
    if features.iter().any(|f| !cargo_feature(f)) {
        return Err(SpecRefusal::Feature);
    }
    Ok(ParsedLibSpec::Cargo {
        name: name.to_owned(),
        req: Some(req.to_owned()),
        features,
    })
}

#[allow(dead_code)] // reason: seam, used by T4.3
fn maven(raw: &str) -> Parsed {
    let parts: Vec<&str> = raw.split(':').collect();
    let [group, artifact, version, rest @ ..] = parts.as_slice() else {
        return Err(SpecRefusal::Coordinate);
    };
    if rest.len() > 2 {
        return Err(SpecRefusal::Coordinate);
    }
    if parts.iter().any(|p| !ident(p, "._-")) {
        return Err(SpecRefusal::Segment);
    }
    if matches!(*version, "LATEST" | "RELEASE") {
        return Err(SpecRefusal::Floating);
    }
    Ok(ParsedLibSpec::Maven {
        group_id: (*group).to_owned(),
        artifact_id: (*artifact).to_owned(),
        version: (*version).to_owned(),
        packaging: rest.first().map(|s| (*s).to_owned()),
        classifier: rest.get(1).map(|s| (*s).to_owned()),
    })
}

/// Validate and parse one raw spec against its ecosystem's grammar: the single door. Over
/// [`MAX_SPEC_CHARS`] chars or containing `..` is refused before any grammar runs.
#[allow(dead_code)] // reason: seam, used by T4.3 / T4.4
pub(crate) fn parse_spec(eco: LibEcosystem, raw: &str) -> Parsed {
    if raw.chars().nth(MAX_SPEC_CHARS).is_some() {
        return Err(SpecRefusal::TooLong);
    }
    if raw.contains("..") {
        return Err(SpecRefusal::Traversal);
    }
    match eco {
        LibEcosystem::Pnpm => pnpm(raw),
        LibEcosystem::Pip => pip(raw),
        LibEcosystem::Cargo => cargo(raw),
        LibEcosystem::Maven => maven(raw),
    }
}

/// Every npm name that fails the grammar, in order; empty means all valid.
#[allow(dead_code)] // reason: seam, used by T4.4
pub(crate) fn validate_lib_names<'a>(names: &[&'a str]) -> Vec<&'a str> {
    names
        .iter()
        .copied()
        .filter(|n| parse_spec(LibEcosystem::Pnpm, n).is_err())
        .collect()
}

/// An npm spec without its `@range`.
#[allow(dead_code)] // reason: seam, used by T4.4
pub(crate) fn package_name_of(spec: &str) -> &str {
    match spec.rsplit_once('@') {
        Some((n, _)) if !n.is_empty() => n,
        _ => spec,
    }
}
