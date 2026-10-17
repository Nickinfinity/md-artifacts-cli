//! The two vault migrations (spec §11): frontmatter `type:` → `artifactType:` and legacy `vks`
//! bodies → YAML. Pure rewrites of decoded content; the ops crate walks, hash-checks and writes.

pub mod frontmatter;
pub mod vks;

use std::collections::BTreeMap;

use crate::error::{
    ARTIFACT_UNREPRESENTABLE, MIGRATE_LEADING_BLANK, MIGRATE_QUOTED_TYPE, MIGRATE_UNFENCED_VARS,
    Unrepresentable, VKS_UNREPRESENTABLE, VarsError,
};

/// A byte normalisation every engine write applies (#35/#36); the plan flags it before apply (M-4).
///
/// # Examples
///
/// ```
/// use mda_core::migrate::Normalise;
/// assert_eq!(serde_json::to_value(Normalise::Bom).unwrap(), "bom");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Normalise {
    Bom,
    Encoding,
}

/// `[bom]` if the bytes start EF BB BF; `[encoding]` if they are not valid UTF-8; both in that order.
///
/// # Examples
///
/// ```
/// assert!(mda_core::migrate::normalises(b"plain").is_empty());
/// ```
pub fn normalises(bytes: &[u8]) -> Vec<Normalise> {
    let mut out = Vec::new();
    if bytes.starts_with(b"\xEF\xBB\xBF") {
        out.push(Normalise::Bom);
    }
    if std::str::from_utf8(bytes).is_err() {
        out.push(Normalise::Encoding);
    }
    out
}

/// Why a file is left untouched. One code + params each.
///
/// # Examples
///
/// ```
/// use mda_core::migrate::Skip;
/// assert_eq!(Skip::LeadingBlank.code(), "migrate.leading_blank");
/// assert!(Skip::LeadingBlank.params().is_empty());
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Skip {
    LeadingBlank,
    QuotedType(String),
    Vars(VarsError),
    Unrepresentable(Unrepresentable),
    RoundTrip { field: &'static str },
    UnfencedVars,
}

impl Skip {
    /// The error code this skip reports.
    pub fn code(&self) -> &'static str {
        match self {
            Self::LeadingBlank => MIGRATE_LEADING_BLANK,
            Self::QuotedType(_) => MIGRATE_QUOTED_TYPE,
            Self::Vars(e) => e.code,
            Self::Unrepresentable(_) => VKS_UNREPRESENTABLE,
            Self::RoundTrip { .. } => ARTIFACT_UNREPRESENTABLE,
            Self::UnfencedVars => MIGRATE_UNFENCED_VARS,
        }
    }

    /// The code's params (string values, as `OpError`).
    pub fn params(&self) -> BTreeMap<String, String> {
        let mut p = BTreeMap::new();
        match self {
            Self::LeadingBlank | Self::UnfencedVars => {}
            Self::QuotedType(v) => {
                p.insert("value".to_owned(), v.clone());
            }
            Self::Vars(e) => p.clone_from(&e.params),
            Self::Unrepresentable(u) => {
                p.insert("var".to_owned(), u.var.clone());
                p.insert("reason".to_owned(), u.reason.as_str().to_owned());
            }
            Self::RoundTrip { field } => {
                p.insert("field".to_owned(), (*field).to_owned());
                p.insert("reason".to_owned(), "round_trip".to_owned());
            }
        }
        p
    }
}

/// `## ` sections the parser drops for lack of a code fence (#54): their trimmed headings, in order.
///
/// # Examples
///
/// ```
/// assert!(mda_core::migrate::fenceless_sections("").is_empty());
/// ```
pub fn fenceless_sections(content: &str) -> Vec<String> {
    crate::parse::fenceless_headings(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{EmitReason, VKS_BAD_KEY};

    #[test]
    fn skip_code_params() {
        let s = |k: &[(&str, &str)]| -> BTreeMap<String, String> {
            k.iter()
                .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
                .collect()
        };
        let cases = [
            (Skip::LeadingBlank, "migrate.leading_blank", s(&[])),
            (
                Skip::QuotedType("\"t\"".into()),
                "migrate.quoted_type",
                s(&[("value", "\"t\"")]),
            ),
            (
                Skip::Vars(VarsError::new(VKS_BAD_KEY, 0).with("key", "VK-a.b")),
                "vks.bad_key",
                s(&[("key", "VK-a.b"), ("line", "0")]),
            ),
            (
                Skip::Unrepresentable(Unrepresentable {
                    var: "VK-a".into(),
                    reason: EmitReason::Backticks,
                }),
                "vks.unrepresentable",
                s(&[("var", "VK-a"), ("reason", "backticks")]),
            ),
            (
                Skip::RoundTrip { field: "vars" },
                "artifact.unrepresentable",
                s(&[("field", "vars"), ("reason", "round_trip")]),
            ),
            (Skip::UnfencedVars, "migrate.unfenced_vars", s(&[])),
        ];
        for (skip, code, params) in cases {
            assert_eq!(skip.code(), code);
            assert_eq!(skip.params(), params);
        }
    }
}
