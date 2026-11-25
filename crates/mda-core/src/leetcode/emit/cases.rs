//! Canonical case fences (W7 T7.3; the YAML emitter W4 Q-31 deferred).
//!
//! Form: YAML (spec §3.9, preferred, `input` as a block map, every value flow-style), but a ```` ```json ````
//! fence (spec §3.3, also valid) when any key is one the YAML reader drops (`__proto__`,
//! `constructor`, `prototype`): JSON reads them back as data, so it is the only exact form there.
//! Strings go through `case_quoted` (YAML) or serde_json (JSON). A value neither reader returns
//! unchanged is refused, never altered: a ```` ``` ```` run (would close the fence), a non-finite or
//! integral-looking float / int past 2^53 (reads back as another variant), nesting past 64 (Q-37).

use super::quote::case_quoted;
use crate::error::NotSerializableReason as Refuse;
use crate::leetcode::cases::{MAX_EXACT_INT, MAX_NESTING, is_safe_key};
use crate::leetcode::{CaseValue, TestCase};

/// Containers deep, or `Err` for a value the readers would not give back as written.
fn depth(v: &CaseValue) -> Result<usize, Refuse> {
    match v {
        CaseValue::Str(s) if s.contains("```") => Err(Refuse::Fence),
        CaseValue::Int(i) if i.unsigned_abs() > 1 << 53 => Err(Refuse::Other),
        CaseValue::Float(f) if !f.is_finite() || (f.fract() == 0.0 && f.abs() <= MAX_EXACT_INT) => {
            Err(Refuse::Other)
        }
        CaseValue::List(l) => l
            .iter()
            .try_fold(0, |m, e| Ok(m.max(depth(e)?)))
            .map(|d| d + 1),
        CaseValue::Object(o) => {
            o.0.iter()
                .try_fold(0, |m, (k, e)| {
                    if k.contains("```") {
                        return Err(Refuse::Fence);
                    }
                    Ok(m.max(depth(e)?))
                })
                .map(|d| d + 1)
        }
        _ => Ok(0),
    }
}

fn has_unsafe_key(v: &CaseValue) -> bool {
    match v {
        CaseValue::List(l) => l.iter().any(has_unsafe_key),
        CaseValue::Object(o) => {
            o.0.iter()
                .any(|(k, e)| !is_safe_key(k) || has_unsafe_key(e))
        }
        _ => false,
    }
}

/// `v` as one YAML flow value (always valid for `depth`-checked input).
fn flow(v: &CaseValue, out: &mut String) -> Result<(), Refuse> {
    match v {
        CaseValue::Str(s) => out.push_str(&case_quoted(s)),
        CaseValue::List(l) => {
            out.push('[');
            for (i, e) in l.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                flow(e, out)?;
            }
            out.push(']');
        }
        CaseValue::Object(o) => {
            out.push('{');
            for (i, (k, e)) in o.0.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&case_quoted(k));
                out.push_str(": ");
                flow(e, out)?;
            }
            out.push('}');
        }
        scalar => out.push_str(&serde_json::to_string(scalar).map_err(|_| Refuse::Other)?),
    }
    Ok(())
}

/// One `check="…"` value: the project reader takes `[^"]*` of a single info line.
fn check_attr(name: &str) -> Result<String, Refuse> {
    if name.contains(['"', '\n', '\r', '\u{2028}', '\u{2029}']) || name.contains("```") {
        return Err(Refuse::Other);
    }
    Ok(format!(" check=\"{name}\""))
}

