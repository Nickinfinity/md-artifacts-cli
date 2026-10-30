//! Output file names for whole-file artifacts (spec §5.1, §5.2): the name precedence per type, the
//! path-injection check on every input, and the final-name validation. Reject, never sanitise.

use crate::error::{
    NAMING_CONTROL_CHAR, NAMING_EDGE_DOT, NAMING_EDGE_SPACE, NAMING_EMPTY, NAMING_ILLEGAL_CHAR,
    NAMING_MD_EXTENSION, NAMING_NOT_NFC, NAMING_PATH_INJECTION, NAMING_RESERVED, NAMING_TOO_LONG,
};
use crate::language::{ext_for_lang, normalize_lang_id};
use crate::model::ParsedArtifact;
use crate::parse::text::js_trim;
use crate::registry::OutputNameKey;
use reserved::is_reserved;
use unicode_normalization::is_nfc;

mod reserved;
mod slug;
pub use slug::slugify;

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
/// assert_eq!(NameError::MdExtension.code(), "naming.md_extension");
/// assert_eq!(NameError::NotNfc.code(), "naming.not_nfc");
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
    MdExtension,
    NotNfc,
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
            Self::MdExtension => NAMING_MD_EXTENSION,
            Self::NotNfc => NAMING_NOT_NFC,
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

/// Longest vault file-name stem, in bytes: `stem.md` must fit in [`MAX_NAME_BYTES`].
const MAX_STEM_BYTES: usize = MAX_NAME_BYTES - 3;

/// Which name the shared checker is judging.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Output,
    Stem,
    Dir,
}

/// Invisible format characters refused in vault names (spec §14.1 row 6); ZWNJ/ZWJ stay allowed.
fn is_format_char(c: char) -> bool {
    matches!(c, '\u{200b}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}'
        | '\u{2066}'..='\u{2069}' | '\u{feff}' | '\u{2028}' | '\u{2029}')
}

/// The one ordered checker behind [`validate_name`], [`validate_stem`] and
/// [`validate_dir_segment`]; first failure wins. Vault modes add format characters and NFC;
/// `Stem` also refuses a leading dot and a `.md` ending (`x.md.md`).
fn check(name: &str, max: usize, mode: Mode) -> Result<(), NameError> {
    if name.len() > max {
        return Err(NameError::TooLong);
    }
    if js_trim(name).is_empty() {
        return Err(NameError::Empty);
    }
    if name.starts_with(' ') || name.ends_with(' ') {
        return Err(NameError::EdgeSpace);
    }
    if name.ends_with('.') || (mode == Mode::Stem && name.starts_with('.')) {
        return Err(NameError::EdgeDot);
    }
    if name.contains(['\\', '/', ':', '*', '?', '"', '<', '>', '|']) {
        return Err(NameError::IllegalChar);
    }
    let vault = mode != Mode::Output;
    if name
        .chars()
        .any(|c| c.is_control() || (vault && is_format_char(c)))
    {
        return Err(NameError::ControlChar);
    }
    if vault && !is_nfc(name) {
        return Err(NameError::NotNfc);
    }
    if is_reserved(name) {
        return Err(NameError::Reserved);
    }
    if mode == Mode::Stem && name.to_ascii_lowercase().ends_with(".md") {
        return Err(NameError::MdExtension);
    }
    Ok(())
}

/// Validate a final output file name; first failure wins: length, empty, edge space, edge dot
/// (trailing), illegal characters, control characters (C0, DEL, C1), reserved device names (whole
/// name or stem before the first `.`, an R-6 deviation from the whole-name TS rule). Length is
/// bytes (R-7).
///
/// # Examples
///
/// ```
/// use mda_core::naming::{validate_name, NameError};
/// assert_eq!(validate_name("Button.tsx"), Ok(()));
/// assert_eq!(validate_name("CON.txt"), Err(NameError::Reserved));
/// ```
pub fn validate_name(name: &str) -> Result<(), NameError> {
    check(name, MAX_NAME_BYTES, Mode::Output)
}

/// Validate a vault file-name **stem** (`.md` excluded): [`validate_name`]'s rules, plus a leading
/// dot is refused (hidden file) and a stem ending `.md` (any case) is refused (`x.md.md`). The
/// length cap is 252 bytes so `stem.md` fits 255. Check the stem, not the joined name: `trail.`
/// plus `.md` would pass as `trail..md`.
///
/// # Examples
///
/// ```
/// use mda_core::naming::{validate_stem, NameError};
/// assert_eq!(validate_stem("café"), Ok(()));
/// assert_eq!(validate_stem(".hidden"), Err(NameError::EdgeDot));
/// assert_eq!(validate_stem("x.MD"), Err(NameError::MdExtension));
/// ```
pub fn validate_stem(stem: &str) -> Result<(), NameError> {
    check(stem, MAX_STEM_BYTES, Mode::Stem)
}

/// Validate a new **directory segment** of a vault path (spec §14.1a): the stem rule without `md_extension`,
/// and a leading dot allowed.
///
/// # Examples
///
/// ```
/// assert!(mda_core::naming::validate_dir_segment("sub").is_ok());
/// ```
pub fn validate_dir_segment(seg: &str) -> Result<(), NameError> {
    check(seg, MAX_NAME_BYTES, Mode::Dir)
}

/// The default vault stem for a title: [`slugify`], cut to 252 bytes on a char boundary, trailing `-` trimmed, `untitled` when empty. Not validated: a result
/// like `con` is for [`validate_stem`] to refuse (reject, never sanitise).
///
/// # Examples
///
/// ```
/// use mda_core::naming::default_stem;
/// assert_eq!(default_stem("Button Group"), "button-group");
/// assert_eq!(default_stem("日本"), "日本");
/// ```
pub fn default_stem(title: &str) -> String {
    let mut s = slugify(title);
    s.truncate(s.floor_char_boundary(MAX_STEM_BYTES));
    s.truncate(s.trim_end_matches('-').len());
    if s.is_empty() {
        "untitled".to_owned()
    } else {
        s
    }
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

/// Node `path.parse(name).name`: the text before the last dot, unless that dot starts the name.
///
/// # Examples
///
/// ```
/// let _ = mda_core::naming::node_stem("Button.tsx");
/// ```
pub fn node_stem(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if i > 0 => name.get(..i).unwrap_or(name),
        _ => name,
    }
}

/// Node `path.extname(name).slice(1)`: the text after the last dot, unless that dot starts the name.
///
/// # Examples
///
/// ```
/// use mda_core::naming::node_ext;
/// assert_eq!(node_ext("a.tar.gz"), "gz");
/// assert_eq!(node_ext(".cursorrules"), "");
/// assert_eq!(node_ext("a."), "");
/// ```
pub fn node_ext(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if i > 0 => name.get(i + 1..).unwrap_or(""),
        _ => "",
    }
}

/// Stems the rule accepts; T4.2 creates each on disk.
///
/// # Examples
///
/// ```
/// use mda_core::naming::{ACCEPTED_STEMS, validate_stem};
/// assert!(ACCEPTED_STEMS.iter().all(|s| validate_stem(s).is_ok()));
/// ```
#[doc(hidden)]
pub const ACCEPTED_STEMS: &[&str] = &[
    "a#b",
    "x^y",
    "[z]",
    "日本語",
    "caf\u{e9}",
    "\u{dc}n\u{ef}c\u{f8}d\u{e9} \u{2713}",
    // 126 x U+00E9 = 252 bytes (a const cannot call `repeat`).
    concat!(
        "ééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééé",
        "ééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééééé"
    ),
];
