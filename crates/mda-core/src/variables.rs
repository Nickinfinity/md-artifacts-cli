//! Variables-file edits on an [`ArtifactModel`] (spec §3, §13.1): variables and `##` sub-sets, the
//! name rule, the shape transitions blank / flat / sets. Pure: the serializer decides emission and
//! runs the codec's checks; every function here validates fully before its first model write.

use crate::model::{ArtifactModel, ModelBlock, ParsedArtifact, ParsedVar};
use crate::parse::text::js_trim;
use crate::parse::tokens::TOKEN_RE;
use crate::serialize::from_parsed;
use crate::vks::VksValue;

/// Which sub-set an edit targets: its index in the model's blocks and its heading; both must
/// match (X-10). `{index: 0, heading: ""}` is a flat file's untitled block.
///
/// # Examples
///
/// ```
/// use mda_core::variables::SubsetRef;
/// let r: SubsetRef = serde_json::from_value(serde_json::json!({"index": 1, "heading": "Prod"})).unwrap();
/// assert_eq!(r, SubsetRef { index: 1, heading: "Prod".into() });
/// assert_eq!(serde_json::to_value(&r).unwrap(), serde_json::json!({"index": 1, "heading": "Prod"}));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubsetRef {
    pub index: usize,
    pub heading: String,
}

/// Why an edit was refused. Codes live in `mda-ops` (`From<EditError> for OpError`).
///
/// # Examples
///
/// ```
/// use mda_core::variables::EditError;
/// assert_ne!(EditError::BlockNotFound(None), EditError::BlockNotFound(Some(0)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditError {
    /// Not a token-referenceable name (`VK-` + identifier, no dots).
    BadName(String),
    /// The name already exists in that sub-set.
    VarExists(String),
    /// No var of that exact name in that sub-set.
    VarNotFound(String),
    /// Another block already has that (trimmed) heading.
    SubsetExists(String),
    /// The heading is empty after trimming.
    EmptyHeading,
    /// A description on the untitled block.
    UntitledDescription,
    /// The sub-set reference matches no block (`None` = no sub-set given where one is needed).
    BlockNotFound(Option<usize>),
    /// A top untitled fence plus `##` sections: an edit would drop the top vars (RISK 3).
    Mixed,
}

/// Whether `name` is a variable name a plain token can reference: `VK-` + a letter, then
/// letters, digits or `_`, no dots (X-13).
///
/// # Examples
///
/// ```
/// assert!(mda_core::variables::is_var_name("VK-host"));
/// assert!(!mda_core::variables::is_var_name("host"));
/// ```
pub fn is_var_name(name: &str) -> bool {
    // A full match of one *plain* token (group 2): closing, each/join and end tags are refusals, and
    // a dotted path is a reference, not a name.
    let tok = format!("<{name}>");
    !name.contains('.')
        && TOKEN_RE
            .captures(&tok)
            .is_some_and(|c| c.get(0).is_some_and(|w| w.len() == tok.len()) && c.get(2).is_some())
}

/// `Default`, or `Default 2`, `Default 3`, … — the first not in `taken` (TS `uniqueDefaultName`).
///
/// # Examples
///
/// ```
/// assert_eq!(mda_core::variables::unique_default_name(&[]), "Default");
/// ```
pub fn unique_default_name(taken: &[&str]) -> String {
    std::iter::once("Default".to_owned())
        .chain((2..).map(|n| format!("Default {n}")))
        .find(|c| !taken.contains(&c.as_str()))
        .unwrap_or_default()
}

/// The editable model of a parsed Variables file; refuses a mixed file ([`EditError::Mixed`]).
///
/// # Examples
///
/// ```
/// use mda_core::{parse::parse_from_content, variables::model_of};
/// let p = parse_from_content("---\nartifactType: Variables\ntitle: B\n---\n", "Variables/b.md");
/// assert!(model_of(&p).unwrap().blocks.is_empty());
/// ```
pub fn model_of(p: &ParsedArtifact) -> Result<ArtifactModel, EditError> {
    // `from_parsed` lets blocks win over the top fence, so a mixed file would silently lose it. The
    // parser's file-level vars come from the first fence anywhere: in a pure `sets` file they equal
    // some section's vars. ponytail: a top fence byte-equal to a section's vars is not detected —
    // exact detection needs the raw content.
    let mixed =
        !p.vars.is_empty() && !p.blocks.is_empty() && !p.blocks.iter().any(|b| b.vars == p.vars);
    if mixed {
        return Err(EditError::Mixed);
    }
    Ok(from_parsed(p))
}

