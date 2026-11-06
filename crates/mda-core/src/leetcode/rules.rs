//! The LeetCode structural rules 0-3 (D-29; TS `exercise-verify/frontmatter.rules.ts`), pure, on
//! decoded text. Each fires independently and every violation is reported; the verdict
//! (`vault.validate`: errors) is the caller's. `sanitizeUntrustedText` is dropped (Q-2).

use super::config::{kv_line, legacy_frontmatter_keys, split_frontmatter};
use super::{Discriminator, discriminator, order_violation};
use crate::error::{
    LEETCODE_RULE_BODY_KEY, LEETCODE_RULE_DISCRIMINATOR, LEETCODE_RULE_KEY_ORDER,
    LEETCODE_RULE_LEGACY_TYPE,
};
use crate::parse::text::{js_lines, js_trim};
use crate::render::Warning;

fn rule(code: &'static str, params: &[(&str, String)]) -> Warning {
    Warning {
        code,
        params: params
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
    }
}

/// Rule 1: a column-0 `type: leetcode` with no column-0 `artifactType:` beside it.
fn legacy_type(fm_raw: &str) -> bool {
    let (mut has_artifact_type, mut type_is_leetcode) = (false, false);
    for (key, val) in js_lines(fm_raw).filter_map(kv_line) {
        match key {
            "artifactType" => has_artifact_type = true,
            "type" if js_trim(val) == "leetcode" => type_is_leetcode = true,
            _ => {}
        }
    }
    type_is_leetcode && !has_artifact_type
}

/// Every rule violation of `src`, codes from `LEETCODE_RULE_CODES`, in rule order: 0 body-set keys
/// in frontmatter (`keys`, `", "`-joined), 1 legacy `type: leetcode` (no params), 2 key order
/// (`key`, `before`), 3 a discriminator that is present but neither `LeetCode` nor the lowercase
/// `leetcode` the migration handles (`found`, at most 200 chars; skipped when rule 1 fired). A
/// missing discriminator is not a rule violation (`leetcode.not_exercise` is the caller's).
///
/// # Examples
///
/// ```
/// let w = mda_core::leetcode::frontmatter_rules("---\ntitle: X\nartifactType: LeetCode\n---\n");
/// assert_eq!(w[0].code, "leetcode.rule.key_order");
/// assert!(mda_core::leetcode::frontmatter_rules("").is_empty());
/// ```
pub fn frontmatter_rules(src: &str) -> Vec<Warning> {
    let fm_raw = split_frontmatter(src).0;
    let mut out = vec![];
    let keys = legacy_frontmatter_keys(fm_raw);
    if !keys.is_empty() {
        out.push(rule(LEETCODE_RULE_BODY_KEY, &[("keys", keys.join(", "))]));
    }
    let legacy = legacy_type(fm_raw);
    if legacy {
        out.push(rule(LEETCODE_RULE_LEGACY_TYPE, &[]));
    }
    if let Some(v) = order_violation(fm_raw) {
        out.push(rule(
            LEETCODE_RULE_KEY_ORDER,
            &[("key", v.key), ("before", v.before)],
        ));
    }
    if let (false, Discriminator::Other(found)) = (legacy, discriminator(src)) {
        out.push(rule(LEETCODE_RULE_DISCRIMINATOR, &[("found", found)]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(src: &str) -> Vec<(&'static str, Vec<(String, String)>)> {
        frontmatter_rules(src)
            .into_iter()
            .map(|w| (w.code, w.params.into_iter().collect()))
            .collect()
    }
    fn codes(src: &str) -> Vec<&'static str> {
        frontmatter_rules(src).iter().map(|w| w.code).collect()
    }
    fn p(k: &str, v: &str) -> (String, String) {
        (k.to_owned(), v.to_owned())
    }

    #[test]
    fn clean_exercise_has_no_violation() {
        assert!(
            codes("---\nartifactType: LeetCode\nleetcodeType: function\ntitle: X\n---\nBody")
                .is_empty()
        );
        assert!(codes("").is_empty());
        assert!(codes("no frontmatter").is_empty());
    }

    #[test]
    fn rule0_body_key_names_every_key_once() {
        let r = rules(
            "---\nartifactType: LeetCode\nfunction: f\nparams:\n  - name: a\nfunction: g\n---\n",
        );
        assert_eq!(
            r,
            vec![(
                "leetcode.rule.body_key",
                vec![p("keys", "function, params")]
            )]
        );
    }

    #[test]
    fn rule1_legacy_type_only_without_artifact_type() {
        assert_eq!(
            rules("---\ntype: leetcode\ntitle: L\n---\n"),
            vec![("leetcode.rule.legacy_type", vec![])]
        );
        assert!(codes("---\ntype: leetcode\nartifactType: LeetCode\n---\n").is_empty());
        assert!(codes("---\ntype: notes\n---\n").is_empty());
        assert!(codes("---\ntype:   leetcode  \n---\n").contains(&"leetcode.rule.legacy_type"));
    }

    #[test]
    fn rule2_key_order_with_params() {
        let r = rules("---\ntitle: X\nartifactType: LeetCode\n---\n");
        assert_eq!(
            r,
            vec![(
                "leetcode.rule.key_order",
                vec![p("before", "title"), p("key", "artifactType")]
            )]
        );
    }

    #[test]
    fn rule3_discriminator_found_and_lowercase_exempt() {
        let r = rules("---\nartifactType: recipe\n---\n");
        assert_eq!(
            r,
            vec![("leetcode.rule.discriminator", vec![p("found", "recipe")])]
        );
        assert!(codes("---\nartifactType: leetcode\n---\n").is_empty());
        assert!(codes("---\ntitle: X\n---\n").is_empty());
        let q = rules("---\nartifactType: \"LeetCode\"\n---\n");
        assert_eq!(q[0].1, vec![p("found", "\"LeetCode\"")]);
        let long = format!("---\nartifactType: {}\n---\n", "x".repeat(900));
        assert_eq!(
            frontmatter_rules(&long)[0].params["found"].chars().count(),
            200
        );
    }

    #[test]
    fn every_rule_is_reported_in_order_not_first_only() {
        let src = "---\nfunction: f\ntitle: X\nartifactType: recipe\n---\n";
        assert_eq!(
            codes(src),
            [
                "leetcode.rule.body_key",
                "leetcode.rule.key_order",
                "leetcode.rule.discriminator"
            ]
        );
    }

    #[test]
    fn legacy_type_alone_reports_only_rule1_and_rule3_never_adds_to_it() {
        assert_eq!(
            codes("---\ntype: leetcode\n---\n"),
            ["leetcode.rule.legacy_type"]
        );
        assert_eq!(
            codes("---\ntype: leetcode\nparams: x\n---\n"),
            ["leetcode.rule.body_key", "leetcode.rule.legacy_type"]
        );
    }

    #[test]
    fn bom_decoded_text_and_crlf() {
        let t = crate::parse::decode(
            b"\xEF\xBB\xBF---\r\ntitle: X\r\nartifactType: LeetCode\r\n---\r\n",
        );
        assert_eq!(codes(&t), ["leetcode.rule.key_order"]);
    }

    #[test]
    fn rules_do_not_read_the_body() {
        assert!(
            codes("---\nartifactType: LeetCode\n---\n```yaml leetcode\nfunction: f\n```\n")
                .is_empty()
        );
    }
}
