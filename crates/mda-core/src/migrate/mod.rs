//! The vault migration (spec §11): frontmatter `type:` → `artifactType:`. A pure rewrite of decoded
//! content; the ops crate walks, hash-checks and writes. (The pre-W7e vks migration is gone with the
//! legacy `KEY=value` dialect it converted.)

pub mod frontmatter;

use std::collections::BTreeMap;

use crate::error::{ARTIFACT_UNREPRESENTABLE, MIGRATE_LEADING_BLANK, MIGRATE_QUOTED_TYPE};

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
    RoundTrip { field: &'static str },
}

impl Skip {
    /// The error code this skip reports.
    pub fn code(&self) -> &'static str {
        match self {
            Self::LeadingBlank => MIGRATE_LEADING_BLANK,
            Self::QuotedType(_) => MIGRATE_QUOTED_TYPE,
            Self::RoundTrip { .. } => ARTIFACT_UNREPRESENTABLE,
        }
    }

    /// The code's params (string values, as `OpError`).
    pub fn params(&self) -> BTreeMap<String, String> {
        let mut p = BTreeMap::new();
        match self {
            Self::LeadingBlank => {}
            Self::QuotedType(v) => {
                p.insert("value".to_owned(), v.clone());
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
                Skip::RoundTrip { field: "vars" },
                "artifact.unrepresentable",
                s(&[("field", "vars"), ("reason", "round_trip")]),
            ),
        ];
        for (skip, code, params) in cases {
            assert_eq!(skip.code(), code);
            assert_eq!(skip.params(), params);
        }
    }
}
