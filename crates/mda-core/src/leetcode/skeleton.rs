//! The canonical skeleton of a new exercise (W6 D-28; the bytes are `spec/LEETCODE_FILE_FORMAT.md` §3.8).

use super::{LeetCodeDifficulty, LeetcodeType};
use crate::error::BadField;
use crate::parse::text::js_trim;
use crate::serialize::single_line;

/// What `skeleton` needs; the op validates every field first (`validate_title`, `validate_tag`).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{LeetCodeDifficulty, LeetcodeType, SkeletonInput};
/// let i = SkeletonInput { title: "Two Sum".into(), leetcode_type: LeetcodeType::Function,
///     difficulty: LeetCodeDifficulty::Easy, algorithm: None, tags: vec![] };
/// assert_eq!(i.title, "Two Sum");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkeletonInput {
    pub title: String,
    pub leetcode_type: LeetcodeType,
    pub difficulty: LeetCodeDifficulty,
    pub algorithm: Option<String>,
    pub tags: Vec<String>,
}

/// The skeleton file text: the bytes of spec §3.8 (LF, one trailing newline). `title` and
/// `algorithm` go through `single_line` (the parser takes the rest of the line, so no quoting);
/// `algorithm` only when given, `tags` only when non-empty, inline.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{LeetCodeDifficulty, LeetcodeType, SkeletonInput, skeleton};
/// let i = SkeletonInput { title: "Two Sum".into(), leetcode_type: LeetcodeType::Function,
///     difficulty: LeetCodeDifficulty::Easy, algorithm: None, tags: vec![] };
/// assert!(skeleton(&i).starts_with("---\nartifactType: LeetCode\nleetcodeType: function\n"));
/// ```
pub fn skeleton(input: &SkeletonInput) -> String {
    let kind = match input.leetcode_type {
        LeetcodeType::Function => "function",
        LeetcodeType::Package => "package",
        LeetcodeType::Stack => "stack",
    };
    let diff = match input.difficulty {
        LeetCodeDifficulty::Easy => "easy",
        LeetCodeDifficulty::Medium => "medium",
        LeetCodeDifficulty::Hard => "hard",
    };
    let mut out = format!(
        "---\nartifactType: LeetCode\nleetcodeType: {kind}\ntitle: {}\ndifficulty: {diff}\nstatus: unsolved\n",
        single_line(&input.title)
    );
    if let Some(a) = &input.algorithm {
        out.push_str(&format!("algorithm: {}\n", single_line(a)));
    }
    if !input.tags.is_empty() {
        out.push_str(&format!("tags: [{}]\n", input.tags.join(", ")));
    }
    out.push_str(
        "---\n\nDescribe the problem here.\n\n```yaml leetcode\n```\n\n## Examples\n\n## Tests\n\n# Setup\n\n# Solutions\n",
    );
    out
}

/// Every Cc char or a line/paragraph separator: the frontmatter line cannot hold it.
fn bad_char(c: char) -> bool {
    c.is_control() || c == '\u{2028}' || c == '\u{2029}'
}

/// A `title` or `algorithm` the skeleton can write (Q-55, Q-58): no control char (Cc), no
/// U+2028/U+2029, not empty after `single_line`.
///
/// # Examples
///
/// ```
/// assert!(mda_core::leetcode::validate_title("Two Sum").is_ok());
/// assert!(mda_core::leetcode::validate_title("  ").is_err());
/// ```
pub fn validate_title(title: &str) -> Result<(), BadField> {
    if title.chars().any(bad_char) || single_line(title).is_empty() {
        return Err(BadField::Title);
    }
    Ok(())
}

