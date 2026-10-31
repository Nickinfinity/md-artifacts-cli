//! The round-trip guard (W-11): every emitted file is re-parsed and compared with its model under
//! [`canonical`]'s documented normalisations, layout and `paths` included (W7c).

use super::payload::flag_code;
use super::{SerializeError, is_empty_str, lang_for, refuse, single_line, variables_subsets};
use crate::error::FieldReason;
use crate::model::{ArtifactModel, ModelBlock, ParsedArtifact, PayloadKind};
use crate::parse::flags::trim_blank_lines;
use crate::parse::text::{js_trim, js_trim_end};
use crate::parse::{PAYLOAD_LANG, parse_from_content};
use crate::registry::ArtifactType;

/// Re-parse `out` and compare it with `model` (W-11). The first differing field is the error.
pub(super) fn guard(model: &ArtifactModel, out: &str) -> Result<(), SerializeError> {
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
    let list = |l: &[String]| -> Vec<String> {
        l.iter()
            .map(|s| js_trim(s).to_owned())
            .filter(|s| !s.is_empty())
            .collect()
    };
    c.tags = list(&m.tags);
    c.paths = list(&m.paths);
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
    canonical_layout(&mut c, sections);
    c
}

/// Layout normalisations (W7c, spec §7.6): notes are blank-line trimmed; a flagged file has one
/// region entry per block (missing ones are default), named by the block heading when there are
/// several; the payload language defaults to `markdown`; an empty bare body is the fenced shape.
fn canonical_layout(c: &mut ArtifactModel, sections: bool) {
    let t = c.artifact_type;
    let kind = c.layout.payload;
    let one_code = c.blocks.first().map_or("", |b| b.code.as_str());
    if kind == PayloadKind::Bare && !sections && js_trim(one_code).is_empty() {
        c.layout.payload = PayloadKind::Fence;
    }
    match c.layout.payload {
        // Notes or regions on a fenced / bare model are not normalised away: the guard refuses them.
        PayloadKind::Fence => {}
        PayloadKind::Bare => {
            for b in &mut c.blocks {
                b.code = js_trim(&b.code).to_owned();
                b.language = payload_language(t, &b.language);
            }
        }
        PayloadKind::Flags => {
            c.layout.notes = trim_blank_lines(&c.layout.notes);
            let n = c.blocks.len().max(1);
            if c.layout.regions.len() < n {
                c.layout.regions.resize(n, Default::default());
            }
            for (i, r) in c.layout.regions.iter_mut().enumerate() {
                r.notes = trim_blank_lines(&r.notes);
                r.name = match c.blocks.get(i) {
                    Some(b) if sections => b.heading.clone(),
                    _ => js_trim(&r.name).to_owned(),
                };
            }
            for b in &mut c.blocks {
                b.code = flag_code(&b.code);
                b.language = payload_language(t, if sections { "" } else { &b.language });
            }
        }
    }
}

/// The language a flags / bare payload reads back with: the type's fixed one, else the given one,
/// else `markdown`.
fn payload_language(t: ArtifactType, lang: &str) -> String {
    let l = lang_for(t, lang);
    if l.is_empty() {
        PAYLOAD_LANG.to_owned()
    } else {
        l
    }
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
        kv("index", m.index.to_string()),
        kv("tags", format!("{:?}", m.tags)),
        kv("paths", format!("{:?}", m.paths)),
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
    // After the blocks, so a refusal names the same field it did before W7c.
    v.push(kv("layout/payload", format!("{:?}", m.layout.payload)));
    v.push(kv("layout/notes", m.layout.notes.clone()));
    v.push(kv("layout/regions", m.layout.regions.len().to_string()));
    for (i, r) in m.layout.regions.iter().enumerate() {
        v.push(kv(&format!("layout/regions/{i}/name"), r.name.clone()));
        v.push(kv(&format!("layout/regions/{i}/notes"), r.notes.clone()));
        v.push(kv(&format!("layout/regions/{i}/rule"), r.rule.to_string()));
    }
    v
}

/// The model a parsed file corresponds to (TS `parsedToModel`, carrying all ten frontmatter
/// strings, `index`, and — W7c — `paths` and the payload layout, so nothing read is lost).
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
        index: fm.index == Some(true),
        paths: fm.paths.clone().unwrap_or_default(),
        blocks,
        layout: p.layout.clone(),
    }
}
