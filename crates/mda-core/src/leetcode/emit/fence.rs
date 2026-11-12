//! Composing a ```` ```yaml leetcode ```` config fence from key groups (W7 T7.5a).

/// The fence text: opener, each group (a key line plus its indented lines, `\n`-terminated), closer.
///
/// # Examples
///
/// ```
/// let f = mda_core::leetcode::emit::config_fence(&["function: twoSum\n".to_owned()]);
/// assert_eq!(f, "```yaml leetcode\nfunction: twoSum\n```\n");
/// ```
pub fn config_fence(groups: &[String]) -> String {
    let mut out = "```yaml leetcode\n".to_owned();
    for g in groups {
        out.push_str(g);
        if !g.ends_with('\n') {
            out.push('\n'); // a group is whole lines; never glue the next key onto it
        }
    }
    out.push_str("```\n");
    out
}

#[cfg(test)]
mod tests {
    use super::config_fence;
    use crate::leetcode::parse;

    #[test]
    fn opener_groups_closer() {
        let g = [
            "function: f\n".to_owned(),
            "params:\n  - name: a\n    type: int\n".to_owned(),
        ];
        assert_eq!(
            config_fence(&g),
            "```yaml leetcode\nfunction: f\nparams:\n  - name: a\n    type: int\n```\n"
        );
    }

    #[test]
    fn empty_is_opener_and_closer_and_a_missing_newline_is_added() {
        assert_eq!(config_fence(&[]), "```yaml leetcode\n```\n");
        assert_eq!(
            config_fence(&["returns: int".to_owned()]),
            "```yaml leetcode\nreturns: int\n```\n"
        );
    }

    #[test]
    fn the_parser_reads_it() {
        let f = config_fence(&["function: twoSum\n".to_owned()]);
        assert_eq!(parse(&format!("---\n\n---\n\n{f}")).function_name, "twoSum");
    }
}
