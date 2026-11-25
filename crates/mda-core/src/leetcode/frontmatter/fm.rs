//! The frontmatter accumulator (TS `parseFrontmatter` / `FM`): scalar keys plus the five blocks of
//! `fm_blocks`, over one text (frontmatter, or frontmatter-sans-body-set-keys + config fences).

use crate::leetcode::config::kv_line;
use crate::leetcode::fm_blocks::{
    parse_functions_block, parse_params_block, parse_practice_block, parse_tags_block,
    parse_test_block,
};
use crate::leetcode::kind::{parse_difficulty, parse_status, resolve_leetcode_type};
use crate::leetcode::{
    LeetCodeDifficulty, LeetCodeStatus, LeetcodeType, OrderedMap, ParamDef, PracticeConfig,
    TestConfig,
};
use crate::parse::text::{js_lines, js_trim};
use crate::render::Warning;

/// Parsed frontmatter fields; optionals are `None` when undeclared (TS `FM`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Fm {
    pub title: Option<String>,
    pub difficulty: LeetCodeDifficulty,
    pub function_name: Option<String>,
    pub functions: Option<OrderedMap<String>>,
    pub algorithm: Option<String>,
    pub status: LeetCodeStatus,
    pub params: Vec<ParamDef>,
    pub returns: Option<String>,
    pub practice: PracticeConfig,
    pub test: TestConfig,
    pub tags: Option<Vec<String>>,
    /// Resolved once after the whole text is scanned: the declared value when known, else derived
    /// from the raw `test.type`.
    pub leetcode_type: LeetcodeType,
    /// The `leetcodeType:` scalar as written (last wins), known or not.
    pub declared_type: Option<String>,
    /// W02 `leetcode.type.unknown`, set only when a declared value fell back.
    pub type_warning: Option<Warning>,
}

impl Default for Fm {
    fn default() -> Self {
        Self {
            title: None,
            difficulty: LeetCodeDifficulty::Easy,
            function_name: None,
            functions: None,
            algorithm: None,
            status: LeetCodeStatus::Unsolved,
            params: vec![],
            returns: None,
            practice: PracticeConfig::default(),
            test: TestConfig::default(),
            tags: None,
            leetcode_type: LeetcodeType::Function,
            declared_type: None,
            type_warning: None,
        }
    }
}

/// TS `applyScalar`: invalid `difficulty`/`status` keep the default.
fn apply_scalar(fm: &mut Fm, key: &str, val: &str) {
    match key {
        "title" => fm.title = Some(val.to_owned()),
        "function" => fm.function_name = Some(val.to_owned()),
        "algorithm" => fm.algorithm = Some(val.to_owned()),
        "returns" => fm.returns = Some(val.to_owned()),
        "difficulty" => fm.difficulty = parse_difficulty(val).unwrap_or(fm.difficulty),
        "status" => fm.status = parse_status(val).unwrap_or(fm.status),
        _ => {}
    }
}

/// TS `parseFrontmatter`: scan the top level, dispatching the block keys; unknown keys (the
/// `artifactType` discriminator included) are not read.
pub(crate) fn parse_frontmatter(raw: &str) -> Fm {
    let mut fm = Fm::default();
    let lines: Vec<&str> = js_lines(raw).collect();
    let mut raw_test_type: Option<String> = None;
    let mut i = 0;
    while let Some(line) = lines.get(i) {
        let Some((key, val)) = kv_line(line).map(|(k, v)| (k, js_trim(v))) else {
            i += 1;
            continue;
        };
        i = match key {
            "params" => {
                let (v, next) = parse_params_block(&lines, i, val);
                fm.params = v;
                next
            }
            "practice" => {
                let (v, next) = parse_practice_block(&lines, i);
                fm.practice = v;
                next
            }
            "functions" => {
                let (v, next) = parse_functions_block(&lines, i);
                fm.functions = Some(v);
                next
            }
            "test" => {
                let (b, next) = parse_test_block(&lines, i);
                fm.test = b.test;
                raw_test_type = b.raw_type;
                next
            }
            "tags" => {
                let (v, next) = parse_tags_block(&lines, i, val);
                fm.tags = Some(v);
                next
            }
            "leetcodeType" => {
                fm.declared_type = Some(val.to_owned());
                i + 1
            }
            _ => {
                apply_scalar(&mut fm, key, val);
                i + 1
            }
        };
    }
    let (t, warning) = resolve_leetcode_type(fm.declared_type.as_deref(), raw_test_type.as_deref());
    fm.leetcode_type = t;
    fm.type_warning = warning;
    fm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalars_blocks_and_type_resolution_order_independent() {
        let raw = "leetcodeType: package\ntitle: T\nfunction: f\nreturns: int\ntest:\n  type: service\nparams:\n  - name: a\n    type: int\nfunctions:\n  py: g\ntags: [x]\ndifficulty: hard";
        let fm = parse_frontmatter(raw);
        assert_eq!(fm.title.as_deref(), Some("T"));
        assert_eq!(fm.function_name.as_deref(), Some("f"));
        assert_eq!(fm.returns.as_deref(), Some("int"));
        assert_eq!(fm.leetcode_type, LeetcodeType::Package);
        assert_eq!(fm.params.len(), 1);
        assert_eq!(
            fm.functions.map(|m| m.0),
            Some(vec![("python".to_owned(), "g".to_owned())])
        );
        assert_eq!(fm.difficulty, LeetCodeDifficulty::Hard);
        // the test: block is declared AFTER leetcodeType here and BEFORE in the next; same result
        let late = parse_frontmatter("test:\n  type: service\nleetcodeType: bogus");
        assert_eq!(late.leetcode_type, LeetcodeType::Stack);
        assert!(late.type_warning.is_some());
    }

    #[test]
    fn derives_from_a_test_block_and_defaults() {
        assert_eq!(
            parse_frontmatter("test:\n  type: project").leetcode_type,
            LeetcodeType::Package
        );
        assert_eq!(parse_frontmatter("").leetcode_type, LeetcodeType::Function);
        assert_eq!(parse_frontmatter(""), Fm::default());
        let d = parse_frontmatter("test:\n  timeoutMs: 300\ntest:\n  type: project");
        // a second test: block replaces the first wholesale, rawType included
        assert_eq!(d.test.timeout_ms, 5000);
    }

    #[test]
    fn near_miss_unknown_and_indented_keys_are_not_read() {
        let fm = parse_frontmatter("titel: x\n  title: y\nartifactType: LeetCode\n__proto__: z");
        assert_eq!(fm, Fm::default());
    }
}
