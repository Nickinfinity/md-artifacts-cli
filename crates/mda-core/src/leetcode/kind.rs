//! The leetcode-type axis and the small scalar vocabularies (TS `leetcode-type.helpers.ts`,
//! `types/leetcode-type.ts`, `DIFFICULTIES`/`TEST_TYPES`/`isMultiFile` of `types/constants.ts`).
//! Every lookup is a `match` or a slice search, never a property read: `__proto__` selects nothing.

use super::{LeetCodeDifficulty, LeetCodeStatus, LeetcodeType, TestTypeId, truncate_param};
use crate::error::LEETCODE_TYPE_UNKNOWN;
use crate::render::Warning;

/// How many trees solving an exercise materialises (TS `LeetcodeShape`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shape {
    Buffer,
    Tree,
    Trees,
}

/// One row of the TS `TEST_TYPES` table: the wire name, its id and whether anything executes it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct TestType {
    pub name: &'static str,
    pub id: TestTypeId,
    /// `false` = reserved (parses and validates, nothing runs it).
    #[allow(dead_code)] // reason: seam, used by T4.3
    pub implemented: bool,
}

/// The one test-type vocabulary, read by `test.type` and a check's `kind:` (TS `TEST_TYPES`).
pub(crate) const TEST_TYPES: &[TestType] = &[
    TestType {
        name: "call",
        id: TestTypeId::Call,
        implemented: true,
    },
    TestType {
        name: "program",
        id: TestTypeId::Program,
        implemented: true,
    },
    TestType {
        name: "http",
        id: TestTypeId::Http,
        implemented: true,
    },
    TestType {
        name: "build",
        id: TestTypeId::Build,
        implemented: true,
    },
    TestType {
        name: "dom-assert",
        id: TestTypeId::DomAssert,
        implemented: true,
    },
    TestType {
        name: "css-assert",
        id: TestTypeId::CssAssert,
        implemented: true,
    },
    TestType {
        name: "class",
        id: TestTypeId::Class,
        implemented: false,
    },
    TestType {
        name: "in-place",
        id: TestTypeId::InPlace,
        implemented: false,
    },
];

/// TS `DIFFICULTIES`.
#[allow(dead_code)] // reason: seam, used by T4.3
pub(crate) const DIFFICULTIES: [LeetCodeDifficulty; 3] = [
    LeetCodeDifficulty::Easy,
    LeetCodeDifficulty::Medium,
    LeetCodeDifficulty::Hard,
];

/// The wire id of a type (`leetcodeType:` value).
pub(crate) fn leetcode_type_name(t: LeetcodeType) -> &'static str {
    match t {
        LeetcodeType::Function => "function",
        LeetcodeType::Package => "package",
        LeetcodeType::Stack => "stack",
    }
}

/// TS `isLeetcodeType` as a parse: exactly `function`, `package` or `stack`.
pub(crate) fn parse_leetcode_type(s: &str) -> Option<LeetcodeType> {
    match s {
        "function" => Some(LeetcodeType::Function),
        "package" => Some(LeetcodeType::Package),
        "stack" => Some(LeetcodeType::Stack),
        _ => None,
    }
}

/// TS `shapeOf`.
#[allow(dead_code)] // reason: seam, used by T4.3
pub(crate) fn shape_of(t: LeetcodeType) -> Shape {
    match t {
        LeetcodeType::Function => Shape::Buffer,
        LeetcodeType::Package => Shape::Tree,
        LeetcodeType::Stack => Shape::Trees,
    }
}

/// TS `isMultiFile`: anything that is not a single buffer.
#[allow(dead_code)] // reason: seam, used by T4.3, T4.5
pub(crate) fn is_multi_file(t: Option<LeetcodeType>) -> bool {
    t.is_some_and(|t| shape_of(t) != Shape::Buffer)
}

/// TS `deriveLeetcodeType`: the legacy `test.type` scalars `project` → package, `service` → stack;
/// anything else (absent, a typo, `__proto__`) → function.
pub(crate) fn derive_leetcode_type(raw_test_type: Option<&str>) -> LeetcodeType {
    match raw_test_type {
        Some("project") => LeetcodeType::Package,
        Some("service") => LeetcodeType::Stack,
        _ => LeetcodeType::Function,
    }
}

/// TS `resolveLeetcodeType`: a declared known value wins; absent derives; a declared unknown value
/// derives and yields W02 `leetcode.type.unknown{declared, derived}` (`declared` truncated).
pub(crate) fn resolve_leetcode_type(
    declared: Option<&str>,
    raw_test_type: Option<&str>,
) -> (LeetcodeType, Option<Warning>) {
    let derived = derive_leetcode_type(raw_test_type);
    let Some(declared) = declared else {
        return (derived, None);
    };
    if let Some(t) = parse_leetcode_type(declared) {
        return (t, None);
    }
    let warning = Warning {
        code: LEETCODE_TYPE_UNKNOWN,
        params: [
            ("declared".to_owned(), truncate_param(declared)),
            ("derived".to_owned(), leetcode_type_name(derived).to_owned()),
        ]
        .into(),
    };
    (derived, Some(warning))
}