/// Add `name` = `value` to the targeted block (absent `at` on a blank file creates the flat block).
///
/// # Examples
///
/// ```
/// use mda_core::{parse::parse_from_content, variables::{add_var, model_of}, vks::VksValue};
/// let p = parse_from_content("---\nartifactType: Variables\ntitle: B\n---\n", "Variables/b.md");
/// let mut m = model_of(&p).unwrap();
/// let _ = add_var(&mut m, None, "VK-a", VksValue::Str("x".into()));
/// ```
pub fn add_var(
    m: &mut ArtifactModel,
    at: Option<&SubsetRef>,
    name: &str,
    value: VksValue,
) -> Result<(), EditError> {
    if !is_var_name(name) {
        return Err(EditError::BadName(name.to_owned()));
    }
    // Only now is a push safe: nothing below can fail for a blank file.
    if at.is_none() && m.blocks.is_empty() {
        m.blocks.push(ModelBlock::default());
    }
    let b = block(m, target(m, at)?)?;
    if b.vars.iter().any(|v| v.name == name) {
        return Err(EditError::VarExists(name.to_owned()));
    }
    b.vars.push(ParsedVar {
        name: name.to_owned(),
        value,
    });
    Ok(())
}

/// Rename `name` to `new_name` in the targeted block, keeping value and position.
///
/// # Examples
///
/// ```
/// use mda_core::{parse::parse_from_content, variables::{rename_var, model_of}};
/// let p = parse_from_content("---\nartifactType: Variables\ntitle: B\n---\n", "Variables/b.md");
/// let mut m = model_of(&p).unwrap();
/// let _ = rename_var(&mut m, None, "VK-a", "VK-b");
/// ```
pub fn rename_var(
    m: &mut ArtifactModel,
    at: Option<&SubsetRef>,
    name: &str,
    new_name: &str,
) -> Result<(), EditError> {
    if !is_var_name(new_name) {
        return Err(EditError::BadName(new_name.to_owned()));
    }
    let b = block(m, target(m, at)?)?;
    let k = var_at(b, name)?;
    if new_name != name && b.vars.iter().any(|v| v.name == new_name) {
        return Err(EditError::VarExists(new_name.to_owned()));
    }
    if let Some(v) = b.vars.get_mut(k) {
        v.name = new_name.to_owned();
    }
    Ok(())
}

/// Set the value of the first var named `name` in the targeted block.
///
/// # Examples
///
/// ```
/// use mda_core::{parse::parse_from_content, variables::{set_var, model_of}, vks::VksValue};
/// let p = parse_from_content("---\nartifactType: Variables\ntitle: B\n---\n", "Variables/b.md");
/// let mut m = model_of(&p).unwrap();
/// let _ = set_var(&mut m, None, "VK-a", VksValue::Str("x".into()));
/// ```
pub fn set_var(
    m: &mut ArtifactModel,
    at: Option<&SubsetRef>,
    name: &str,
    value: VksValue,
) -> Result<(), EditError> {
    let b = block(m, target(m, at)?)?;
    let k = var_at(b, name)?;
    if let Some(v) = b.vars.get_mut(k) {
        v.value = value;
    }
    Ok(())
}

/// Delete every var named `name` in the targeted block.
///
/// # Examples
///
/// ```
/// use mda_core::{parse::parse_from_content, variables::{delete_var, model_of}};
/// let p = parse_from_content("---\nartifactType: Variables\ntitle: B\n---\n", "Variables/b.md");
/// let mut m = model_of(&p).unwrap();
/// let _ = delete_var(&mut m, None, "VK-a");
/// ```
pub fn delete_var(
    m: &mut ArtifactModel,
    at: Option<&SubsetRef>,
    name: &str,
) -> Result<(), EditError> {
    let b = block(m, target(m, at)?)?;
    var_at(b, name)?;
    b.vars.retain(|v| v.name != name);
    Ok(())
}

/// Append an empty sub-set; a flat file's untitled block is titled `Default` (or `Default N`) first.
///
/// # Examples
///
/// ```
/// use mda_core::{parse::parse_from_content, variables::{add_subset, model_of}};
/// let p = parse_from_content("---\nartifactType: Variables\ntitle: B\n---\n", "Variables/b.md");
/// let mut m = model_of(&p).unwrap();
/// let _ = add_subset(&mut m, "Dev");
/// ```
pub fn add_subset(m: &mut ArtifactModel, heading: &str) -> Result<(), EditError> {
    let h = new_heading(m, heading, None)?;
    if is_flat(m) {
        // TS :278–281: the new heading is the only name taken.
        if let Some(b) = m.blocks.first_mut() {
            b.heading = unique_default_name(&[&h]);
        }
    }
    m.blocks.push(ModelBlock {
        heading: h,
        ..ModelBlock::default()
    });
    Ok(())
}

