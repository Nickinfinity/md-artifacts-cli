//! Section bounds (TS `leetcode-section-bounds.helpers.ts`).

use regex::Regex;

/// Byte bounds of a section body: `[heading_end, body_end)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SectionBounds {
    pub heading_end: usize,
    pub body_end: usize,
}

/// The body bounds of the section whose heading `heading` matches: from just after the match to
/// the first `boundary` line outside a ``` fence, or end of text (TS `sectionBounds`; the boundary
/// `RegExp` is a line predicate, tested per `\n`-split line with any `\r` kept). The shared reader
/// and writer primitive. `None` when the heading is absent.
#[allow(dead_code)] // reason: seam, used by T4.7
pub(crate) fn section_bounds(
    text: &str,
    heading: &Regex,
    boundary: &dyn Fn(&str) -> bool,
) -> Option<SectionBounds> {
    let heading_end = heading.find(text)?.end();
    Some(SectionBounds {
        heading_end,
        body_end: boundary_outside_fence(text, heading_end, boundary),
    })
}

/// Offset of the first `boundary` line at or after `from` that is not inside a ``` fence (a plain
/// toggle on lines opening with three backticks; `~~~` is not a fence here); `text.len()` when
/// none, so an unterminated fence runs to the end. A `from` off a char boundary also yields the end.
#[allow(dead_code)] // reason: seam, used by T4.7
pub(crate) fn boundary_outside_fence(
    text: &str,
    from: usize,
    boundary: &dyn Fn(&str) -> bool,
) -> usize {
    let Some(rest) = text.get(from..) else {
        return text.len();
    };
    let (mut offset, mut fenced) = (from, false);
    for line in rest.split('\n') {
        if line.starts_with("```") {
            fenced = !fenced;
        } else if !fenced && boundary(line) {
            return offset;
        }
        offset += line.len() + 1;
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn top(l: &str) -> bool {
        l.starts_with("# ")
    }
    fn sub(l: &str) -> bool {
        l.starts_with("# ") || l.starts_with("## ")
    }
    fn re(p: &str) -> Regex {
        crate::parse::text::compile(p)
    }
    fn body<'a>(t: &'a str, h: &str, b: &dyn Fn(&str) -> bool) -> Option<&'a str> {
        let s = section_bounds(t, &re(h), b)?;
        Some(&t[s.heading_end..s.body_end])
    }

    #[test]
    fn absent_heading_is_none() {
        assert_eq!(
            section_bounds("no heading here", &re(r"(?m)^# Setup\s*$"), &top),
            None
        );
    }

    #[test]
    fn stops_at_next_top_level_heading_keeping_children() {
        let h = r"(?m)^# Setup[ \t]*$";
        assert_eq!(
            body("# Setup\nbody line\n# Solutions\nx", h, &top),
            Some("\nbody line\n")
        );
        assert_eq!(
            body("# Setup\n## Java\ncode\n# Solutions", h, &top),
            Some("\n## Java\ncode\n")
        );
    }

    #[test]
    fn double_hash_boundary() {
        let t = "## Tests\n```json\n[]\n```\n## Final Tests\n";
        assert_eq!(
            body(t, r"(?m)^## Tests[ \t]*$", &sub),
            Some("\n```json\n[]\n```\n")
        );
    }

    #[test]
    fn runs_to_end_without_a_boundary() {
        let t = "# Setup\ntail content";
        let b = section_bounds(t, &re(r"(?m)^# Setup[ \t]*$"), &top).unwrap();
        assert_eq!(b.body_end, t.len());
    }

    #[test]
    fn fenced_boundary_shaped_lines_do_not_end_the_section() {
        let h = r"(?m)^# Setup[ \t]*$";
        let t = "# Setup\n```python\n# a comment at column zero\ncode\n```\n# Solutions\ntail";
        assert_eq!(
            body(t, h, &top),
            Some("\n```python\n# a comment at column zero\ncode\n```\n")
        );
        let t = "## Tests\n```markdown\n## not a heading\n```\n## Final Tests\n";
        assert_eq!(
            body(t, r"(?m)^## Tests[ \t]*$", &sub),
            Some("\n```markdown\n## not a heading\n```\n")
        );
        let t = "# Setup\n```py\n# c\n```\nprose\n# Solutions\nx";
        assert_eq!(body(t, h, &top), Some("\n```py\n# c\n```\nprose\n"));
    }

    #[test]
    fn unterminated_fence_swallows_the_rest() {
        let t = "# Setup\n```py\n# c\n# Solutions\nstill fenced";
        let b = section_bounds(t, &re(r"(?m)^# Setup[ \t]*$"), &top).unwrap();
        assert_eq!(b.body_end, t.len());
    }

    #[test]
    fn boundary_outside_fence_doc_example() {
        assert_eq!(
            boundary_outside_fence("# S\n```py\n# c\n```\n# T", 3, &top),
            18 // the TS doc comment says 17: off by one
        );
    }

    #[test]
    fn hostile_bad_offset_and_huge_input_do_not_panic() {
        // `from` inside a multibyte char or past the end: end of text, no panic.
        assert_eq!(boundary_outside_fence("é# x", 1, &top), "é# x".len());
        assert_eq!(boundary_outside_fence("abc", 99, &top), 3);
        let big = format!("# S\n{}\n# T", "x".repeat(1_000_000));
        assert_eq!(boundary_outside_fence(&big, 3, &top), big.len() - 3);
    }
}
