//! Body-section types: cases, setups, solutions, attempts, and the picker summary.

use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

use super::{LeetCodeDifficulty, LeetCodeStatus, LeetcodeType};

/// A string-keyed map that serializes as a JSON object in **JS object order** (TS builds plain
/// objects): array-index keys (`0`, `[1-9][0-9]*` ≤ 2³²−2) first, ascending, then every other key in
/// insertion order. The `Vec` keeps insertion order; only the wire is reordered (W4 review).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::OrderedMap;
/// let m = OrderedMap(vec![("target".to_owned(), 1), ("nums".to_owned(), 2)]);
/// assert_eq!(serde_json::to_string(&m).unwrap(), r#"{"target":1,"nums":2}"#);
/// let js = OrderedMap(vec![("b".to_owned(), 1), ("2".to_owned(), 2), ("1".to_owned(), 3)]);
/// assert_eq!(serde_json::to_string(&js).unwrap(), r#"{"1":3,"2":2,"b":1}"#);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OrderedMap<V>(pub Vec<(String, V)>);

impl<V> OrderedMap<V> {
    /// Insert with JS object semantics: a repeated key keeps its **first position** and takes the
    /// **last value**, so no key is ever emitted twice (spec §10 intro).
    ///
    /// Linear in the map's size: for a handful of keys only. A parser building a map from vault text
    /// uses [`OrderedMap::from_pairs`] (a hostile 100 000-key fence would be quadratic here).
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
    pub fn insert(&mut self, key: impl Into<String>, value: V) {
        let key = key.into();
        match self.0.iter_mut().find(|(k, _)| *k == key) {
            Some((_, v)) => *v = value,
            None => self.0.push((key, value)),
        }
    }
}

impl<V> OrderedMap<V> {
    /// Build from `(key, value)` pairs with JS object semantics in O(n): a repeated key keeps its
    /// **first position** and takes the **last value**.
    ///
    /// # Examples
    ///
    /// ```
    /// use mda_core::leetcode::OrderedMap;
    /// let m = OrderedMap::from_pairs([("a".to_owned(), 1), ("b".to_owned(), 2), ("a".to_owned(), 3)]);
    /// assert_eq!(m.0, vec![("a".to_owned(), 3), ("b".to_owned(), 2)]);
    /// ```
    pub fn from_pairs(pairs: impl IntoIterator<Item = (String, V)>) -> Self {
        let mut at: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        let mut out: Vec<(String, V)> = Vec::new();
        for (k, v) in pairs {
            match at.get(&k).and_then(|&i| out.get_mut(i)) {
                Some(slot) => slot.1 = v,
                None => {
                    at.insert(k.clone(), out.len());
                    out.push((k, v));
                }
            }
        }
        OrderedMap(out)
    }
}

/// A JS array-index key: `0`, or `[1-9][0-9]*` up to 2³²−2 (ECMAScript `CanonicalNumericIndex`).
fn array_index(k: &str) -> Option<u32> {
    let canonical = k == "0" || (!k.starts_with('0') && !k.is_empty());
    if !canonical || !k.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    k.parse::<u32>().ok().filter(|&n| n != u32::MAX)
}

impl<V: Serialize> Serialize for OrderedMap<V> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut index: Vec<(u32, &(String, V))> = self
            .0
            .iter()
            .filter_map(|e| array_index(&e.0).map(|n| (n, e)))
            .collect();
        index.sort_by_key(|(n, _)| *n);
        let rest = self.0.iter().filter(|e| array_index(&e.0).is_none());
        let mut m = s.serialize_map(Some(self.0.len()))?;
        for (k, v) in index.into_iter().map(|(_, e)| e).chain(rest) {
            m.serialize_entry(k, v)?;
        }
        m.end()
    }
}

/// A JSON value as a test case carries it (spec §3.3: JSON's type system in YAML syntax).
/// Integers and floats are kept apart so `5` stays `5` on the wire. A `Float` that JS would print
/// as an integer (integral, below 1e21) is written as that integer when it fits i64/u64, with JS's
/// digits (`Number#toString`: the shortest round-trip digits, zero-padded); NaN and ±∞ are `null`.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::CaseValue;
/// assert_eq!(serde_json::to_value(CaseValue::Int(5)).unwrap(), 5);
/// assert_eq!(serde_json::to_string(&CaseValue::Float(9007199254740994.0)).unwrap(), "9007199254740994");
/// assert_eq!(serde_json::to_string(&CaseValue::Float(1.2345678901234567e19)).unwrap(), "12345678901234567000");
/// assert_eq!(serde_json::to_string(&CaseValue::Float(2.5)).unwrap(), "2.5");
/// assert_eq!(serde_json::to_string(&CaseValue::Float(f64::NAN)).unwrap(), "null");
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum CaseValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<CaseValue>),
    Object(OrderedMap<CaseValue>),
}

/// JS `Number#toString` digits of an integral `x` with |x| < 1e21: the shortest round-trip digits
/// (Rust's `{:e}`, the same shortest form) followed by zeros.
fn js_integer_text(x: f64) -> String {
    let e = format!("{:e}", x.abs());
    let (mantissa, exp) = e.split_once('e').unwrap_or((&e, "0"));
    let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let point = exp.parse::<usize>().unwrap_or(0) + 1;
    let mut text = digits.chars().take(point).collect::<String>();
    text.extend(std::iter::repeat_n('0', point.saturating_sub(digits.len())));
    if x < 0.0 { format!("-{text}") } else { text }
}

// ponytail: JS `Number#toString` text only for integral floats that fit i64/u64; integral floats in
// (u64::MAX, 1e21) print as serde's exponent form (`2e+19`, JS `20000000000000000000`) and small
// non-integrals switch to exponent form at 1e-5, not JS's 1e-7 (same value either way). A byte-exact
// JS layout of the ryu digits is W7's (the emitter writes numbers into `.md` files).
fn serialize_float<S: Serializer>(x: f64, s: S) -> Result<S::Ok, S::Error> {
    if !x.is_finite() {
        return s.serialize_unit();
    }
    if x.fract() == 0.0 && x.abs() < 1e21 {
        let text = js_integer_text(x);
        if let Ok(i) = text.parse::<i64>() {
            return s.serialize_i64(i);
        }
        if let Ok(u) = text.parse::<u64>() {
            return s.serialize_u64(u);
        }
    }
    s.serialize_f64(x)
}

impl Serialize for CaseValue {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            CaseValue::Null => s.serialize_unit(),
            CaseValue::Bool(b) => s.serialize_bool(*b),
            CaseValue::Int(i) => s.serialize_i64(*i),
            CaseValue::Float(x) => serialize_float(*x, s),
            CaseValue::Str(t) => s.serialize_str(t),
            CaseValue::List(v) => v.serialize(s),
            CaseValue::Object(m) => m.serialize(s),
        }
    }
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
    pub code: String,
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
