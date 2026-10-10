//! The parsed-artifact model: what `artifact.read` returns. Serialized camelCase; `None` omitted.

use serde::ser::SerializeMap;

use crate::error::VarsError;
use crate::registry::ArtifactType;
use crate::vks::VksValue;

/// Frontmatter keys the parser recognises; every other key is ignored.
///
/// # Examples
///
/// ```
/// use mda_core::{model::Frontmatter, registry::ArtifactType};
/// let f = Frontmatter::new(ArtifactType::Snippet);
/// assert_eq!(serde_json::to_string(&f).unwrap(), r#"{"artifactType":"Snippet"}"#);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Frontmatter {
    pub artifact_type: ArtifactType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extension: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<String>>,
}

impl Frontmatter {
    /// Frontmatter of type `t` with every optional key absent.
    pub fn new(t: ArtifactType) -> Self {
        Self {
            artifact_type: t,
            title: None,
            description: None,
            language: None,
            tags: None,
            env: None,
            target: None,
            extension: None,
            provider: None,
            model: None,
            version: None,
            index: None,
            paths: None,
        }
    }
}

/// One variable and its default. Serialized as `{name, defaultValue}` plus `value` only when the
/// value is structured (clients read `value ?? defaultValue`); `defaultValue` is derived, so the
/// two can never disagree (D-3).
///
/// # Examples
///
/// ```
/// use mda_core::model::ParsedVar;
/// let v = ParsedVar::text("VK-a", "1");
/// assert!(serde_json::to_string(&v).unwrap().starts_with(r#"{"name":"VK-a","defaultValue":"#));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedVar {
    pub name: String,
    pub value: VksValue,
}

impl ParsedVar {
    /// A variable with a plain string value.
    pub fn text(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: VksValue::Str(value.into()),
        }
    }
}

impl serde::Serialize for ParsedVar {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let structured = !matches!(self.value, VksValue::Str(_));
        let mut m = s.serialize_map(Some(if structured { 3 } else { 2 }))?;
        m.serialize_entry("name", &self.name)?;
        m.serialize_entry("defaultValue", self.value.default_value())?;
        if structured {
            m.serialize_entry("value", &self.value)?;
        }
        m.end()
    }
}

/// One `## heading` block of a multi-block artifact (or one sub-set of a Variables file).
///
/// # Examples
///
/// ```
/// use mda_core::model::ParsedBlock;
/// let b = ParsedBlock { heading: "H".into(), description: String::new(), code: "x".into(),
///     fence_lang: None, vars: vec![], vars_error: None };
/// assert!(serde_json::to_string(&b).unwrap().contains(r#""heading":"H""#));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedBlock {
    pub heading: String,
    pub description: String,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fence_lang: Option<String>,
    pub vars: Vec<ParsedVar>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vars_error: Option<VarsError>,
}

/// A parsed artifact file. `file_path` is vault-relative POSIX; `relative_path` is relative to the
/// type directory.
///
/// # Examples
///
/// ```
/// let a = mda_core::parse::parse_from_content("", "Snippets/a.md");
/// assert!(serde_json::to_value(&a).unwrap().get("filePath").is_some());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedArtifact {
    pub file_path: String,
    pub file_name: String,
    pub relative_path: String,
    pub frontmatter: Frontmatter,
    pub code: String,
    pub vars: Vec<ParsedVar>,
    pub blocks: Vec<ParsedBlock>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vars_error: Option<VarsError>,
}
