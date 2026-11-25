//! Canonical config key groups for the frontmatter-shaped keys `function`/`functions`/`params`/
//! `returns`/`practice` (W7 T7.3; no `test:` line, Q-76).
//!
//! These keys are read by `fm.rs`/`fm_blocks.rs`, whose scalar reader is `js_trim` only: it never
//! unquotes. Neither `pair_quoted` (the `unquote_pair` inverse) nor `case_quoted` inverts it, so a
//! value is written bare when bare reads back, else refused; there is no quoting rule here.

use crate::error::NotSerializableReason as Refuse;
use crate::leetcode::{LeetCodeExercise, PracticeConfig, resolve_lang_id};
use crate::parse::text::js_trim;

/// A value the `fm` reader returns unchanged: non-empty, no edge whitespace, no line terminator
/// (`kv_line` refuses them), no fence run (it would close the config fence).
fn plain(s: &str) -> Result<&str, Refuse> {
    if s.contains("```") {
        return Err(Refuse::Fence);
    }
    let bad = s.is_empty() || js_trim(s) != s || s.contains(['\n', '\r', '\u{2028}', '\u{2029}']);
    if bad { Err(Refuse::Other) } else { Ok(s) }
}

/// A `functions:` key: `\w+`, already canonical (the reader runs `resolve_lang_id`), and not the
/// `__proto__` key the parser drops.
fn lang_key(k: &str) -> Result<&str, Refuse> {
    let word = !k.is_empty() && k.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if word && k != "__proto__" && resolve_lang_id(k) == k {
        Ok(k)
    } else {
        Err(Refuse::Other)
    }
}

