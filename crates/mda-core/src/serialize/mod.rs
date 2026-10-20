//! THE `.md` emitter (spec §1.4, §2.1, §3, §7.6): an [`ArtifactModel`] in, file bytes out, vks
//! bodies as YAML only (D-8). Every output is re-parsed and compared to the model before it is
//! returned (W-11, `guard.rs`), so a model the format cannot carry is refused, never written. Ports
//! `artifact-serializer.service.ts` (extension @ `b368c20`); the frontmatter list style and the
//! flag / bare-body layouts are W7c deviations (Obsidian style, lossless round-trip).

mod guard;
mod payload;

pub use guard::from_parsed;

use crate::error::{FieldReason, Limit, Unrepresentable, VarsError};
use crate::model::{ArtifactModel, ModelBlock, ParsedVar, PayloadKind};
use crate::parse::DEFAULTS_INFO;
use crate::parse::text::{js_trim, js_trim_end};
use crate::registry::{ArtifactType, LanguageMode};
use crate::vks::{self, VksValue};

/// Frontmatter key emission order (spec §1.4; W7c: `tags` last, as Obsidian's property editor keeps
/// it, after `index` and `paths`, which are not in this list). The patcher inserts an absent key in
/// this order too.
///
/// # Examples
///
/// ```
/// assert_eq!(mda_core::serialize::FRONTMATTER_KEY_ORDER[0], "artifactType");
/// ```
pub const FRONTMATTER_KEY_ORDER: [&str; 11] = [
    "artifactType",
    "title",
    "description",
    "language",
    "extension",
    "provider",
    "model",
    "version",
    "env",
    "target",
    "tags",
];

/// The §1.4 single-line rule (`safeYamlValue`, `artifact-serializer.service.ts:328-330`): every
/// `\r\n` / `\r` / `\n` becomes one space, runs of spaces collapse, then JS `trim`. A newline would
/// otherwise inject a sibling frontmatter key on re-parse.
///
/// # Examples
///
/// ```
/// assert_eq!(mda_core::serialize::single_line(" a\r\nb   c\n"), "a b c");
/// ```
pub fn single_line(s: &str) -> String {
    let flat = s.replace("\r\n", " ").replace(['\r', '\n'], " ");
    let mut out = String::with_capacity(flat.len());
    let mut prev_space = false;
    for c in flat.chars() {
        if c == ' ' && prev_space {
            continue;
        }
        prev_space = c == ' ';
        out.push(c);
    }
    js_trim(&out).to_owned()
}

/// Why [`serialize`] refused a model.
///
/// # Examples
///
/// ```
/// use mda_core::serialize::SerializeError;
/// let e = SerializeError::TooLarge { size: 2 };
/// assert!(matches!(e, SerializeError::TooLarge { .. }));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SerializeError {
    /// A semantic vks check failed (`vks.*` code).
    Vars(VarsError),
    /// The emitter cannot write a value (`vks.unrepresentable`).
    Vks(Unrepresentable),
    /// The model cannot be carried by the format (`artifact.unrepresentable`); `field` is a path
    /// such as `title` or `blocks/0/code`.
    Field { field: String, reason: FieldReason },
    /// The output exceeds the caller's byte limit.
    TooLarge { size: usize },
}

/// Emit `model` as file bytes, at most `max_bytes` long, re-parsed and checked against the model.
///
/// Order: vks checks, multi-block refusal, emit, size, then the round-trip guard (W-11): the output
/// is parsed back and compared with the model under [`canonical`]'s documented normalisations, so
/// every model the format cannot carry is refused rather than written lossy.
///
/// # Examples
///
/// ```
/// use mda_core::{model::ArtifactModel, serialize::serialize};
/// let m: ArtifactModel = serde_json::from_str(r#"{"artifactType":"Snippet"}"#).unwrap();
/// assert!(serialize(&m, 1 << 20).unwrap().starts_with("---\nartifactType: Snippet\n"));
/// ```
pub fn serialize(model: &ArtifactModel, max_bytes: usize) -> Result<String, SerializeError> {
    for b in &model.blocks {
        vks::check_vars(&b.vars).map_err(SerializeError::Vars)?;
    }
    let t = model.artifact_type;
    if !t.info().multi_block && model.blocks.len() > 1 {
        return Err(refuse("blocks", FieldReason::MultiBlock));
    }
    let out = emit(model)?;
    if out.len() > max_bytes {
        return Err(SerializeError::TooLarge { size: out.len() });
    }
    guard::guard(model, &out)?;
    Ok(out)
}

fn refuse(field: &str, reason: FieldReason) -> SerializeError {
    SerializeError::Field {
        field: field.to_owned(),
        reason,
    }
}

/// A Variables file is sub-sets when it has several blocks or one headed block (TS `keepsHeading`,
/// `serializer.ts:54`); otherwise one flat, heading-less fence.
fn variables_subsets(blocks: &[ModelBlock]) -> bool {
    blocks.len() > 1 || blocks.first().is_some_and(|b| !b.heading.is_empty())
}

/// Fence info string for a block: locked/hidden types fix it, Variables have none (`ts:304-310`).
fn lang_for(t: ArtifactType, block_lang: &str) -> String {
    match t.info().language {
        Some(l) if l.mode != LanguageMode::Free => l.default.to_owned(),
        Some(_) => block_lang.to_owned(),
        None => String::new(),
    }
}

