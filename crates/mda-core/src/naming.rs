//! Output file names for whole-file artifacts (spec §5.1, §5.2): the name precedence per type, the
//! path-injection check on every input, and the final-name validation. Reject, never sanitise.

use crate::error::{
    NAMING_CONTROL_CHAR, NAMING_EDGE_DOT, NAMING_EDGE_SPACE, NAMING_EMPTY, NAMING_ILLEGAL_CHAR,
    NAMING_PATH_INJECTION, NAMING_RESERVED, NAMING_TOO_LONG,
};
use crate::language::{ext_for_lang, normalize_lang_id};
use crate::model::ParsedArtifact;
use crate::parse::text::js_trim;
use crate::registry::OutputNameKey;

/// Longest output file name, in bytes.
pub const MAX_NAME_BYTES: usize = 255;

/// Which input a path-injection rejection came from.
///
/// # Examples
///
/// ```
/// use mda_core::naming::NameField;
/// assert_eq!(NameField::FileName.as_str(), "fileName");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameField {
    FileName,
    Extension,
    Target,
    Title,
}

impl NameField {
    /// The protocol spelling (the `field` param of `naming.path_injection`).
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FileName => "fileName",
            Self::Extension => "extension",
            Self::Target => "target",
            Self::Title => "title",
        }
    }
}

/// Why an output file name was rejected.
///
/// # Examples
///
/// ```
/// use mda_core::naming::{NameError, NameField};
/// assert_eq!(NameError::PathInjection(NameField::Title).code(), "naming.path_injection");
/// assert_eq!(NameError::Empty.code(), "naming.empty");
/// assert_eq!(NameError::EdgeSpace.code(), "naming.edge_space");
/// assert_eq!(NameError::EdgeDot.code(), "naming.edge_dot");
/// assert_eq!(NameError::IllegalChar.code(), "naming.illegal_char");
/// assert_eq!(NameError::ControlChar.code(), "naming.control_char");
/// assert_eq!(NameError::Reserved.code(), "naming.reserved");
/// assert_eq!(NameError::TooLong.code(), "naming.too_long");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameError {
    PathInjection(NameField),
    Empty,
    EdgeSpace,
    EdgeDot,
    IllegalChar,
    ControlChar,
    Reserved,
    TooLong,
}

impl NameError {
    /// The `naming.*` code.
    pub fn code(self) -> &'static str {
        match self {
            Self::PathInjection(_) => NAMING_PATH_INJECTION,
            Self::Empty => NAMING_EMPTY,
            Self::EdgeSpace => NAMING_EDGE_SPACE,
            Self::EdgeDot => NAMING_EDGE_DOT,
            Self::IllegalChar => NAMING_ILLEGAL_CHAR,
            Self::ControlChar => NAMING_CONTROL_CHAR,
            Self::Reserved => NAMING_RESERVED,
            Self::TooLong => NAMING_TOO_LONG,
        }
    }
}

/// Last `.` strictly inside the name (index > 0, not the final char): the name already has an
/// extension (`helpers.ts:48-51`).
fn carries_extension(s: &str) -> bool {
    s.rfind('.').is_some_and(|i| i > 0 && i + 1 < s.len())
}

fn strip_trailing_dots(s: &str) -> &str {
    s.trim_end_matches('.')
}

/// `title || file_name` (JS falsy: only `""` falls through), trimmed.
fn title_or_stem(p: &ParsedArtifact) -> &str {
    let t = p.frontmatter.title.as_deref().unwrap_or("");
    js_trim(if t.is_empty() { &p.file_name } else { t })
}

/// The output file name of a whole-file artifact (`typed` = the name the user typed, if any).
/// Every input is injection-checked before composition and the result is validated.
///
/// Deviations from the TS: the title fallback is injection-checked too (R-16), a leading dot is
/// allowed (B-3).
///
/// # Examples
///
/// ```
/// use mda_core::{naming::output_name, parse::parse_from_content};
/// let p = parse_from_content("---\nartifactType: Template\ntitle: Button\nextension: tsx\n---\n```tsx\nx\n```\n", "Templates/b.md");
/// assert_eq!(output_name(&p, None).unwrap(), "Button.tsx");
/// ```
pub fn output_name(p: &ParsedArtifact, typed: Option<&str>) -> Result<String, NameError> {
    let typed = js_trim(typed.unwrap_or(""));
    let name = match p.frontmatter.artifact_type.info().output_name_key {
        Some(OutputNameKey::Extension) => template_name(p, typed)?,
        Some(OutputNameKey::Target) => agent_name(p, typed)?,
        None => p.file_name.clone(),
    };
    validate_name(&name)?;
    Ok(name)
}

