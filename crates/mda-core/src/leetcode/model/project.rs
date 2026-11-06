//! `package` / `stack` exercise types: files, libs, checks, packages (spec §9).

use serde::Serialize;

use super::{OrderedMap, TestCase};

/// What the solver may do with a `## Files` entry.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::FileRole;
/// assert_eq!(serde_json::to_value(FileRole::Readonly).unwrap(), "readonly");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FileRole {
    Editable,
    Readonly,
    Hidden,
}

/// One file of a project tree. `path` is untrusted author input; containment is the writer's job.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{FileRole, FileSpec};
/// let f = FileSpec { path: "a.py".into(), language: "python".into(), role: FileRole::Editable, content: "".into() };
/// assert_eq!(serde_json::to_value(f).unwrap()["role"], "editable");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileSpec {
    pub path: String,
    /// Canonical language id from the fence info-string.
    pub language: String,
    pub role: FileRole,
    pub content: String,
}

/// Per-language dependency lists from `libs:`, keyed by canonical language id.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{LibSpec, OrderedMap};
/// let l: LibSpec = OrderedMap(vec![("python".to_owned(), vec!["numpy".to_owned()])]);
/// assert_eq!(serde_json::to_value(l).unwrap(), serde_json::json!({"python": ["numpy"]}));
/// ```
pub type LibSpec = OrderedMap<Vec<String>>;

/// Fields every check carries, flattened into each kind.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::CheckBase;
/// let b = CheckBase { name: "n".into(), cases: vec![], public_count: 0 };
/// assert_eq!(serde_json::to_value(b).unwrap(), serde_json::json!({"name": "n", "cases": [], "publicCount": 0}));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckBase {
    /// Unique within the artifact.
    pub name: String,
    pub cases: Vec<TestCase>,
    /// Leading entries of `cases` that came from `## Tests` (the rest are `## Final Tests`).
    pub public_count: usize,
}

/// Grades one exported function (`kind: "call"`).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{CheckBase, FunctionCheck};
/// let c = FunctionCheck { base: CheckBase { name: "n".into(), cases: vec![], public_count: 0 },
///     file: "a.py".into(), function: "f".into() };
/// assert_eq!(serde_json::to_value(c).unwrap()["function"], "f");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FunctionCheck {
    #[serde(flatten)]
    pub base: CheckBase,
    pub file: String,
    pub function: String,
}

/// Passes when a declared argv exits 0 (`kind: "build"`).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{BuildCheck, CheckBase};
/// let c = BuildCheck { base: CheckBase { name: "n".into(), cases: vec![], public_count: 0 },
///     argv: vec!["make".into()], dir: None };
/// assert_eq!(serde_json::to_value(c).unwrap()["argv"], serde_json::json!(["make"]));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BuildCheck {
    #[serde(flatten)]
    pub base: CheckBase,
    /// An argv array, never a command string.
    pub argv: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dir: Option<String>,
}

/// Mounts a component and asserts the DOM (`kind: "dom-assert"`).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{CheckBase, DomAssertCheck};
/// let c = DomAssertCheck { base: CheckBase { name: "n".into(), cases: vec![], public_count: 0 },
///     file: "a.jsx".into(), package: None };
/// assert_eq!(serde_json::to_value(c).unwrap()["file"], "a.jsx");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DomAssertCheck {
    #[serde(flatten)]
    pub base: CheckBase,
    pub file: String,
    /// The `packages:` entry whose port the component's `fetch` may reach.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
}

/// Asserts declared style only (`kind: "css-assert"`).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{CheckBase, CssAssertCheck};
/// let c = CssAssertCheck { base: CheckBase { name: "n".into(), cases: vec![], public_count: 0 },
///     file: "a.jsx".into(), package: Some("api".into()) };
/// assert_eq!(serde_json::to_value(c).unwrap()["package"], "api");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CssAssertCheck {
    #[serde(flatten)]
    pub base: CheckBase,
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
}

