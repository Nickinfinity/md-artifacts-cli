//! LeetCode exercises (`spec/LEETCODE_FILE_FORMAT.md`): the wire model (`model`) and the parser.
//! Module list:
//! orchestrator-owned (H4.0a); a task may turn a module it owns into `<name>/mod.rs`.

use crate::render::Warning;

mod bounds;
mod cases;
mod checks;
mod config;
pub mod emit;
mod fm_blocks;
mod frontmatter;
mod guard;
mod kind;
mod layout;
mod libs;
mod model;
mod packages;
mod patch;
mod program;
mod project;
mod rules;
mod safe_json;
mod sections;
mod shared;
mod skeleton;

pub use config::legacy_frontmatter_keys;
pub use frontmatter::{
    Discriminator, OrderViolation, discriminator, is_exercise, order_violation, summary,
};
pub use guard::{OutcomeExpect, outcome_guard, write_guard};
pub use layout::{LayoutRefusal, layout};
pub use model::*;
pub use patch::{insert_solved_meta, patch_status, prepend_attempt, validate_language};
pub use rules::frontmatter_rules;
pub use shared::{MAX_PARAM_CHARS, resolve_lang_id, truncate_param};
pub use skeleton::{SkeletonInput, skeleton, validate_tag, validate_title};

/// Parse an exercise file (TS `parseLeetCode`). Never fails: malformed input degrades to the spec
/// default plus a warning (D-24). Takes decoded text. The project grammar runs only for the
/// multi-file shapes, as in TS.
///
/// # Examples
///
/// ```
/// use mda_core::leetcode::{parse, LeetcodeType};
/// let e = parse("---\ntitle: Two Sum\nleetcodeType: package\n---\n\n# Solutions\n");
/// assert_eq!(e.title, "Two Sum");
/// assert_eq!(e.leetcode_type, LeetcodeType::Package);
/// assert!(e.files.is_some() && e.warnings.is_empty());
/// ```
pub fn parse(src: &str) -> LeetCodeExercise {
    let (fm_raw, body) = config::split_frontmatter(src);
    let blocks = config::extract_config_blocks(body);
    let config_text = format!("{}\n{}", config::without_body_set_keys(fm_raw), blocks.raw);
    let fm = frontmatter::parse_frontmatter(&config_text);
    let lines: Vec<&str> = config_text
        .split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect();
    let project = kind::is_multi_file(Some(fm.leetcode_type))
        .then(|| project::parse_project_artifact(&config_text, body));
    let (warnings, libs, program) = collect_warnings(
        project.as_ref(),
        fm_raw,
        blocks.warnings,
        &lines,
        fm.type_warning,
    );
    let (files, checks, packages, solution_files, unimplemented_kinds) = split_project(project);
    LeetCodeExercise {
        title: fm.title.unwrap_or_default(),
        leetcode_type: fm.leetcode_type,
        difficulty: fm.difficulty,
        function_name: fm.function_name.unwrap_or_default(),
        functions: fm.functions.map(drop_proto_key),
        algorithm: fm.algorithm,
        status: fm.status,
        params: fm.params,
        returns: fm.returns.unwrap_or_default(),
        description: sections::extract_description(body),
        examples: sections::extract_examples(body),
        tests: sections::extract_tests(body),
        final_tests: sections::extract_final_tests(body),
        test: fm.test,
        setups: sections::extract_setups(body),
        practice: fm.practice,
        solutions: sections::extract_solutions(body),
        attempts: sections::extract_attempts(body),
        tags: fm.tags.unwrap_or_default(),
        files,
        libs,
        solution_files,
        checks,
        packages,
        program,
        warnings,
        unimplemented_kinds,
    }
}

/// JS `out["__proto__"] = v` sets the prototype instead of a key, so TS never emits it; a Rust
/// map would keep it as data (spec §10.4 hostile cases).
fn drop_proto_key(mut m: OrderedMap<String>) -> OrderedMap<String> {
    m.0.retain(|(k, _)| k != "__proto__");
    m
}

type Slots = (Vec<Warning>, Option<LibSpec>, Option<ProgramConfig>);

/// Warnings in TS's slot order (project, legacy key, extractor, lib, program, type), plus the
/// `libs` / `program` values read on the way. A project carries its own `libs`; the other shapes
/// read them here, and an empty map is absent.
fn collect_warnings(
    project: Option<&project::ProjectSections>,
    fm_raw: &str,
    extractor: Vec<Warning>,
    lines: &[&str],
    type_warning: Option<Warning>,
) -> Slots {
    let mut warnings = project.map(|p| p.warnings.clone()).unwrap_or_default();
    for key in config::legacy_frontmatter_keys(fm_raw) {
        warnings.push(program::warning(
            crate::error::LEETCODE_FRONTMATTER_IGNORED_KEY,
            &[("key", &key)],
        ));
    }
    warnings.extend(extractor);
    let libs = match project {
        Some(p) => Some(p.libs.clone()),
        None => {
            let l = project::parse_lib_declarations(lines, &mut warnings);
            (!l.0.is_empty()).then_some(l)
        }
    };
    let program = program::parse_program(lines, &mut warnings);
    warnings.extend(type_warning);
    (warnings, libs, program)
}

type ProjectFields = (
    Option<Vec<FileSpec>>,
    Option<Vec<ProjectCheck>>,
    Option<Vec<PackageSpec>>,
    Option<Vec<FileSpec>>,
    Vec<String>,
);

/// The project-only exercise fields; all absent for a single-buffer exercise, and an empty
/// `packages:` is absent too.
fn split_project(project: Option<project::ProjectSections>) -> ProjectFields {
    match project {
        Some(p) => (
            Some(p.files),
            Some(p.checks),
            (!p.packages.is_empty()).then_some(p.packages),
            Some(p.solution_files),
            p.unimplemented_kinds,
        ),
        None => (None, None, None, None, vec![]),
    }
}
