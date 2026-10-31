//! Windows device names (spec §5.1).

/// Windows device names (spec §5.1): the part before the first dot, trailing spaces removed.
pub(super) fn is_reserved(name: &str) -> bool {
    let head = name.split('.').next().unwrap_or(name);
    let head = head.trim_end_matches(' ').to_ascii_lowercase();
    if ["con", "prn", "aux", "nul", "conin$", "conout$"].contains(&head.as_str()) {
        return true;
    }
    let n = head
        .strip_prefix("com")
        .or_else(|| head.strip_prefix("lpt"));
    n.is_some_and(|d| {
        let mut it = d.chars();
        matches!(
            (it.next(), it.next()),
            (Some('0'..='9' | '\u{b9}' | '\u{b2}' | '\u{b3}'), None)
        )
    })
}