/// A tag the skeleton can write inline (Q-58): as a title, plus no `,` `[` `]` `#` and no
/// whitespace at either edge.
///
/// # Examples
///
/// ```
/// assert!(mda_core::leetcode::validate_tag("arrays").is_ok());
/// assert!(mda_core::leetcode::validate_tag("a,b").is_err());
/// ```
pub fn validate_tag(tag: &str) -> Result<(), BadField> {
    // Edges: the parser JS-trims (U+FEFF too) and strips a matching quote pair.
    let quote = |c: Option<char>| matches!(c, Some('"' | '\''));
    if tag
        .chars()
        .any(|c| bad_char(c) || matches!(c, ',' | '[' | ']' | '#'))
        || js_trim(tag) != tag
        || tag.is_empty()
        || quote(tag.chars().next())
        || quote(tag.chars().next_back())
    {
        return Err(BadField::Tag);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::leetcode::{LeetCodeStatus, parse};
    use crate::serialize::single_line;

    const TYPES: [LeetcodeType; 3] = [
        LeetcodeType::Function,
        LeetcodeType::Package,
        LeetcodeType::Stack,
    ];

    fn input(title: &str, t: LeetcodeType, alg: Option<&str>, tags: &[&str]) -> SkeletonInput {
        SkeletonInput {
            title: title.into(),
            leetcode_type: t,
            difficulty: LeetCodeDifficulty::Medium,
            algorithm: alg.map(Into::into),
            tags: tags.iter().map(|t| (*t).into()).collect(),
        }
    }

    #[test]
    fn golden_bytes() {
        let i = SkeletonInput {
            difficulty: LeetCodeDifficulty::Easy,
            ..input(
                "Two Sum",
                LeetcodeType::Function,
                Some("hash-map"),
                &["leetcode", "arrays"],
            )
        };
        let want = "---\nartifactType: LeetCode\nleetcodeType: function\ntitle: Two Sum\ndifficulty: easy\nstatus: unsolved\nalgorithm: hash-map\ntags: [leetcode, arrays]\n---\n\nDescribe the problem here.\n\n```yaml leetcode\n```\n\n## Examples\n\n## Tests\n\n# Setup\n\n# Solutions\n";
        assert_eq!(skeleton(&i), want);
        let bare = skeleton(&input("Two Sum", LeetcodeType::Stack, None, &[]));
        assert!(
            bare.contains("leetcodeType: stack\n")
                && !bare.contains("algorithm")
                && !bare.contains("tags")
        );
    }

    #[test]
    fn parses_back_for_every_type() {
        for t in TYPES {
            for (alg, tags) in [(None, vec![]), (Some("dp"), vec!["a", "b c"])] {
                let e = parse(&skeleton(&input("Two Sum", t, alg, &tags)));
                assert_eq!((e.title.as_str(), e.leetcode_type), ("Two Sum", t));
                assert_eq!(e.difficulty, LeetCodeDifficulty::Medium);
                assert_eq!(e.status, LeetCodeStatus::Unsolved);
                assert_eq!(e.algorithm.as_deref(), alg);
                assert_eq!(e.tags, tags);
                assert!(e.warnings.is_empty(), "{t:?}: {:?}", e.warnings);
            }
        }
    }

    #[test]
    fn valid_tags_parse_back_identical() {
        let tags = ["arrays", "two pointers", "a\"b", "it's", "x-y_z", "é"];
        assert!(tags.iter().all(|t| validate_tag(t).is_ok()));
        let e = parse(&skeleton(&input("T", LeetcodeType::Function, None, &tags)));
        assert_eq!(e.tags, tags);
    }

    #[test]
    fn hostile_title_parses_back_single_line() {
        for t in [
            "a: b",
            "# not a heading",
            "say \"hi\" 'x'",
            "a\nb: c\nstatus: solved",
            "  pad  ",
            "---",
        ] {
            let e = parse(&skeleton(&input(t, LeetcodeType::Function, None, &[])));
            assert_eq!(e.title, single_line(t), "{t:?}");
            assert_eq!(e.status, LeetCodeStatus::Unsolved, "{t:?}");
        }
    }

    #[test]
    fn validators_refuse() {
        assert!(validate_title("Two Sum: #1 \"x\"").is_ok());
        for bad in [
            "",
            "   ",
            "\n",
            "a\tb",
            "a\nb",
            "a\u{7f}b",
            "a\u{85}b",
            "a\u{2028}b",
            "a\u{2029}b",
            "a\0b",
        ] {
            assert_eq!(validate_title(bad), Err(BadField::Title), "{bad:?}");
        }
        assert!(validate_tag("arrays").is_ok() && validate_tag("two pointers").is_ok());
        for bad in [
            "",
            "a,b",
            "[a",
            "a]",
            "#a",
            " a",
            "a ",
            "a\nb",
            "a\u{2028}",
            "\u{a0}",
            "\"q\"",
            "'q'",
            "\"q",
            "q'",
            "\u{feff}a",
            "a\u{feff}",
        ] {
            assert_eq!(validate_tag(bad), Err(BadField::Tag), "{bad:?}");
        }
    }
}
