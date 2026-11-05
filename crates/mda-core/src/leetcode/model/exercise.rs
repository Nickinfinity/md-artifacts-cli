//! The exercise itself: TS `ParsedLeetCode` plus `unimplementedKinds`.

use serde::Serialize;

use super::{
    Attempt, Example, ExerciseSetup, FileSpec, LeetCodeSolution, LibSpec, OrderedMap, PackageSpec,
    PracticeConfig, ProgramConfig, ProjectCheck, TestCase, TestConfig,
};
use crate::render::Warning;

/// What an exercise is: one buffer, one package, or several (spec §2.1).
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::LeetcodeType;
/// assert_eq!(serde_json::to_value(LeetcodeType::Stack).unwrap(), "stack");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LeetcodeType {
    Function,
    Package,
    Stack,
}

/// Difficulty tier; any other written value is `easy`.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::LeetCodeDifficulty;
/// assert_eq!(serde_json::to_value(LeetCodeDifficulty::Hard).unwrap(), "hard");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LeetCodeDifficulty {
    Easy,
    Medium,
    Hard,
}

/// Solve status; any other written value is `unsolved`.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::LeetCodeStatus;
/// assert_eq!(serde_json::to_value(LeetCodeStatus::Solved).unwrap(), "solved");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LeetCodeStatus {
    Unsolved,
    Attempted,
    Solved,
}

/// One named parameter of the candidate function.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::ParamDef;
/// let p = ParamDef { name: "nums".into(), r#type: "number[]".into() };
/// assert_eq!(serde_json::to_value(p).unwrap(), serde_json::json!({"name": "nums", "type": "number[]"}));
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ParamDef {
    pub name: String,
    /// Generic type annotation, not validated.
    pub r#type: String,
}

/// One parsed exercise, as clients receive it. Optionals are absent when undeclared;
/// `warnings` and `unimplementedKinds` are always arrays. [`Default`] is the one authority
/// for an undeclared exercise: the parser starts from it.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::LeetCodeExercise;
/// let v = serde_json::to_value(LeetCodeExercise::default()).unwrap();
/// assert_eq!(v["difficulty"], "easy");
/// assert!(v.get("files").is_none());
/// ```
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LeetCodeExercise {
    pub title: String,
    pub leetcode_type: LeetcodeType,
    pub difficulty: LeetCodeDifficulty,
    pub function_name: String,
    /// Per-language override of `function_name`, keyed by canonical language id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub functions: Option<OrderedMap<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub algorithm: Option<String>,
    pub status: LeetCodeStatus,
    pub params: Vec<ParamDef>,
    pub returns: String,
    pub description: String,
    pub examples: Vec<Example>,
    pub tests: Vec<TestCase>,
    pub final_tests: Vec<TestCase>,
    pub test: TestConfig,
    pub setups: Vec<ExerciseSetup>,
    pub practice: PracticeConfig,
    pub solutions: Vec<LeetCodeSolution>,
    pub attempts: Vec<Attempt>,
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<FileSpec>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub libs: Option<LibSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solution_files: Option<Vec<FileSpec>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checks: Option<Vec<ProjectCheck>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub packages: Option<Vec<PackageSpec>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program: Option<ProgramConfig>,
    /// Author-facing parse problems that degraded to a default (`leetcode.*` codes).
    pub warnings: Vec<Warning>,
    /// Check kinds dropped as reserved, deduplicated, in first-seen order.
    pub unimplemented_kinds: Vec<String>,
}

impl Default for LeetCodeExercise {
    /// The undeclared exercise: spec §2.2 defaults, §2.3 derivation of a bare file.
    fn default() -> Self {
        Self {
            title: String::new(),
            leetcode_type: LeetcodeType::Function,
            difficulty: LeetCodeDifficulty::Easy,
            function_name: String::new(),
            functions: None,
            algorithm: None,
            status: LeetCodeStatus::Unsolved,
            params: vec![],
            returns: String::new(),
            description: String::new(),
            examples: vec![],
            tests: vec![],
            final_tests: vec![],
            test: TestConfig::default(),
            setups: vec![],
            practice: PracticeConfig::default(),
            solutions: vec![],
            attempts: vec![],
            tags: vec![],
            files: None,
            libs: None,
            solution_files: None,
            checks: None,
            packages: None,
            program: None,
            warnings: vec![],
            unimplemented_kinds: vec![],
        }
    }
}