/// The signature key groups (each a key line plus its indented lines, `\n`-terminated), only keys
/// whose value is non-default (Q-73), in spec order: `function`, `functions`, `params`, `returns`.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{LeetCodeExercise, emit::signature_groups};
/// let x = LeetCodeExercise { function_name: "twoSum".into(), ..Default::default() };
/// assert_eq!(signature_groups(&x).unwrap(), ["function: twoSum\n"]);
/// ```
pub fn signature_groups(x: &LeetCodeExercise) -> Result<Vec<String>, Refuse> {
    let mut out = vec![];
    if !x.function_name.is_empty() {
        out.push(format!("function: {}\n", plain(&x.function_name)?));
    }
    if let Some(m) = &x.functions {
        let mut g = "functions:\n".to_owned();
        for (k, v) in &m.0 {
            g.push_str(&format!("  {}: {}\n", lang_key(k)?, plain(v)?));
        }
        out.push(g);
    }
    if !x.params.is_empty() {
        let mut g = "params:\n".to_owned();
        for p in &x.params {
            g.push_str(&format!(
                "  - name: {}\n    type: {}\n",
                plain(&p.name)?,
                plain(&p.r#type)?
            ));
        }
        out.push(g);
    }
    if !x.returns.is_empty() {
        out.push(format!("returns: {}\n", plain(&x.returns)?));
    }
    Ok(out)
}

/// The `practice:` group, `None` when the value is the default.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{LeetCodeExercise, emit::practice_group};
/// let mut x = LeetCodeExercise::default();
/// assert_eq!(practice_group(&x).unwrap(), None);
/// x.practice.locked = true;
/// assert_eq!(practice_group(&x).unwrap().as_deref(), Some("practice:\n  locked: true\n"));
/// ```
pub fn practice_group(x: &LeetCodeExercise) -> Result<Option<String>, Refuse> {
    let (p, d) = (&x.practice, PracticeConfig::default());
    if *p == d {
        return Ok(None);
    }
    let mut g = "practice:\n".to_owned();
    if p.time_limit_minutes != 0 {
        g.push_str(&format!("  timeLimit: {}\n", p.time_limit_minutes));
    }
    if p.locked {
        g.push_str("  locked: true\n");
    }
    if p.options != d.options {
        let names = p
            .options
            .iter()
            .map(|o| match serde_json::to_value(o) {
                Ok(serde_json::Value::String(s)) => Ok(s),
                _ => Err(Refuse::Other),
            })
            .collect::<Result<Vec<_>, _>>()?;
        g.push_str(&format!("  options: [{}]\n", names.join(", ")));
    }
    Ok(Some(g))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leetcode::emit::testkit::{field, parsed_with};
    use crate::leetcode::{LeetcodeType, OrderedMap, ParamDef, PracticeOptionId};
    use serde_json::{Value, to_value};

    const KEYS: &[&str] = &["functionName", "functions", "params", "returns", "practice"];

    /// Wraps the groups in a config fence by hand (`config_fence` is T7.5a's) and parses.
    fn parse_back(x: &LeetCodeExercise) -> Result<Value, Refuse> {
        let mut text = signature_groups(x)?.concat();
        text.push_str(&practice_group(x)?.unwrap_or_default());
        Ok(parsed_with(
            LeetcodeType::Function,
            &format!("\n```yaml leetcode\n{text}```\n"),
        ))
    }

    fn assert_round_trip(x: &LeetCodeExercise) {
        let got = parse_back(x).unwrap();
        let want = to_value(x).unwrap();
        for k in KEYS {
            assert_eq!(field(&got, k), field(&want, k), "{k}");
        }
    }

    fn base() -> LeetCodeExercise {
        LeetCodeExercise::default()
    }

    #[test]
    fn each_key_round_trips() {
        let mut x = base();
        assert_round_trip(&x);
        x.function_name = "two_Sum".into();
        assert_round_trip(&x);
        x.functions = Some(OrderedMap(vec![
            ("python".into(), "two_sum".into()),
            ("constructor".into(), "x".into()),
        ]));
        assert_round_trip(&x);
        x.functions = Some(OrderedMap(vec![]));
        assert_round_trip(&x);
        x.params = vec![
            ParamDef {
                name: "nums".into(),
                r#type: "int[]".into(),
            },
            ParamDef {
                name: "m".into(),
                r#type: "Map<string, int>".into(),
            },
        ];
        x.returns = "int[]".into();
        assert_round_trip(&x);
        x.practice = PracticeConfig {
            options: vec![PracticeOptionId::NoSnippets],
            time_limit_minutes: 30,
            locked: true,
        };
        assert_round_trip(&x);
        x.practice.options = vec![];
        assert_round_trip(&x);
    }

    #[test]
    fn defaults_emit_nothing() {
        assert!(signature_groups(&base()).unwrap().is_empty());
        assert_eq!(practice_group(&base()).unwrap(), None);
    }

    const HOSTILE: &[&str] = &[
        "",
        "x",
        " x",
        "x ",
        "\"x\"",
        "'x'",
        "a\nb",
        "a\rb",
        "---",
        "```",
        "a```b",
        "``",
        "#x",
        "x # y",
        "a: b",
        "- x",
        "[a]",
        "\u{2028}",
        "\u{2029}",
        "__proto__",
        "constructor",
        "\\",
        "\t",
        "\u{feff}x",
        "x\u{a0}",
        "\"",
        "{",
        "|",
        ">",
        "&a",
        "*a",
        "!!str",
        "%",
        "@",
        "`",
    ];

    #[test]
    fn hostile_scalars_parse_back_or_are_refused() {
        for s in HOSTILE {
            let mut x = base();
            x.function_name = (*s).into();
            x.returns = (*s).into();
            x.params = vec![ParamDef {
                name: (*s).into(),
                r#type: (*s).into(),
            }];
            x.functions = Some(OrderedMap(vec![("python".into(), (*s).into())]));
            if let Ok(got) = parse_back(&x) {
                let want = to_value(&x).unwrap();
                for k in KEYS {
                    assert_eq!(field(&got, k), field(&want, k), "{s:?} {k}");
                }
            }
            for bad in ["a\nb", "a```b", " x", ""] {
                if *s == bad && !s.is_empty() {
                    assert!(signature_groups(&x).is_err(), "{s:?}");
                }
            }
        }
        let mut x = base();
        x.functions = Some(OrderedMap(vec![("__proto__".into(), "a".into())]));
        assert!(signature_groups(&x).is_err());
        x.functions = Some(OrderedMap(vec![("py".into(), "a".into())]));
        assert!(signature_groups(&x).is_err());
        x.functions = Some(OrderedMap(vec![("a b".into(), "a".into())]));
        assert!(signature_groups(&x).is_err());
    }

    #[test]
    fn a_fence_run_never_reaches_the_output() {
        let mut x = base();
        x.returns = "a```b".into();
        assert_eq!(signature_groups(&x), Err(Refuse::Fence));
    }
}
