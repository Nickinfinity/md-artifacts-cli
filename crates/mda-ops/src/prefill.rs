//! `artifact.prefill` (create-from-source): turn a selection, a file's text or terminal text into
//! an [`ArtifactModel`] for the client's create form. Pure: the client passes the text; nothing is
//! read or written.

use mda_core::language::map_language_id;
use mda_core::model::{ArtifactModel, ModelBlock};
use mda_core::naming::{NameField, check_injection, validate_name};
use mda_core::registry::{ArtifactType, LanguageMode, OutputNameKey};

use crate::{Ctx, OpError, error};

/// Largest source text, in **bytes** (the extension counts UTF-16 units,
/// `explorer.capture.ts:27` — bytes are stricter on non-ASCII text; recorded deviation R-26).
pub const MAX_PREFILL_BYTES: usize = 512 * 1024;

/// Where the source text came from.
///
/// # Examples
///
/// ```
/// let s: mda_ops::prefill::PrefillSource = serde_json::from_str(r#""terminal""#).unwrap();
/// assert_eq!(s, mda_ops::prefill::PrefillSource::Terminal);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrefillSource {
    Selection,
    File,
    Terminal,
}

/// `artifact.prefill` params.
///
/// # Examples
///
/// ```
/// let r: mda_ops::prefill::PrefillRequest = serde_json::from_value(serde_json::json!(
///     {"source": "selection", "type": "Snippet", "text": "x"})).unwrap();
/// assert_eq!(r.text, "x");
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PrefillRequest {
    pub source: PrefillSource,
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,
    pub text: String,
    #[serde(default)]
    pub language_id: Option<String>,
    #[serde(default)]
    pub file_name: Option<String>,
}

/// `artifact.prefill`: build a create-form model from source text.
///
/// # Examples
///
/// ```
/// use mda_ops::{Ctx, prefill};
/// let req: prefill::PrefillRequest = serde_json::from_value(serde_json::json!(
///     {"source": "selection", "type": "Snippet", "text": "x"})).unwrap();
/// assert!(prefill::prefill(&Ctx::new(None), req).is_ok());
/// ```
pub fn prefill(ctx: &Ctx, req: PrefillRequest) -> Result<ArtifactModel, OpError> {
    let _ = ctx; // pure: no vault, no I/O
    let name = req.file_name.as_deref();
    if req.text.len() > MAX_PREFILL_BYTES {
        return Err(OpError::new(error::FILE_TOO_LARGE)
            .with("path", name.unwrap_or(""))
            .with("size", req.text.len().to_string())
            .with("max", MAX_PREFILL_BYTES.to_string()));
    }
    let info = req.artifact_type.info();
    let mut model = ArtifactModel {
        artifact_type: req.artifact_type,
        title: String::new(),
        description: String::new(),
        tags: vec![],
        extension: String::new(),
        provider: String::new(),
        model: String::new(),
        version: String::new(),
        env: String::new(),
        target: String::new(),
        blocks: vec![],
    };
    let mapped = map_language_id(req.language_id.as_deref().unwrap_or(""));
    let language = match req.source {
        PrefillSource::Terminal => "",
        _ => mapped,
    };
    let language = match info.language {
        // Hidden types never show a language picker, so the capture is overridden (TS captures).
        Some(l) if l.mode == LanguageMode::Hidden && req.source != PrefillSource::File => l.default,
        _ => language,
    };
    if req.source == PrefillSource::File {
        let name = name.ok_or_else(|| {
            OpError::new(error::OP_BAD_REQUEST).with("reason", "fileName required")
        })?;
        check_injection(name, NameField::FileName)?;
        validate_name(name)?;
        match info.output_name_key {
            Some(OutputNameKey::Target) => model.target = name.to_owned(),
            Some(OutputNameKey::Extension) => model.extension = node_ext(name).to_owned(),
            None => {}
        }
    }
    model.blocks.push(ModelBlock {
        language: language.to_owned(),
        code: req.text,
        ..ModelBlock::default()
    });
    Ok(model)
}

/// Node `path.extname(name).slice(1)`: the text after the last dot, unless that dot starts the name.
fn node_ext(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if i > 0 => name.get(i + 1..).unwrap_or(""),
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefill_source_is_lowercase() {
        assert!(serde_json::from_str::<PrefillSource>(r#""Selection""#).is_err());
    }

    #[test]
    fn oversize_text_is_refused() {
        let req = |n| PrefillRequest {
            source: PrefillSource::Selection,
            artifact_type: ArtifactType::Snippet,
            text: "x".repeat(n),
            language_id: None,
            file_name: None,
        };
        assert!(prefill(&Ctx::new(None), req(MAX_PREFILL_BYTES)).is_ok());
        let e = prefill(&Ctx::new(None), req(MAX_PREFILL_BYTES + 1)).unwrap_err();
        assert_eq!(e.code, error::FILE_TOO_LARGE);
    }

    #[test]
    fn node_extname_table() {
        for (n, want) in [
            ("a.tar.gz", "gz"),
            ("a.", ""),
            (".x", ""),
            ("a", ""),
            ("Button.tsx", "tsx"),
        ] {
            assert_eq!(node_ext(n), want, "{n}");
        }
    }
}
