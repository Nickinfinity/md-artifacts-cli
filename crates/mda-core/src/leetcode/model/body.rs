//! Body-section types: cases, setups, solutions, attempts, and the picker summary.

use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

use super::{LeetCodeDifficulty, LeetCodeStatus, LeetcodeType};

/// A string-keyed map that serializes as a JSON object in declared order (TS objects keep
/// insertion order and clients print them as written). Same technique as `VksRecord`.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::OrderedMap;
/// let m = OrderedMap(vec![("target".to_owned(), 1), ("nums".to_owned(), 2)]);
/// assert_eq!(serde_json::to_string(&m).unwrap(), r#"{"target":1,"nums":2}"#);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OrderedMap<V>(pub Vec<(String, V)>);

impl<V> OrderedMap<V> {
    /// Insert with JS object semantics: a repeated key keeps its **first position** and takes the
    /// **last value**, so no key is ever emitted twice (spec §10 intro).
    ///
    /// # Examples
    ///
    /// ```
    /// let mut m = mda_core::leetcode::OrderedMap(vec![]);
    /// m.insert("a", 1);
    /// m.insert("b", 2);
    /// m.insert("a", 3);
    /// assert_eq!(m.0, vec![("a".to_owned(), 3), ("b".to_owned(), 2)]);
    /// ```
    // ponytail: linear scan per insert; maps are a handful of keys — index by a HashMap if one grows.
    pub fn insert(&mut self, key: impl Into<String>, value: V) {
        let key = key.into();
        match self.0.iter_mut().find(|(k, _)| *k == key) {
            Some((_, v)) => *v = value,
            None => self.0.push((key, value)),
        }
    }
}

impl<V: Serialize> Serialize for OrderedMap<V> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            m.serialize_entry(k, v)?;
        }
        m.end()
    }
}

/// A JSON value as a test case carries it (spec §3.3: JSON's type system in YAML syntax).
/// Integers and floats are kept apart so `5` stays `5` on the wire.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::CaseValue;
/// assert_eq!(serde_json::to_value(CaseValue::Int(5)).unwrap(), 5);
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum CaseValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<CaseValue>),
    Object(OrderedMap<CaseValue>),
}

/// One `input`/`expected` case. `input` maps parameter name to argument.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{CaseValue, OrderedMap, TestCase};
/// let c = TestCase { input: OrderedMap(vec![("n".to_owned(), CaseValue::Int(2))]), expected: CaseValue::Int(4) };
/// assert_eq!(serde_json::to_value(c).unwrap(), serde_json::json!({"input": {"n": 2}, "expected": 4}));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TestCase {
    pub input: OrderedMap<CaseValue>,
    pub expected: CaseValue,
}

/// One `## Examples` line pair.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::Example;
/// let e = Example { input: "n = 2".into(), output: "4".into() };
/// assert_eq!(serde_json::to_value(e).unwrap(), serde_json::json!({"input": "n = 2", "output": "4"}));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Example {
    pub input: String,
    pub output: String,
}

/// Starter code for one language, from a `# Setup` `## <Language>` fence.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::ExerciseSetup;
/// let s = ExerciseSetup { language: "javascript".into(), code: "f()".into() };
/// assert_eq!(serde_json::to_value(s).unwrap(), serde_json::json!({"language": "javascript", "code": "f()"}));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExerciseSetup {
    /// Lower-cased fence language as written.
    pub language: String,
    pub code: String,
}

/// One stored solution from `# Solutions`.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::LeetCodeSolution;
/// let s = LeetCodeSolution { language: "java".into(), label: None, code: "x".into(), solved_at: None, duration: None };
/// assert_eq!(serde_json::to_value(s).unwrap(), serde_json::json!({"language": "java", "code": "x"}));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeetCodeSolution {
    pub language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub code: String,
    /// ISO timestamp of the last successful run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solved_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
}

/// One recorded run from `# Attempts`.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::Attempt;
/// let a = Attempt { language: "java".into(), at: "t".into(), duration: "1s".into(), passed: true,
///     big_o: Some("O(n)".into()), confidence: None, code: "x".into() };
/// assert_eq!(serde_json::to_value(a).unwrap()["bigO"], "O(n)");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attempt {
    /// Lower-cased `## <Language>` heading text.
    pub language: String,
    /// ISO-8601 timestamp.
    pub at: String,
    /// Human-readable elapsed time, e.g. `8m22s`.
    pub duration: String,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub big_o: Option<String>,
    /// `high`, `medium` or `low`, as written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<String>,
    pub code: String,
}

/// An attempt to append: [`Attempt`] without its language.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::AttemptEntry;
/// let a = AttemptEntry { at: "t".into(), duration: "1s".into(), passed: false, big_o: None, confidence: None, code: "x".into() };
/// assert_eq!(serde_json::to_value(a).unwrap()["passed"], false);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptEntry {
    pub at: String,
    pub duration: String,
    pub passed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub big_o: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<String>,
    pub code: String,
}

/// What the exercise picker needs: the frontmatter-only parse.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{LeetCodeDifficulty, LeetCodeStatus, LeetCodeSummary};
/// let s = LeetCodeSummary { title: "t".into(), leetcode_type: None, difficulty: LeetCodeDifficulty::Easy,
///     status: LeetCodeStatus::Unsolved, algorithm: None, tags: vec![] };
/// assert_eq!(serde_json::to_value(s).unwrap()["difficulty"], "easy");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeetCodeSummary {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub leetcode_type: Option<LeetcodeType>,
    pub difficulty: LeetCodeDifficulty,
    pub status: LeetCodeStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub algorithm: Option<String>,
    pub tags: Vec<String>,
}
