//! THE `.md` emitter (spec §1.4, §2.1, §3): an [`ArtifactModel`] in, file bytes out, vks bodies as
//! YAML only (D-8). Every output is re-parsed and compared to the model before it is returned
//! (W-11), so a model the format cannot carry is refused, never written. Ports
//! `artifact-serializer.service.ts` (extension @ `b368c20`).

use crate::error::{FieldReason, Unrepresentable, VarsError};
use crate::model::{ArtifactModel, ModelBlock, ParsedArtifact, ParsedVar};
use crate::parse::parse_from_content;
use crate::parse::text::{js_trim, js_trim_end};
use crate::registry::{ArtifactType, LanguageMode};
use crate::vks::{self, VksValue};

/// Frontmatter key emission order (`artifact-serializer.service.ts:17-19`, spec §1.4). The patcher
/// inserts an absent key in this order too.
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
    "tags",
    "env",
    "target",
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
    let out = emit(model).map_err(SerializeError::Vks)?;
    if out.len() > max_bytes {
        return Err(SerializeError::TooLarge { size: out.len() });
    }
    guard(model, &out)?;
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

fn emit(m: &ArtifactModel) -> Result<String, Unrepresentable> {
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
    let code = format!("\n```{lang}\n{}\n```\n", js_trim_end(&b.code));
    Ok(frontmatter(m, &lang) + &code + &vars_part(&b.vars)?)
}

/// Frontmatter in [`FRONTMATTER_KEY_ORDER`]; empty values are omitted, strings go through
/// [`single_line`]. Tags are emitted raw: the guard refuses any that do not survive the re-parse.
fn frontmatter(m: &ArtifactModel, lang: &str) -> String {
    let mut out = format!("---\nartifactType: {}\n", m.artifact_type.as_str());
    for key in FRONTMATTER_KEY_ORDER.iter().skip(1) {
        let value = match *key {
            "title" => single_line(&m.title),
            "description" => single_line(&m.description),
            "language" => lang.to_owned(),
            "extension" => single_line(&m.extension),
            "provider" => single_line(&m.provider),
            "model" => single_line(&m.model),
            "version" => single_line(&m.version),
            "env" => single_line(&m.env),
            "target" => single_line(&m.target),
            "tags" if !m.tags.is_empty() => format!("[{}]", m.tags.join(", ")),
            _ => String::new(),
        };
        if !value.is_empty() {
            out.push_str(&format!("{key}: {value}\n"));
        }
    }
    out + "---\n"
}

/// `\n```vks\n<body>```\n`; with no vars the empty fence a sub-set needs (`ts:181-210`).
fn vks_fence(vars: &[ParsedVar]) -> Result<String, Unrepresentable> {
    Ok(format!("\n```vks\n{}```\n", vks::emit_body(vars)?))
}

fn subset(b: &ModelBlock) -> Result<String, Unrepresentable> {
    let desc = if b.description.is_empty() {
        String::new()
    } else {
        format!("{}\n", b.description)
    };
    Ok(format!("\n## {}\n{desc}{}", b.heading, vks_fence(&b.vars)?))
}