fn emit(m: &ArtifactModel) -> Result<String, SerializeError> {
    let t = m.artifact_type;
    if t == ArtifactType::Variables {
        let body = if variables_subsets(&m.blocks) {
            m.blocks.iter().map(subset).collect::<Result<String, _>>()?
        } else {
            match m.blocks.first() {
                Some(b) if !b.vars.is_empty() => vks_fence(&b.vars)?,
                _ => String::new(),
            }
        };
        return Ok(frontmatter(m, "") + &body);
    }
    if m.layout.payload == PayloadKind::Flags {
        return payload::flags(m);
    }
    if m.layout.payload == PayloadKind::Bare
        && m.blocks.len() <= 1
        && let Some(out) = payload::bare(m)?
    {
        return Ok(out);
    }
    if m.blocks.len() > 1 {
        let body = m
            .blocks
            .iter()
            .map(|b| multi_block(b, t))
            .collect::<Result<String, _>>()?;
        return Ok(frontmatter(m, "") + &body);
    }
    let empty = ModelBlock::default();
    let b = m.blocks.first().unwrap_or(&empty);
    let lang = lang_for(t, &b.language);
    // A whole-file artifact with nothing to write is frontmatter only: an empty fence would
    // re-parse as bare-body text (spec §1.4).
    if t.info().writes_file
        && m.blocks.len() <= 1
        && js_trim_end(&b.code).is_empty()
        && vars_part(&b.vars)?.is_empty()
    {
        return Ok(frontmatter(m, &lang));
    }
    let code = format!("\n```{lang}\n{}\n```\n", js_trim_end(&b.code));
    Ok(frontmatter(m, &lang) + &code + &vars_part(&b.vars)?)
}

/// Frontmatter in [`FRONTMATTER_KEY_ORDER`], `index` and `paths` before `tags`; empty values are
/// omitted, strings go through [`single_line`]. Lists are YAML block lists (Obsidian style), items
/// emitted raw: the guard refuses any that do not survive the re-parse.
fn frontmatter(m: &ArtifactModel, lang: &str) -> String {
    let mut out = format!("---\nartifactType: {}\n", m.artifact_type.as_str());
    for key in FRONTMATTER_KEY_ORDER.iter().skip(1) {
        let value = match *key {
            "tags" => String::new(), // a list, emitted last below
            "title" => single_line(&m.title),
            "description" => single_line(&m.description),
            "language" => lang.to_owned(),
            "extension" => single_line(&m.extension),
            "provider" => single_line(&m.provider),
            "model" => single_line(&m.model),
            "version" => single_line(&m.version),
            "env" => single_line(&m.env),
            "target" => single_line(&m.target),
            _ => String::new(),
        };
        if !value.is_empty() {
            out.push_str(&format!("{key}: {value}\n"));
        }
    }
    // Not in `FRONTMATTER_KEY_ORDER` (it ranks the patcher's scalar insertions): just before `tags`.
    if m.index {
        out.push_str("index: true\n");
    }
    out.push_str(&block_list("paths", &m.paths));
    out.push_str(&block_list("tags", &m.tags));
    out + "---\n"
}

/// `key:` then one `  - item` line per item; nothing for an empty list.
fn block_list(key: &str, items: &[String]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let lines: String = items.iter().map(|i| format!("  - {i}\n")).collect();
    format!("{key}:\n{lines}")
}

/// `\n```yaml VKs\n<body>```\n` ([`DEFAULTS_INFO`]); with no vars the empty fence a sub-set needs (`ts:181-210`).
fn vks_fence(vars: &[ParsedVar]) -> Result<String, SerializeError> {
    let body = vks::emit_body(vars).map_err(SerializeError::Vks)?;
    // Per fence, where the reader applies the same bound (X-15): a larger body could not be re-read.
    if body.len() > vks::MAX_BODY_BYTES {
        return Err(SerializeError::Vars(vks::limit_error(
            Limit::BodyBytes,
            vks::MAX_BODY_BYTES,
            0,
        )));
    }
    Ok(format!("\n```{DEFAULTS_INFO}\n{body}```\n"))
}

fn subset(b: &ModelBlock) -> Result<String, SerializeError> {
    let desc = if b.description.is_empty() {
        String::new()
    } else {
        format!("{}\n", b.description)
    };
    Ok(format!("\n## {}\n{desc}{}", b.heading, vks_fence(&b.vars)?))
}

fn multi_block(b: &ModelBlock, t: ArtifactType) -> Result<String, SerializeError> {
    let desc = if b.description.is_empty() {
        "\n".to_owned()
    } else {
        format!("{}\n\n", b.description)
    };
    let lang = lang_for(t, &b.language);
    Ok(format!(
        "\n## {}\n{desc}```{lang}\n{}\n```\n{}",
        b.heading,
        js_trim_end(&b.code),
        vars_part(&b.vars)?
    ))
}

fn is_empty_str(v: &ParsedVar) -> bool {
    matches!(&v.value, VksValue::Str(s) if s.is_empty())
}

/// The `vars:` section of a code block: only vars that carry a value (`ts:281-284`, spec §1.4);
/// a detected-but-empty token is not a default.
fn vars_part(vars: &[ParsedVar]) -> Result<String, SerializeError> {
    let kept: Vec<ParsedVar> = vars.iter().filter(|v| !is_empty_str(v)).cloned().collect();
    if kept.is_empty() {
        return Ok(String::new());
    }
    Ok(format!("\nvars:{}", vks_fence(&kept)?))
}