/// Rename the targeted sub-set (heading trimmed, unique per file).
///
/// # Examples
///
/// ```
/// use mda_core::{parse::parse_from_content, variables::{rename_subset, model_of, SubsetRef}};
/// let p = parse_from_content("---\nartifactType: Variables\ntitle: B\n---\n", "Variables/b.md");
/// let mut m = model_of(&p).unwrap();
/// let _ = rename_subset(&mut m, &SubsetRef { index: 0, heading: "A".into() }, "B");
/// ```
pub fn rename_subset(
    m: &mut ArtifactModel,
    at: &SubsetRef,
    heading: &str,
) -> Result<(), EditError> {
    let i = target(m, Some(at))?;
    let h = new_heading(m, heading, Some(i))?;
    block(m, i)?.heading = h;
    Ok(())
}

/// Set (trimmed) or remove (`""`) the targeted sub-set's description.
///
/// # Examples
///
/// ```
/// use mda_core::{parse::parse_from_content, variables::{describe_subset, model_of, SubsetRef}};
/// let p = parse_from_content("---\nartifactType: Variables\ntitle: B\n---\n", "Variables/b.md");
/// let mut m = model_of(&p).unwrap();
/// let _ = describe_subset(&mut m, &SubsetRef { index: 0, heading: "A".into() }, "d");
/// ```
pub fn describe_subset(
    m: &mut ArtifactModel,
    at: &SubsetRef,
    description: &str,
) -> Result<(), EditError> {
    let i = target(m, Some(at))?;
    let b = block(m, i)?;
    if b.heading.is_empty() {
        return Err(EditError::UntitledDescription);
    }
    b.description = js_trim(description).to_owned();
    Ok(())
}

/// Delete the targeted sub-set (the last one leaves a blank file).
///
/// # Examples
///
/// ```
/// use mda_core::{parse::parse_from_content, variables::{delete_subset, model_of, SubsetRef}};
/// let p = parse_from_content("---\nartifactType: Variables\ntitle: B\n---\n", "Variables/b.md");
/// let mut m = model_of(&p).unwrap();
/// let _ = delete_subset(&mut m, &SubsetRef { index: 0, heading: "A".into() });
/// ```
pub fn delete_subset(m: &mut ArtifactModel, at: &SubsetRef) -> Result<(), EditError> {
    let i = target(m, Some(at))?;
    m.blocks.remove(i);
    Ok(())
}

fn is_flat(m: &ArtifactModel) -> bool {
    matches!(m.blocks.as_slice(), [b] if b.heading.is_empty())
}

/// Block `i`, mutably; `target` already vouched for `i`, so the error arm is unreachable.
fn block(m: &mut ArtifactModel, i: usize) -> Result<&mut ModelBlock, EditError> {
    m.blocks.get_mut(i).ok_or(EditError::BlockNotFound(Some(i)))
}

/// The block an edit addresses (X-10): index and heading must both match; absent `at` is the
/// untitled block of a flat file. Shared by the edits and `varsets.apply` (one address rule).
///
/// # Examples
///
/// ```
/// use mda_core::{parse::parse_from_content, serialize::from_parsed, variables::target};
/// let p = parse_from_content("---\nartifactType: Variables\n---\n\n```yaml VKs\nVK-a: x\n```\n", "Variables/f.md");
/// assert_eq!(target(&from_parsed(&p), None), Ok(0));
/// ```
pub fn target(m: &ArtifactModel, at: Option<&SubsetRef>) -> Result<usize, EditError> {
    match at {
        Some(r) => m
            .blocks
            .get(r.index)
            .filter(|b| b.heading == r.heading)
            .map(|_| r.index)
            .ok_or(EditError::BlockNotFound(Some(r.index))),
        None if is_flat(m) => Ok(0),
        None => Err(EditError::BlockNotFound(None)),
    }
}

/// The trimmed heading, unique among the blocks other than `except`.
fn new_heading(
    m: &ArtifactModel,
    heading: &str,
    except: Option<usize>,
) -> Result<String, EditError> {
    let h = js_trim(heading);
    if h.is_empty() {
        return Err(EditError::EmptyHeading);
    }
    let taken = m
        .blocks
        .iter()
        .enumerate()
        .any(|(i, b)| Some(i) != except && b.heading == h);
    if taken {
        return Err(EditError::SubsetExists(h.to_owned()));
    }
    Ok(h.to_owned())
}

/// Index of the first var named exactly `name`.
fn var_at(b: &ModelBlock, name: &str) -> Result<usize, EditError> {
    b.vars
        .iter()
        .position(|v| v.name == name)
        .ok_or_else(|| EditError::VarNotFound(name.to_owned()))
}