/// Sends requests to a booted package (`kind: "http"`).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{CheckBase, HttpCheck};
/// let c = HttpCheck { base: CheckBase { name: "n".into(), cases: vec![], public_count: 0 }, package: "api".into() };
/// assert_eq!(serde_json::to_value(c).unwrap()["package"], "api");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HttpCheck {
    #[serde(flatten)]
    pub base: CheckBase,
    pub package: String,
}

/// How a `package` / `stack` is graded: five kinds, tagged by `kind` as TS tags them.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{CheckBase, HttpCheck, ProjectCheck};
/// let c = ProjectCheck::Http(HttpCheck { base: CheckBase { name: "n".into(), cases: vec![], public_count: 0 }, package: "api".into() });
/// assert_eq!(serde_json::to_value(c).unwrap()["kind"], "http");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum ProjectCheck {
    Call(FunctionCheck),
    Build(BuildCheck),
    DomAssert(DomAssertCheck),
    CssAssert(CssAssertCheck),
    Http(HttpCheck),
}

/// Key order is TS's object literal: `name, kind, <kind fields>, cases, publicCount`, then the
/// optional `dir` / `package` last (serde's tagged enum would emit `kind` first).
impl Serialize for ProjectCheck {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut m = s.serialize_map(None)?;
        let (kind, base) = match self {
            Self::Call(c) => ("call", &c.base),
            Self::Build(c) => ("build", &c.base),
            Self::DomAssert(c) => ("dom-assert", &c.base),
            Self::CssAssert(c) => ("css-assert", &c.base),
            Self::Http(c) => ("http", &c.base),
        };
        m.serialize_entry("name", &base.name)?;
        m.serialize_entry("kind", kind)?;
        match self {
            Self::Call(c) => {
                m.serialize_entry("file", &c.file)?;
                m.serialize_entry("function", &c.function)?;
            }
            Self::Build(c) => m.serialize_entry("argv", &c.argv)?,
            Self::DomAssert(c) => m.serialize_entry("file", &c.file)?,
            Self::CssAssert(c) => m.serialize_entry("file", &c.file)?,
            Self::Http(c) => m.serialize_entry("package", &c.package)?,
        }
        m.serialize_entry("cases", &base.cases)?;
        m.serialize_entry("publicCount", &base.public_count)?;
        let (dir, package) = match self {
            Self::Build(c) => (c.dir.as_ref(), None),
            Self::DomAssert(c) => (None, c.package.as_ref()),
            Self::CssAssert(c) => (None, c.package.as_ref()),
            _ => (None, None),
        };
        if let Some(d) = dir {
            m.serialize_entry("dir", d)?;
        }
        if let Some(p) = package {
            m.serialize_entry("package", p)?;
        }
        m.end()
    }
}

/// One `packages:` entry: a process the stack boots.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::PackageSpec;
/// let p = PackageSpec { name: "api".into(), dir: "s".into(), install: vec![], start: vec!["run".into()],
///     ready: None, expose_as: None, depends_on: None };
/// assert_eq!(serde_json::to_value(p).unwrap()["start"], serde_json::json!(["run"]));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageSpec {
    pub name: String,
    /// Shape-guarded relative directory.
    pub dir: String,
    /// Argv array, never a command string.
    pub install: Vec<String>,
    /// Argv array; `${PORT}` is the only substitution admitted.
    pub start: Vec<String>,
    /// Extra stdout readiness substring.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ready: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub depends_on: Option<Vec<String>>,
    /// Variable name to loopback URL template.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expose_as: Option<OrderedMap<String>>,
}

/// One declarative instruction against a mounted component, tagged by `op`.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::RenderStep;
/// let s = RenderStep::Click { selector: "#b".into() };
/// assert_eq!(serde_json::to_value(s).unwrap(), serde_json::json!({"op": "click", "selector": "#b"}));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "op", rename_all = "lowercase")]
pub enum RenderStep {
    Click {
        selector: String,
    },
    Change {
        selector: String,
        value: String,
    },
    Text {
        selector: String,
    },
    Count {
        selector: String,
    },
    Attr {
        selector: String,
        name: String,
    },
    /// Declared style only, never a computed box.
    Style {
        selector: String,
        property: String,
    },
}
