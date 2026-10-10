//! The var table and per-token resolution: loop scopes first, then globals (T3.1, T3.3).

use std::collections::HashMap;

use super::Values;
use crate::error::{RENDER_NOT_SCALAR, RENDER_UNKNOWN_VAR};
use crate::model::ParsedVar;
use crate::parse::tokens::detect_vars;
use crate::vks::{VksList, VksRecord, VksValue};

/// Full name (`VK-x`) -> effective value. A name with no effective value is absent.
pub(super) struct Table<'v>(HashMap<String, &'v VksValue>);

/// What a token resolved to; `Literal` carries the warning code for the failure.
pub(super) enum Resolved<'v> {
    Text(&'v str),
    Literal(&'static str),
}

/// A value node: a string, a record, or a list. Loop scopes hold the current list item.
#[derive(Clone, Copy)]
pub(super) enum Item<'v> {
    Str(&'v str),
    Rec(&'v VksRecord),
    List(&'v VksList),
}

/// An open loop: its path segments (no `VK-`) and the current item.
pub(super) type Scope<'a, 'v> = (Vec<&'a str>, Item<'v>);

impl<'v> Item<'v> {
    fn of(v: &'v VksValue) -> Self {
        match v {
            VksValue::Str(s) => Self::Str(s),
            VksValue::Record(r) => Self::Rec(r),
            VksValue::List(l) => Self::List(l),
        }
    }

    /// One path segment down. Failure carries the warning code.
    fn step(self, seg: &str) -> Result<Self, &'static str> {
        match self {
            Self::Rec(r) => {
                r.0.iter()
                    .find(|(k, _)| k == seg)
                    .map(|(_, v)| Self::of(v))
                    .ok_or(RENDER_UNKNOWN_VAR)
            }
            Self::Str(_) => Err(RENDER_UNKNOWN_VAR),
            Self::List(_) => Err(RENDER_NOT_SCALAR),
        }
    }

    /// Items of a list, for `each` and `join`.
    pub fn list_len(l: &VksList) -> usize {
        match l {
            VksList::Strings(v) => v.len(),
            VksList::Records(v) => v.len(),
        }
    }

    /// The `k`th item of a list.
    pub fn list_item(l: &'v VksList, k: usize) -> Option<Self> {
        match l {
            VksList::Strings(v) => v.get(k).map(|s| Self::Str(s)),
            VksList::Records(v) => v.get(k).map(Self::Rec),
        }
    }

    /// The field `seg` of a record (`None` for anything else or a missing field).
    pub fn field(self, seg: &str) -> Option<Self> {
        match self {
            Self::Rec(_) => self.step(seg).ok(),
            _ => None,
        }
    }
}

/// `""` counts as not supplied (R-12).
fn nonempty(v: &VksValue) -> bool {
    !matches!(v, VksValue::Str(s) if s.is_empty())
}

impl<'v> Table<'v> {
    /// Variables are the declared ones (last duplicate wins, like TS object assignment) plus every
    /// root named in `code` (B-RISK 1). Client keys outside that set are ignored.
    pub fn new(vars: &'v [ParsedVar], code: &str, values: &'v Values) -> Self {
        let defaults: HashMap<&str, &VksValue> =
            vars.iter().map(|v| (v.name.as_str(), &v.value)).collect();
        let detected = detect_vars(code);
        let names = vars
            .iter()
            .map(|v| v.name.as_str())
            .chain(detected.iter().map(|v| v.name.as_str()));
        let mut t = HashMap::new();
        for name in names {
            let chosen = values
                .get(name)
                .filter(|v| nonempty(v))
                .or_else(|| defaults.get(name).copied().filter(|v| nonempty(v)));
            if let Some(v) = chosen {
                t.insert(name.to_owned(), v);
            }
        }
        Self(t)
    }

    /// Where `path` starts: the innermost open loop whose path is a segment-prefix of it (the
    /// remainder is relative to that loop's item), else the global root. Returns the start node,
    /// the segments still to walk, and whether it is global (Choice-eligible).
    pub fn locate<'p>(
        &self,
        scopes: &[Scope<'_, 'v>],
        path: &'p str,
    ) -> Option<(Item<'v>, Vec<&'p str>, bool)> {
        let segs: Vec<&str> = path.split('.').collect();
        for (prefix, item) in scopes.iter().rev() {
            if segs.starts_with(prefix) {
                return Some((*item, segs.get(prefix.len()..)?.to_vec(), false));
            }
        }
        let root = self.0.get(format!("VK-{}", segs.first()?).as_str())?;
        Some((Item::of(root), segs.get(1..)?.to_vec(), true))
    }

    /// The node `path` names, plus whether Choice applies (global, no segments).
    pub fn node(
        &self,
        scopes: &[Scope<'_, 'v>],
        path: &str,
    ) -> Result<(Item<'v>, bool), &'static str> {
        let (mut cur, segs, global) = self.locate(scopes, path).ok_or(RENDER_UNKNOWN_VAR)?;
        for seg in &segs {
            cur = cur.step(seg)?;
        }
        Ok((cur, global && segs.is_empty()))
    }

    /// Resolve a plain token's `path` (`users.name`, no `VK-`).
    pub fn resolve(&self, scopes: &[Scope<'_, 'v>], path: &str) -> Resolved<'v> {
        let (cur, choice) = match self.node(scopes, path) {
            Ok(n) => n,
            Err(code) => return Resolved::Literal(code),
        };
        match cur {
            Item::Str(s) => Resolved::Text(s),
            // Choice: a plain token on a list of strings is its first item.
            Item::List(VksList::Strings(v)) if choice => match v.first() {
                Some(s) => Resolved::Text(s),
                None => Resolved::Literal(RENDER_UNKNOWN_VAR),
            },
            Item::List(VksList::Records(v)) if choice && v.is_empty() => {
                Resolved::Literal(RENDER_UNKNOWN_VAR)
            }
            _ => Resolved::Literal(RENDER_NOT_SCALAR),
        }
    }
}
