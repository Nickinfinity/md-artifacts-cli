//! Variable sets over parsed Variables files (spec §6, §13.2–§13.3): shapes, listed sub-sets, the
//! search text and filter, and (in `score`) scoring, ordering and apply. Pure.

mod score;
pub use score::*;

use crate::error::VarsError;
use crate::model::{ParsedArtifact, ParsedVar};
use crate::parse::text::js_trim;
use crate::vks::{VksList, VksValue};

/// A Variables file's shape (TS `getVarsFileShape`): `sets` if any `##` block, `flat` if top vars,
/// else `blank`.
///
/// # Examples
///
/// ```
/// use mda_core::varset::Shape;
/// assert_eq!(serde_json::to_value(Shape::Flat).unwrap(), "flat");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Shape {
    Blank,
    Flat,
    Sets,
}

/// The shape of a parsed Variables file.
///
/// # Examples
///
/// ```
/// use mda_core::{parse::parse_from_content, varset::{shape, Shape}};
/// let p = parse_from_content("---\nartifactType: Variables\n---\n", "Variables/b.md");
/// assert_eq!(shape(&p), Shape::Blank);
/// ```
pub fn shape(p: &ParsedArtifact) -> Shape {
    if !p.blocks.is_empty() {
        Shape::Sets
    } else if !p.vars.is_empty() {
        Shape::Flat
    } else {
        Shape::Blank
    }
}

/// One listed sub-set; `index`/`heading` are the X-10 address (`{0, ""}` = a flat file's untitled block).
///
/// # Examples
///
/// ```
/// use mda_core::varset::Subset;
/// let s = Subset { index: 0, heading: String::new(), description: String::new(), vars: vec![], vars_error: None };
/// assert!(serde_json::to_value(&s).unwrap().get("varsError").is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Subset {
    pub index: usize,
    pub heading: String,
    pub description: String,
    pub vars: Vec<ParsedVar>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vars_error: Option<VarsError>,
}

/// The listed sub-sets: blocks → one each (empty kept); no blocks + top vars → `[{0, "", "", vars}]`;
/// else `[]`. Indexes equal `from_parsed(p).blocks` positions.
///
/// # Examples
///
/// ```
/// use mda_core::{parse::parse_from_content, varset::subsets};
/// let p = parse_from_content("---\nartifactType: Variables\n---\n", "Variables/b.md");
/// assert!(subsets(&p).is_empty());
/// ```
pub fn subsets(p: &ParsedArtifact) -> Vec<Subset> {
    if !p.blocks.is_empty() {
        return (p.blocks.iter().enumerate())
            .map(|(index, b)| Subset {
                index,
                heading: b.heading.clone(),
                description: b.description.clone(),
                vars: b.vars.clone(),
                vars_error: b.vars_error.clone(),
            })
            .collect();
    }
    if p.vars.is_empty() {
        return vec![];
    }
    vec![Subset {
        index: 0,
        heading: String::new(),
        description: String::new(),
        vars: p.vars.clone(),
        vars_error: p.vars_error.clone(),
    }]
}

/// A search query: trimmed and lowercased; `None` when empty (empty query = everything).
///
/// # Examples
///
/// ```
/// use mda_core::varset::Query;
/// assert!(Query::new("host").is_some());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query(String);

impl Query {
    /// The query for `raw`; `None` when it is empty after trimming.
    pub fn new(raw: &str) -> Option<Self> {
        let t = js_trim(raw);
        (!t.is_empty()).then(|| Self(t.to_lowercase()))
    }

    /// Whether `text` contains the query, case-insensitively.
    pub fn hit(&self, text: &str) -> bool {
        text.to_lowercase().contains(&self.0)
    }
}

/// A file's search text (TS): `"{label} {description} {#tag #tag…}"`.
///
/// # Examples
///
/// ```
/// assert!(mda_core::varset::file_text("Flat", "", &[]).starts_with("Flat"));
/// ```
pub fn file_text(label: &str, description: &str, tags: &[String]) -> String {
    // Two spaces when the description is empty: TS joins the fields unconditionally.
    let tag_line: Vec<String> = tags.iter().map(|t| format!("#{t}")).collect();
    format!("{label} {description} {}", tag_line.join(" "))
}

/// Sub-sets narrowed to `q` below file level: a sub-set hit keeps it whole; else only its matching
/// vars (name + every string leaf); sub-sets left empty are dropped; order and `index` kept.
///
/// # Examples
///
/// ```
/// use mda_core::varset::{filter_subsets, Query};
/// assert!(filter_subsets(vec![], &Query::new("x").unwrap()).is_empty());
/// ```
pub fn filter_subsets(subsets: Vec<Subset>, q: &Query) -> Vec<Subset> {
    subsets
        .into_iter()
        .filter_map(|mut s| {
            if q.hit(&subset_text(&s)) {
                return Some(s);
            }
            s.vars.retain(|v| q.hit(&var_text(v)));
            (!s.vars.is_empty()).then_some(s)
        })
        .collect()
}

fn subset_text(s: &Subset) -> String {
    format!("{} {}", s.heading, s.description)
}

/// Name then every string leaf, single-space joined (X-6: record keys are not text).
fn var_text(v: &ParsedVar) -> String {
    let mut parts = vec![v.name.as_str()];
    leaves(&v.value, &mut parts);
    parts.join(" ")
}

pub(crate) fn leaves<'a>(v: &'a VksValue, out: &mut Vec<&'a str>) {
    match v {
        VksValue::Str(s) => out.push(s),
        VksValue::List(VksList::Strings(l)) => out.extend(l.iter().map(String::as_str)),
        VksValue::List(VksList::Records(rs)) => {
            for r in rs {
                r.0.iter().for_each(|(_, x)| leaves(x, out));
            }
        }
        VksValue::Record(r) => r.0.iter().for_each(|(_, x)| leaves(x, out)),
    }
}
