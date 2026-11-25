//! Execution config blocks: `test:`, `practice:`, `program:`.

use serde::{Deserialize, Serialize};

/// The test-type axis: how a case is delivered and compared (spec §2.5.1). Wire names are TS's
/// kebab-case ids. `class` and `in-place` are reserved.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::TestTypeId;
/// assert_eq!(serde_json::to_value(TestTypeId::DomAssert).unwrap(), "dom-assert");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TestTypeId {
    Call,
    Program,
    Http,
    Build,
    DomAssert,
    CssAssert,
    Class,
    InPlace,
}

/// The `test:` block. Undeclared: `{ type: "call", timeoutMs: 5000 }`. No `rawType`: the wire
/// carries the collapsed value only.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{TestConfig, TestTypeId};
/// let t = TestConfig { r#type: TestTypeId::Call, timeout_ms: 5000 };
/// assert_eq!(serde_json::to_value(t).unwrap(), serde_json::json!({"type": "call", "timeoutMs": 5000}));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct TestConfig {
    pub r#type: TestTypeId,
    /// Per-case budget in ms, clamped to `[100, 60000]` by the parser.
    pub timeout_ms: u32,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            r#type: TestTypeId::Call,
            timeout_ms: 5000,
        }
    }
}

/// A practice-mode restriction id (spec §2.5.2).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::PracticeOptionId;
/// assert_eq!(serde_json::to_value(PracticeOptionId::NoAiAgents).unwrap(), "noAiAgents");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PracticeOptionId {
    NoCompletion,
    NoAiAgents,
    NoSnippets,
    NoParameterHints,
}

/// The `practice:` block. The fence key `timeLimit` is the wire's `timeLimitMinutes`.
/// Undeclared: options `[noCompletion, noAiAgents]`, no limit, unlocked.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::PracticeConfig;
/// let v = serde_json::to_value(PracticeConfig::default()).unwrap();
/// assert_eq!(v["timeLimitMinutes"], 0);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct PracticeConfig {
    pub options: Vec<PracticeOptionId>,
    /// Countdown minutes; `0` is unlimited.
    pub time_limit_minutes: u32,
    pub locked: bool,
}

impl Default for PracticeConfig {
    fn default() -> Self {
        Self {
            options: vec![PracticeOptionId::NoCompletion, PracticeOptionId::NoAiAgents],
            time_limit_minutes: 0,
            locked: false,
        }
    }
}

/// How a `program` candidate receives its case (spec §2.5.5).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::ProgramChannel;
/// assert_eq!(serde_json::to_value(ProgramChannel::Stdin).unwrap(), "stdin");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProgramChannel {
    Argv,
    Flags,
    Stdin,
}

/// The `program:` block; absent from the wire when undeclared.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{ProgramChannel, ProgramConfig};
/// let p = ProgramConfig { channel: ProgramChannel::Argv, entry: None, flags: None };
/// assert_eq!(serde_json::to_value(p).unwrap(), serde_json::json!({"channel": "argv"}));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
pub struct ProgramConfig {
    pub channel: ProgramChannel,
    /// Relative entry point, shape-guarded but not resolved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    /// Named flags in declared order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<Vec<String>>,
}