/// One case fence holding `cases`, bound to `check` (a `check="…"` attribute) when given.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{CaseValue, OrderedMap, TestCase, emit::case_fence};
/// let c = TestCase::Call { input: OrderedMap(vec![("n".into(), CaseValue::Int(2))]), expected: CaseValue::Int(4) };
/// assert_eq!(
///     case_fence(&[c], None).unwrap(),
///     "```yaml\n- input:\n    \"n\": 2\n  expected: 4\n```\n"
/// );
/// ```
pub fn case_fence(cases: &[TestCase], check: Option<&str>) -> Result<String, Refuse> {
    let attr = check.map(check_attr).transpose()?.unwrap_or_default();
    let mut json = false;
    for c in cases {
        let (input, expected) = match c {
            // an http case is a verbatim object: JSON is the exact form, depth counts as one value
            TestCase::Http(o) => {
                let v = CaseValue::Object(o.clone());
                if depth(&v)? > MAX_NESTING {
                    return Err(Refuse::Other);
                }
                json = true;
                continue;
            }
            TestCase::Call { input, expected } => (CaseValue::Object(input.clone()), expected),
        };
        // block and flow nesting count separately (reader): `input` is block-level, its values flow
        if depth(&input)?.saturating_sub(1).max(depth(expected)?) > MAX_NESTING {
            return Err(Refuse::Other);
        }
        json |= has_unsafe_key(&input) || has_unsafe_key(expected);
    }
    let mut out = format!("```{}{attr}\n", if json { "json" } else { "yaml" });
    if json {
        if !cases.is_empty() {
            let rows = cases
                .iter()
                .map(|c| serde_json::to_string(c).map(|s| format!("  {s}")))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| Refuse::Other)?;
            out.push_str(&format!("[\n{}\n]\n", rows.join(",\n")));
        }
    } else {
        for c in cases {
            let TestCase::Call { input, expected } = c else {
                return Err(Refuse::Other); // unreachable: an http case set `json`
            };
            out.push_str("- input:");
            if input.0.is_empty() {
                out.push_str(" {}");
            }
            for (k, v) in &input.0 {
                out.push_str(&format!("\n    {}: ", case_quoted(k)));
                flow(v, &mut out)?;
            }
            out.push_str("\n  expected: ");
            flow(expected, &mut out)?;
            out.push('\n');
        }
    }
    out.push_str("```\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leetcode::LeetcodeType;
    use crate::leetcode::OrderedMap;
    use crate::leetcode::emit::testkit::{field, parsed_with};
    use serde_json::{Value, to_value};

    fn tc(input: Vec<(&str, CaseValue)>, expected: CaseValue) -> TestCase {
        let input = input.into_iter().map(|(k, v)| (k.to_owned(), v)).collect();
        TestCase::Call {
            input: OrderedMap(input),
            expected,
        }
    }

    /// The `finalTests` field of a minimal exercise holding the fence under `## Final Tests` (the
    /// skeleton already owns an empty `## Tests`).
    fn parse_back(fence: &str) -> Value {
        let v = parsed_with(
            LeetcodeType::Function,
            &format!("\n## Final Tests\n\n{fence}"),
        );
        field(&v, "finalTests")
    }

    fn assert_round_trip(cases: &[TestCase]) {
        let fence = case_fence(cases, None).unwrap();
        assert_eq!(parse_back(&fence), to_value(cases).unwrap(), "{fence}");
    }

    fn s(x: &str) -> CaseValue {
        CaseValue::Str(x.to_owned())
    }

    #[test]
    fn scalars_and_collections_round_trip() {
        let nested = CaseValue::List(vec![
            CaseValue::Object(OrderedMap(vec![("k".into(), CaseValue::List(vec![]))])),
            CaseValue::Object(OrderedMap(vec![])),
            CaseValue::Null,
        ]);
        assert_round_trip(&[]);
        assert_round_trip(&[
            tc(vec![], CaseValue::Null),
            tc(
                vec![
                    ("a", CaseValue::Int(-3)),
                    ("b", CaseValue::Float(2.5)),
                    ("c", CaseValue::Bool(true)),
                ],
                nested,
            ),
            tc(
                vec![("s", s("yes")), ("n", s("12")), ("e", s(""))],
                s("null"),
            ),
            tc(
                vec![("big", CaseValue::Float(1e30))],
                CaseValue::Float(-0.000_001),
            ),
        ]);
    }

    #[test]
    fn check_attribute_is_emitted_and_validated() {
        let f = case_fence(&[], Some("api filters")).unwrap();
        assert!(f.starts_with("```yaml check=\"api filters\"\n"));
        for bad in ["a\"b", "a\nb", "a\rb", "a```b"] {
            assert!(case_fence(&[], Some(bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn http_cases_round_trip_verbatim_beside_call_cases() {
        let obj = |kv: Vec<(&str, CaseValue)>| {
            OrderedMap(kv.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
        };
        let http = TestCase::Http(obj(vec![
            (
                "expect",
                CaseValue::Object(obj(vec![("status", CaseValue::Int(200))])),
            ),
            (
                "request",
                CaseValue::Object(obj(vec![("__proto__", s("x")), ("p", s("/"))])),
            ),
            ("note", s("n")),
        ]));
        assert_round_trip(&[
            http.clone(),
            tc(vec![("a", CaseValue::Int(1))], CaseValue::Null),
        ]);
        // `request` not an object, and nesting past the cap: no panic, refused or kept as written
        assert_round_trip(&[TestCase::Http(obj(vec![("request", CaseValue::Int(5))]))]);
        let mut deep = CaseValue::Null;
        for _ in 0..100 {
            deep = CaseValue::List(vec![deep]);
        }
        assert!(case_fence(&[TestCase::Http(obj(vec![("request", deep)]))], None).is_err());
    }

    const HOSTILE: &[&str] = &[
        "",
        "a\nb",
        "---",
        "a ---\n---",
        "\"",
        "'",
        "a\"b",
        "\\",
        "\\n",
        "\u{2028}",
        "\u{2029}",
        "#",
        "x # y",
        "a: b",
        "[a, b]",
        "{a: b}",
        "k: v, k2: v2",
        "\r\n",
        "\t",
        "\u{feff}",
        "- x",
        "|",
        "&a",
        "!!str",
        "``",
        "~",
        "1e3",
        "0051",
        "1:1",
        ".inf",
        "true",
        " ",
        "\u{0}",
    ];

    #[test]
    fn hostile_strings_round_trip_as_values_and_keys() {
        for h in HOSTILE {
            let c = tc(vec![(h, s(h)), ("l", CaseValue::List(vec![s(h)]))], s(h));
            let fence = case_fence(std::slice::from_ref(&c), None).unwrap();
            assert_eq!(
                parse_back(&fence),
                to_value(std::slice::from_ref(&c)).unwrap(),
                "{h:?}\n{fence}"
            );
        }
    }

    #[test]
    fn fence_runs_never_land_in_the_output() {
        for h in ["```", "a```b", "````"] {
            let r = case_fence(&[tc(vec![], s(h))], None);
            assert_eq!(r, Err(Refuse::Fence), "{h:?}");
            assert!(case_fence(&[tc(vec![(h, CaseValue::Null)], CaseValue::Null)], None).is_err());
        }
    }

    #[test]
    fn unsafe_keys_use_a_json_fence_and_round_trip() {
        for k in ["__proto__", "constructor", "prototype"] {
            let c = tc(
                vec![(k, CaseValue::Int(1))],
                CaseValue::Object(OrderedMap(vec![(k.into(), s("x"))])),
            );
            let fence = case_fence(std::slice::from_ref(&c), None).unwrap();
            assert!(fence.starts_with("```json\n"), "{fence}");
            assert_eq!(
                parse_back(&fence),
                to_value(std::slice::from_ref(&c)).unwrap()
            );
        }
    }

    #[test]
    fn values_a_reader_would_change_are_refused() {
        for bad in [
            CaseValue::Float(f64::NAN),
            CaseValue::Float(f64::INFINITY),
            CaseValue::Float(5.0),
            CaseValue::Int(i64::MAX),
        ] {
            assert!(case_fence(&[tc(vec![], bad)], None).is_err());
        }
    }

    fn nest(n: usize) -> CaseValue {
        (0..n).fold(CaseValue::Null, |v, _| CaseValue::List(vec![v]))
    }

    #[test]
    fn nesting_cap_is_64_flow_containers() {
        let ok = tc(vec![("x", nest(64))], nest(64));
        assert_round_trip(&[ok]);
        assert!(case_fence(&[tc(vec![("x", nest(65))], CaseValue::Null)], None).is_err());
        assert!(case_fence(&[tc(vec![], nest(65))], None).is_err());
    }

    fn conformance_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                conformance_files(&p, out);
            } else if p.to_string_lossy().ends_with(".expected.json") {
                out.push(p);
            }
        }
    }

    #[test]
    fn every_conformance_case_value_round_trips() {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/leetcode");
        let mut files = vec![];
        conformance_files(&root, &mut files);
        let mut suites = 0;
        for f in files {
            let v: Value = serde_json::from_str(&std::fs::read_to_string(&f).unwrap()).unwrap();
            for key in ["tests", "finalTests"] {
                let Some(raw) = v
                    .get(key)
                    .filter(|a| a.as_array().is_some_and(|a| !a.is_empty()))
                else {
                    continue;
                };
                let Ok(cases) = serde_json::from_value::<Vec<TestCase>>(raw.clone()) else {
                    continue;
                };
                let fence =
                    case_fence(&cases, None).unwrap_or_else(|e| panic!("{f:?} {key}: {e:?}"));
                assert_eq!(parse_back(&fence), to_value(&cases).unwrap(), "{f:?} {key}");
                suites += 1;
            }
        }
        assert!(suites >= 10, "only {suites} suites");
    }
}