fn multi_block(b: &ModelBlock, t: ArtifactType) -> Result<String, Unrepresentable> {
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
fn vars_part(vars: &[ParsedVar]) -> Result<String, Unrepresentable> {
    let kept: Vec<ParsedVar> = vars.iter().filter(|v| !is_empty_str(v)).cloned().collect();
    if kept.is_empty() {
        return Ok(String::new());
    }
    Ok(format!("\nvars:{}", vks_fence(&kept)?))
}

/// Re-parse `out` and compare it with `model` (W-11). The first differing field is the error.
fn guard(model: &ArtifactModel, out: &str) -> Result<(), SerializeError> {
    let t = model.artifact_type;
    let parsed = parse_from_content(out, &format!("{}/_.md", t.info().dir));
    let sections = if t == ArtifactType::Variables {
        variables_subsets(&model.blocks)
    } else {
        model.blocks.len() > 1
    };
    // A one-block model shows no `##` sections; extra ones were smuggled in by the code.
    let want_blocks = if sections { model.blocks.len() } else { 0 };
    if parsed.blocks.len() != want_blocks {
        return Err(refuse("blocks", FieldReason::RoundTrip));
    }
    let a = flatten(&canonical(model));
    let b = flatten(&canonical(&from_parsed(&parsed)));
    match a.iter().zip(&b).find(|(x, y)| x != y) {
        Some(((field, _), _)) => Err(refuse(field, FieldReason::RoundTrip)),
        None => Ok(()),
    }
}

/// The serializer's documented normalisations and nothing else; applied to both sides of the guard.
/// Vars are sorted by name because a re-parse puts detected tokens first (model order is not kept).
fn canonical(m: &ArtifactModel) -> ArtifactModel {
    let t = m.artifact_type;
    let variables = t == ArtifactType::Variables;
    let sections = if variables {
        variables_subsets(&m.blocks)
    } else {
        m.blocks.len() > 1
    };
    let mut c = m.clone();
    for s in [
        &mut c.title,
        &mut c.description,
        &mut c.extension,
        &mut c.provider,
        &mut c.model,
        &mut c.version,
        &mut c.env,
        &mut c.target,
    ] {
        *s = single_line(s);
    }
    c.tags = m
        .tags
        .iter()
        .map(|s| js_trim(s).to_owned())
        .filter(|s| !s.is_empty())
        .collect();
    if variables && !sections {
        c.blocks = match m.blocks.first() {
            Some(b) if !b.vars.is_empty() => vec![b.clone()],
            _ => vec![],
        };
    } else if m.blocks.is_empty() {
        c.blocks = vec![ModelBlock::default()];
    }
    for b in &mut c.blocks {
        if sections || variables {
            b.heading = js_trim(&b.heading).to_owned();
            b.description = js_trim(&b.description).to_owned();
        } else {
            b.heading.clear();
            b.description.clear();
        }
        b.language = lang_for(t, &b.language); // Variables → ""
        if !variables {
            b.code = js_trim_end(&b.code).to_owned();
            b.vars.retain(|v| !is_empty_str(v));
        }
        b.vars.sort_by(|x, y| x.name.cmp(&y.name));
    }
    c
}

/// `(path, value)` pairs; the first differing pair names the field.
fn flatten(m: &ArtifactModel) -> Vec<(String, String)> {
    let kv = |k: &str, v: String| (k.to_owned(), v);
    let mut v = vec![
        kv("artifactType", m.artifact_type.as_str().to_owned()),
        kv("title", m.title.clone()),
        kv("description", m.description.clone()),
        kv("extension", m.extension.clone()),
        kv("provider", m.provider.clone()),
        kv("model", m.model.clone()),
        kv("version", m.version.clone()),
        kv("env", m.env.clone()),
        kv("target", m.target.clone()),
        kv("tags", format!("{:?}", m.tags)),
        kv("blocks", m.blocks.len().to_string()),
    ];
    for (i, b) in m.blocks.iter().enumerate() {
        v.push(kv(&format!("blocks/{i}/heading"), b.heading.clone()));
        v.push(kv(
            &format!("blocks/{i}/description"),
            b.description.clone(),
        ));
        v.push(kv(&format!("blocks/{i}/language"), b.language.clone()));
        v.push(kv(&format!("blocks/{i}/code"), b.code.clone()));
        v.push(kv(&format!("blocks/{i}/vars"), format!("{:?}", b.vars)));
    }
    v
}

/// The model a parsed file corresponds to (TS `parsedToModel`, carrying all ten frontmatter
/// strings); `index`/`paths` are not carried (§8.7).
///
/// # Examples
///
/// ```
/// use mda_core::{parse::parse_from_content, serialize::from_parsed};
/// let p = parse_from_content("---\nartifactType: Snippet\n---\n", "Snippets/a.md");
/// assert_eq!(from_parsed(&p).artifact_type, p.frontmatter.artifact_type);
/// ```
pub fn from_parsed(p: &ParsedArtifact) -> ArtifactModel {
    let fm = &p.frontmatter;
    let s = |o: &Option<String>| o.clone().unwrap_or_default();
    let variables = fm.artifact_type == ArtifactType::Variables;
    let blocks = if !p.blocks.is_empty() {
        p.blocks
            .iter()
            .map(|b| ModelBlock {
                heading: b.heading.clone(),
                description: b.description.clone(),
                // A Variables block's "code" is its vks body, which the vars already carry.
                language: if variables {
                    String::new()
                } else {
                    s(&b.fence_lang)
                },
                code: if variables {
                    String::new()
                } else {
                    b.code.clone()
                },
                vars: b.vars.clone(),
            })
            .collect()
    } else if variables {
        if p.vars.is_empty() {
            vec![]
        } else {
            vec![ModelBlock {
                vars: p.vars.clone(),
                ..ModelBlock::default()
            }]
        }
    } else {
        vec![ModelBlock {
            language: s(&fm.language),
            code: p.code.clone(),
            vars: p.vars.clone(),
            ..ModelBlock::default()
        }]
    };
    ArtifactModel {
        artifact_type: fm.artifact_type,
        title: s(&fm.title),
        description: s(&fm.description),
        tags: fm.tags.clone().unwrap_or_default(),
        extension: s(&fm.extension),
        provider: s(&fm.provider),
        model: s(&fm.model),
        version: s(&fm.version),
        env: s(&fm.env),
        target: s(&fm.target),
        blocks,
    }
}