fn template_name(p: &ParsedArtifact, typed: &str) -> Result<String, NameError> {
    if !typed.is_empty() {
        check_injection(typed, NameField::FileName)?;
    }
    let ext = js_trim(p.frontmatter.extension.as_deref().unwrap_or(""));
    if !ext.is_empty() {
        check_injection(ext, NameField::Extension)?;
    }
    if !typed.is_empty() && carries_extension(typed) {
        return Ok(typed.to_owned());
    }
    let raw = if typed.is_empty() {
        title_or_stem(p)
    } else {
        typed
    };
    let base = strip_trailing_dots(raw);
    if !base.is_empty() && typed.is_empty() {
        check_injection(base, NameField::Title)?;
    }
    let base = if base.is_empty() { "template" } else { base };
    // `extension: .` leaves "" and suppresses the language fallback (TS resolveExtension).
    let ext = if !ext.is_empty() {
        ext.trim_start_matches('.').to_owned()
    } else {
        let lang = js_trim(p.frontmatter.language.as_deref().unwrap_or(""));
        if lang.is_empty() {
            String::new()
        } else {
            ext_for_lang(&normalize_lang_id(lang)).to_owned()
        }
    };
    Ok(if ext.is_empty() {
        base.to_owned()
    } else {
        format!("{base}.{ext}")
    })
}

fn agent_name(p: &ParsedArtifact, typed: &str) -> Result<String, NameError> {
    if !typed.is_empty() {
        check_injection(typed, NameField::FileName)?;
        return Ok(typed.to_owned());
    }
    let target = js_trim(p.frontmatter.target.as_deref().unwrap_or(""));
    if !target.is_empty() {
        check_injection(target, NameField::Target)?;
        return Ok(target.to_owned());
    }
    let base = strip_trailing_dots(title_or_stem(p));
    let base = if base.is_empty() { "agent" } else { base };
    check_injection(base, NameField::Title)?;
    Ok(if carries_extension(base) {
        base.to_owned()
    } else {
        format!("{base}.md")
    })
}

/// Validate a final file name; first failure wins: length, empty, edge space, edge dot, illegal
/// characters, control characters, reserved device names (whole name or stem before the first
/// `.`, an R-6 deviation from the whole-name TS rule). Length is bytes (R-7).
///
/// # Examples
///
/// ```
/// use mda_core::naming::{validate_name, NameError};
/// assert_eq!(validate_name("Button.tsx"), Ok(()));
/// assert_eq!(validate_name("CON.txt"), Err(NameError::Reserved));
/// ```
pub fn validate_name(name: &str) -> Result<(), NameError> {
    if name.len() > MAX_NAME_BYTES {
        return Err(NameError::TooLong);
    }
    if js_trim(name).is_empty() {
        return Err(NameError::Empty);
    }
    if name.starts_with(' ') || name.ends_with(' ') {
        return Err(NameError::EdgeSpace);
    }
    if name.ends_with('.') {
        return Err(NameError::EdgeDot);
    }
    if name.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|']) {
        return Err(NameError::IllegalChar);
    }
    if name.chars().any(|c| c.is_ascii_control()) {
        return Err(NameError::ControlChar);
    }
    let stem = name.split('.').next().unwrap_or(name).to_ascii_lowercase();
    let n = stem
        .strip_prefix("com")
        .or_else(|| stem.strip_prefix("lpt"));
    let numbered = n.is_some_and(|d| matches!(d.as_bytes(), [b'1'..=b'9']));
    if numbered || ["con", "prn", "aux", "nul"].contains(&stem.as_str()) {
        return Err(NameError::Reserved);
    }
    Ok(())
}

/// Reject a name input containing `/`, `\`, NUL or `..` (a substring test, so `a..b` fails).
///
/// # Examples
///
/// ```
/// use mda_core::naming::{NameField, check_injection};
/// assert!(check_injection("Button", NameField::Title).is_ok());
/// assert!(check_injection("a..b", NameField::Title).is_err());
/// ```
pub fn check_injection(value: &str, field: NameField) -> Result<(), NameError> {
    if value.contains(['/', '\\', '\0']) || value.contains("..") {
        return Err(NameError::PathInjection(field));
    }
    Ok(())
}