/// A `test.type` / check `kind:` value, `None` for anything outside [`TEST_TYPES`].
pub(crate) fn parse_test_type(s: &str) -> Option<TestTypeId> {
    TEST_TYPES.iter().find(|t| t.name == s).map(|t| t.id)
}

/// `difficulty:` value; anything else keeps the default.
pub(crate) fn parse_difficulty(s: &str) -> Option<LeetCodeDifficulty> {
    match s {
        "easy" => Some(LeetCodeDifficulty::Easy),
        "medium" => Some(LeetCodeDifficulty::Medium),
        "hard" => Some(LeetCodeDifficulty::Hard),
        _ => None,
    }
}

/// `status:` value; anything else keeps the default.
pub(crate) fn parse_status(s: &str) -> Option<LeetCodeStatus> {
    match s {
        "unsolved" => Some(LeetCodeStatus::Unsolved),
        "attempted" => Some(LeetCodeStatus::Attempted),
        "solved" => Some(LeetCodeStatus::Solved),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use LeetcodeType::*;

    #[test]
    fn derive_table_and_hostile_keys() {
        assert_eq!(derive_leetcode_type(None), Function);
        assert_eq!(derive_leetcode_type(Some("function")), Function);
        assert_eq!(derive_leetcode_type(Some("project")), Package);
        assert_eq!(derive_leetcode_type(Some("service")), Stack);
        assert_eq!(derive_leetcode_type(Some("call")), Function);
        assert_eq!(derive_leetcode_type(Some("__proto__")), Function);
        assert_eq!(derive_leetcode_type(Some("constructor")), Function);
    }

    #[test]
    fn declared_known_wins_and_never_derives() {
        assert_eq!(
            resolve_leetcode_type(Some("package"), Some("call")),
            (Package, None)
        );
        assert_eq!(
            resolve_leetcode_type(None, Some("project")),
            (Package, None)
        );
        assert_eq!(resolve_leetcode_type(None, None), (Function, None));
    }

    #[test]
    fn declared_unknown_derives_and_warns_with_params() {
        let (t, w) = resolve_leetcode_type(Some("bogus"), Some("project"));
        assert_eq!(t, Package);
        let w = w.expect("warning");
        assert_eq!(w.code, "leetcode.type.unknown");
        assert_eq!(w.params["declared"], "bogus");
        assert_eq!(w.params["derived"], "package");
        let (t, w) = resolve_leetcode_type(Some("__proto__"), None);
        assert_eq!(t, Function);
        assert_eq!(w.expect("warning").params["declared"], "__proto__");
        // a project is only a derivation input, never an id
        assert_eq!(resolve_leetcode_type(Some("project"), None).0, Function);
    }

    #[test]
    fn long_declared_value_is_truncated_in_the_param() {
        let huge = "x".repeat(10_000);
        let (t, w) = resolve_leetcode_type(Some(&huge), Some("service"));
        assert_eq!(t, Stack);
        assert_eq!(w.expect("warning").params["declared"].chars().count(), 200);
    }

    #[test]
    fn membership_is_exact() {
        assert_eq!(parse_leetcode_type("stack"), Some(Stack));
        assert_eq!(parse_leetcode_type("Stack"), None);
        assert_eq!(parse_leetcode_type("__proto__"), None);
        assert!(is_multi_file(Some(Package)) && is_multi_file(Some(Stack)));
        assert!(!is_multi_file(Some(Function)) && !is_multi_file(None));
    }

    #[test]
    fn scalar_tables() {
        assert_eq!(parse_difficulty("hard"), Some(LeetCodeDifficulty::Hard));
        assert_eq!(parse_difficulty("Hard"), None);
        assert_eq!(parse_status("attempted"), Some(LeetCodeStatus::Attempted));
        assert_eq!(parse_status("done"), None);
        assert_eq!(parse_test_type("dom-assert"), Some(TestTypeId::DomAssert));
        assert_eq!(parse_test_type("in-place"), Some(TestTypeId::InPlace));
        assert_eq!(parse_test_type("project"), None);
        assert_eq!(parse_test_type("__proto__"), None);
    }

    #[test]
    fn test_types_table_has_the_eight_ids_two_reserved() {
        assert_eq!(TEST_TYPES.len(), 8);
        assert_eq!(TEST_TYPES.iter().filter(|t| !t.implemented).count(), 2);
        assert_eq!(DIFFICULTIES.len(), 3);
    }
}
