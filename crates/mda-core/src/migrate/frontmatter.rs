//! Frontmatter migration: the legacy `type:` line → `artifactType: <TypeName>` (spec §11.1).
//! Ports `planFrontmatterRewrite` + `explainUnrecognisedFrontmatter` (`frontmatter-migration.service.ts`).

use std::ops::Range;

use super::Skip;
use crate::parse::text::js_trim;
use crate::parse::{body_range, frontmatter_of, key_of};
use crate::registry::{ArtifactType, TYPES};

/// One planned rewrite: the line before and after, the mapped type, and the whole new content.
///
/// # Examples
///
/// ```
/// use mda_core::{migrate::frontmatter::FmRewrite, registry::ArtifactType};
/// let r = FmRewrite { old_line: "type: snippet".into(), new_line: "artifactType: Snippet".into(),
///     artifact_type: ArtifactType::Snippet, content: String::new() };
/// assert_eq!(r.artifact_type, ArtifactType::Snippet);
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FmRewrite {
    pub old_line: String,
    pub new_line: String,
    pub artifact_type: ArtifactType,
    pub content: String,
}

/// TS `planFrontmatterRewrite` + `explainUnrecognisedFrontmatter` on decoded content (BOM already
/// stripped, Q-B): `Ok(Some)` = rewrite, `Ok(None)` = nothing to do, `Err` = left with a reason.
///
/// # Examples
///
/// ```
/// assert!(mda_core::migrate::frontmatter::rewrite("no frontmatter").is_ok());
/// ```
pub fn rewrite(content: &str) -> Result<Option<FmRewrite>, Skip> {
    let Some(block) = body_range(content) else {
        let mut rest = content;
        while let Some(r) = rest
            .strip_prefix("\r\n")
            .or_else(|| rest.strip_prefix('\n'))
        {
            rest = r;
        }
        if rest.len() == content.len() {
            return Ok(None);
        }
        return match rewrite(rest) {
            Ok(Some(_)) => Err(Skip::LeadingBlank),
            _ => Ok(None),
        };
    };
    let lines = lines(content, block);
    if lines.iter().any(|(_, l)| key_of(l) == Some("artifactType")) {
        return Ok(None);
    }
    let mut type_lines = lines.iter().filter(|(_, l)| key_of(l) == Some("type"));
    if let Some(&(off, text)) = type_lines.next()
        && let Some(t) = text
            .split_once(':')
            .and_then(|(_, v)| legacy_type(js_trim(v)))
    {
        {
            let new_line = format!("artifactType: {}", t.as_str());
            let (Some(head), Some(tail)) = (content.get(..off), content.get(off + text.len()..))
            else {
                return Ok(None);
            };
            let out = format!("{head}{new_line}{tail}");
            guard(content, &out, t)?;
            return Ok(Some(FmRewrite {
                old_line: text.to_owned(),
                new_line,
                artifact_type: t,
                content: out,
            }));
        }
    }
    // TS `explainUnrecognisedFrontmatter` scans every `type` line, not only the first.
    let quoted = lines
        .iter()
        .filter(|(_, l)| key_of(l) == Some("type"))
        .find_map(|(_, text)| {
            let raw = js_trim(text.split_once(':')?.1);
            (raw != unquote(raw) && legacy_type(unquote(raw)).is_some()).then(|| raw.to_owned())
        });
    quoted.map_or(Ok(None), |raw| Err(Skip::QuotedType(raw)))
}

/// The legacy lowercase `type:` value → its type (`aiprompt` never existed as a legacy value).
fn legacy_type(v: &str) -> Option<ArtifactType> {
    TYPES
        .iter()
        .map(|t| t.artifact_type)
        .filter(|&t| t != ArtifactType::AIPrompt)
        .find(|t| match t {
            ArtifactType::AIAgentsConfig => v == "agent",
            _ => t.as_str().to_ascii_lowercase() == v,
        })
}

/// `(absolute offset, text without `\n` and one trailing `\r`)` per line of `span` (TS `linesWithOffsets`).
fn lines(content: &str, span: Range<usize>) -> Vec<(usize, &str)> {
    let Some(block) = content.get(span.clone()) else {
        return vec![];
    };
    let mut off = span.start;
    block
        .split_inclusive('\n')
        .map(|l| {
            let at = off;
            off += l.len();
            let l = l.strip_suffix('\n').unwrap_or(l);
            (at, l.strip_suffix('\r').unwrap_or(l))
        })
        .collect()
}

/// TS `/^['"]|['"]$/g`: one quote off each end, independently.
fn unquote(raw: &str) -> &str {
    let r = raw.strip_prefix(['\'', '"']).unwrap_or(raw);
    r.strip_suffix(['\'', '"']).unwrap_or(r)
}

/// RISK 1: only the type may change, compared on the frontmatter alone (the whole-file parse
/// depends on the directory type, which a mis-filed file would trip).
fn guard(before: &str, after: &str, t: ArtifactType) -> Result<(), Skip> {
    let mut want = frontmatter_of(before);
    want.artifact_type = t;
    if want == frontmatter_of(after) {
        Ok(())
    } else {
        Err(Skip::RoundTrip {
            field: "artifactType",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_refuses_a_changed_field() {
        assert_eq!(
            guard(
                "---\ntitle: A\n---\n",
                "---\ntitle: B\nartifactType: Snippet\n---\n",
                ArtifactType::Snippet
            ),
            Err(Skip::RoundTrip {
                field: "artifactType"
            })
        );
    }
}
