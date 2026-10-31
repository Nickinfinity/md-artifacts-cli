//! THE artifact-type table: behaviour fields only (no UI prose). Ports the extension's
//! `constants.ts:21-121` rows and `artifact-type-config.service.ts:327-330`. `initialize` sends it to
//! clients verbatim.

/// The six artifact types. Serialized as the exact frontmatter spelling (`artifactType: Snippet`).
///
/// # Examples
///
/// ```
/// use mda_core::registry::ArtifactType;
/// assert_eq!(ArtifactType::from_name("AIPrompt"), Some(ArtifactType::AIPrompt));
/// assert_eq!(ArtifactType::Command.info().dir, "Commands");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ArtifactType {
    Snippet,
    AIAgentsConfig,
    Command,
    Template,
    Variables,
    AIPrompt,
}

/// Where a type's artifacts are offered for insertion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Context {
    Editor,
    Terminal,
    Explorer,
    All,
}

/// Whether the client lets the user pick a fence language.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LanguageMode {
    Free,
    Locked,
    Hidden,
}

/// A type's fence-language behaviour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Language {
    pub mode: LanguageMode,
    pub default: &'static str,
}

/// The frontmatter key that names a whole-file artifact's output file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OutputNameKey {
    Target,
    Extension,
}

/// One registry row.
#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeInfo {
    #[serde(rename = "type")]
    pub artifact_type: ArtifactType,
    pub dir: &'static str,
    pub contexts: &'static [Context],
    pub default_enabled: bool,
    pub writes_file: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_name_key: Option<OutputNameKey>,
    pub multi_block: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<Language>,
}

/// Every type, in the extension's order.
///
/// # Examples
///
/// ```
/// assert_eq!(mda_core::registry::TYPES.len(), 6);
/// assert_eq!(mda_core::registry::TYPES[0].dir, "Snippets");
/// ```
pub static TYPES: [TypeInfo; 6] = [
    TypeInfo {
        artifact_type: ArtifactType::Snippet,
        dir: "Snippets",
        contexts: &[Context::Editor],
        default_enabled: true,
        writes_file: false,
        output_name_key: None,
        multi_block: true,
        language: Some(Language {
            mode: LanguageMode::Free,
            default: "",
        }),
    },
    TypeInfo {
        artifact_type: ArtifactType::AIAgentsConfig,
        dir: "AIAgentsConf",
        contexts: &[Context::Explorer],
        default_enabled: true,
        writes_file: true,
        output_name_key: Some(OutputNameKey::Target),
        multi_block: true,
        language: Some(Language {
            mode: LanguageMode::Free,
            default: "",
        }),
    },
    TypeInfo {
        artifact_type: ArtifactType::Command,
        dir: "Commands",
        contexts: &[Context::Terminal],
        default_enabled: false,
        writes_file: false,
        output_name_key: None,
        multi_block: true,
        language: Some(Language {
            mode: LanguageMode::Locked,
            default: "bash",
        }),
    },
    TypeInfo {
        artifact_type: ArtifactType::Template,
        dir: "Templates",
        contexts: &[Context::Explorer],
        default_enabled: false,
        writes_file: true,
        output_name_key: Some(OutputNameKey::Extension),
        multi_block: false,
        language: Some(Language {
            mode: LanguageMode::Free,
            default: "",
        }),
    },
    // `multi_block` = sub-sets (spec §5); no fence language.
    TypeInfo {
        artifact_type: ArtifactType::Variables,
        dir: "Variables",
        contexts: &[Context::All],
        default_enabled: false,
        writes_file: false,
        output_name_key: None,
        multi_block: true,
        language: None,
    },
    TypeInfo {
        artifact_type: ArtifactType::AIPrompt,
        dir: "AIPrompts",
        contexts: &[Context::Editor, Context::Terminal],
        default_enabled: true,
        writes_file: false,
        output_name_key: None,
        multi_block: true,
        language: Some(Language {
            mode: LanguageMode::Hidden,
            default: "markdown",
        }),
    },
];

impl ArtifactType {
    /// This type's registry row.
    pub fn info(self) -> &'static TypeInfo {
        match self {
            Self::Snippet => &TYPES[0],
            Self::AIAgentsConfig => &TYPES[1],
            Self::Command => &TYPES[2],
            Self::Template => &TYPES[3],
            Self::Variables => &TYPES[4],
            Self::AIPrompt => &TYPES[5],
        }
    }

    /// The exact PascalCase name, the inverse of [`ArtifactType::from_name`].
    ///
    /// # Examples
    ///
    /// ```
    /// use mda_core::registry::{ArtifactType, TYPES};
    /// for t in &TYPES {
    ///     let a = t.artifact_type;
    ///     assert_eq!(ArtifactType::from_name(a.as_str()), Some(a));
    /// }
    /// ```
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Snippet => "Snippet",
            Self::AIAgentsConfig => "AIAgentsConfig",
            Self::Command => "Command",
            Self::Template => "Template",
            Self::Variables => "Variables",
            Self::AIPrompt => "AIPrompt",
        }
    }

    /// The type named exactly `s` (case-sensitive, as `parser.service.ts:12,160`).
    pub fn from_name(s: &str) -> Option<Self> {
        Some(match s {
            "Snippet" => Self::Snippet,
            "AIAgentsConfig" => Self::AIAgentsConfig,
            "Command" => Self::Command,
            "Template" => Self::Template,
            "Variables" => Self::Variables,
            "AIPrompt" => Self::AIPrompt,
            _ => return None,
        })
    }
}

/// The type whose directory is `dir`, ASCII case-insensitive (TS `toLowerCase`; every dir is ASCII).
///
/// # Examples
///
/// ```
/// use mda_core::registry::{type_for_dir, ArtifactType};
/// assert_eq!(type_for_dir("commands"), Some(ArtifactType::Command));
/// assert_eq!(type_for_dir("Other"), None);
/// ```
pub fn type_for_dir(dir: &str) -> Option<ArtifactType> {
    TYPES
        .iter()
        .find(|t| t.dir.eq_ignore_ascii_case(dir))
        .map(|t| t.artifact_type)
}
