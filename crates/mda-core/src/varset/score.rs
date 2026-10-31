//! Var-set scoring, ordering and apply (spec §13.3, X-16, X-17). The artifact's side is built once
//! per call (`Profile`), so scoring stays linear in the input (#26).

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use crate::model::ParsedVar;
use crate::render::Values;
use crate::vks::VksValue;

/// An artifact's side of a match, built once per call: names → first position, tag → count, tag total.
///
/// # Examples
///
/// ```
/// use mda_core::varset::Profile;
/// let p = Profile::new(vec!["VK-a".into()], &["api".into()]);
/// assert_eq!(p.names(), ["VK-a"]);
/// ```
#[derive(Debug, Clone)]
pub struct Profile {
    names: Vec<String>,
    pos: HashMap<String, usize>,
    tags: HashMap<String, usize>,
    n_tags: usize,
}

impl Profile {
    /// The profile of an artifact's variable names (render rule) and its tags.
    pub fn new(names: Vec<String>, tags: &[String]) -> Self {
        let mut pos = HashMap::with_capacity(names.len());
        for (i, n) in names.iter().enumerate() {
            pos.entry(n.clone()).or_insert(i);
        }
        let mut counts: HashMap<String, usize> = HashMap::new();
        for t in tags {
            *counts.entry(t.clone()).or_insert(0) += 1;
        }
        Self {
            names,
            pos,
            tags: counts,
            n_tags: tags.len(),
        }
    }

    /// The artifact's variable names, in order.
    pub fn names(&self) -> &[String] {
        &self.names
    }
}

/// One candidate's score against a [`Profile`].
///
/// # Examples
///
/// ```
/// use mda_core::varset::{score, Profile};
/// let s = score(&Profile::new(vec![], &[]), &[], &[]);
/// assert_eq!(s.match_ratio, 0.0);
/// ```
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Score {
    pub score: f64,
    pub match_ratio: f64,
    pub tag_matches: usize,
    pub matched_vars: Vec<String>,
    pub extra_vars: Vec<String>,
}

/// X-16: `matchRatio × 0.7 + tagRatio × 0.3`, one pass over the **set's** vars and tags.
///
/// # Examples
///
/// ```
/// use mda_core::varset::{score, Profile};
/// assert!(score(&Profile::new(vec![], &[]), &[], &[]).matched_vars.is_empty());
/// ```
pub fn score(a: &Profile, set_vars: &[ParsedVar], set_tags: &[String]) -> Score {
    let mut seen = HashSet::new();
    let mut hits: Vec<(usize, &str)> = Vec::new();
    let mut extra_vars = Vec::new();
    for v in set_vars.iter().filter(|v| seen.insert(v.name.as_str())) {
        match a.pos.get(&v.name) {
            Some(&i) => hits.push((i, &v.name)),
            None => extra_vars.push(v.name.clone()),
        }
    }
    hits.sort_unstable_by_key(|h| h.0);
    let matched_vars: Vec<String> = hits.iter().map(|h| h.1.to_owned()).collect();
    let ratio = |n: usize, d: usize| if d == 0 { 0.0 } else { n as f64 / d as f64 };
    let match_ratio = ratio(matched_vars.len(), a.names.len());
    // Distinct set tags only: the artifact side already carries its duplicates (TS reduce).
    let distinct: HashSet<&String> = set_tags.iter().collect();
    let tag_matches = distinct.iter().filter_map(|t| a.tags.get(*t)).sum();
    let score = match_ratio * 0.7 + ratio(tag_matches, a.n_tags) * 0.3;
    Score {
        score,
        match_ratio,
        tag_matches,
        matched_vars,
        extra_vars,
    }
}

/// X-16 order: score descending (`total_cmp`), then path (bytes), then sub-set index.
///
/// # Examples
///
/// ```
/// use mda_core::varset::order;
/// let _ = order((0.5, "a", 0), (0.5, "a", 0));
/// ```
pub fn order(a: (f64, &str, usize), b: (f64, &str, usize)) -> Ordering {
    b.0.total_cmp(&a.0)
        .then_with(|| a.1.as_bytes().cmp(b.1.as_bytes()))
        .then_with(|| a.2.cmp(&b.2))
}

/// What applying a set did to one variable.
///
/// # Examples
///
/// ```
/// assert_eq!(serde_json::to_value(mda_core::varset::Action::Filled).unwrap(), "filled");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Action {
    Filled,
    Overridden,
    Kept,
}

/// One variable's change.
///
/// # Examples
///
/// ```
/// use mda_core::{varset::{Action, Change}, vks::VksValue};
/// let c = Change { name: "VK-a".into(), old_value: VksValue::Str(String::new()), new_value: VksValue::Str("x".into()), action: Action::Filled };
/// assert!(serde_json::to_value(&c).unwrap().get("oldValue").is_some());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub name: String,
    pub old_value: VksValue,
    pub new_value: VksValue,
    pub action: Action,
}

/// The merged values and the per-variable changes, in name order.
///
/// # Examples
///
/// ```
/// use mda_core::{render::Values, varset::Applied};
/// let a = Applied { values: Values::new(), changes: vec![] };
/// assert!(a.changes.is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Applied {
    pub values: Values,
    pub changes: Vec<Change>,
}

/// X-17: union of current and set names (name order); the set's value wins; "old empty" = absent or `""`.
///
/// # Examples
///
/// ```
/// use mda_core::{render::Values, varset::apply};
/// let _ = apply(&Values::new(), &[]);
/// ```
pub fn apply(current: &Values, set_vars: &[ParsedVar]) -> Applied {
    let set: BTreeMap<&str, &VksValue> = set_vars
        .iter()
        .map(|v| (v.name.as_str(), &v.value))
        .collect();
    let empty = VksValue::Str(String::new());
    let names: BTreeSet<&str> = current
        .keys()
        .map(String::as_str)
        .chain(set.keys().copied())
        .collect();
    let mut values = Values::new();
    let mut changes = Vec::with_capacity(names.len());
    for name in names {
        let old = current.get(name).unwrap_or(&empty);
        let (new, action) = match set.get(name) {
            Some(n) if *old == empty => ((*n).clone(), Action::Filled),
            Some(n) => ((*n).clone(), Action::Overridden),
            None => (old.clone(), Action::Kept),
        };
        values.insert(name.to_owned(), new.clone());
        changes.push(Change {
            name: name.to_owned(),
            old_value: old.clone(),
            new_value: new,
            action,
        });
    }
    Applied { values, changes }
}
