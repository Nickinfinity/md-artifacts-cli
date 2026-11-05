//! Render warnings and the render expansion limits (spec §10).

/// Render warning: a variable with no effective value, a missing field, or a field of a string.
pub const RENDER_UNKNOWN_VAR: &str = "render.unknown_var";
/// Render warning: `each` over a value that is not a list.
pub const RENDER_EACH_NOT_LIST: &str = "render.each_not_list";
/// Render warning: `each` with no matching `end`.
pub const RENDER_UNTERMINATED: &str = "render.unterminated";
/// Render warning: `end` with no matching `each`.
pub const RENDER_UNMATCHED_END: &str = "render.unmatched_end";
/// Render warning: `each` over a path it is already inside.
pub const RENDER_SELF_NESTED: &str = "render.self_nested";
/// Render warning: a token reaching a list or record outside a loop (Choice excepted).
pub const RENDER_NOT_SCALAR: &str = "render.not_scalar";
/// Render warning: `join` reaching a record.
pub const RENDER_JOIN_RECORD: &str = "render.join_record";
/// Render warning: `join` with an empty result.
pub const RENDER_JOIN_EMPTY: &str = "render.join_empty";

/// Every render warning code (spec §10). Warnings travel inside a successful render response, so
/// none of these is an error code (`ALL_CODES` excludes them).
///
/// # Examples
///
/// ```
/// assert_eq!(mda_core::error::RENDER_WARNING_CODES.len(), 8);
/// ```
pub const RENDER_WARNING_CODES: &[&str] = &[
    RENDER_UNKNOWN_VAR,
    RENDER_EACH_NOT_LIST,
    RENDER_UNTERMINATED,
    RENDER_UNMATCHED_END,
    RENDER_SELF_NESTED,
    RENDER_NOT_SCALAR,
    RENDER_JOIN_RECORD,
    RENDER_JOIN_EMPTY,
];

/// The `limit` vocabulary of `render.limit` (spec §10).
///
/// # Examples
///
/// ```
/// use mda_core::error::RenderLimit;
/// assert_eq!(RenderLimit::OutputBytes.as_str(), "output_bytes");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderLimit {
    OutputBytes,
    Iterations,
    Steps,
    Depth,
}

impl RenderLimit {
    /// The exact spec §10 spelling.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OutputBytes => "output_bytes",
            Self::Iterations => "iterations",
            Self::Steps => "steps",
            Self::Depth => "depth",
        }
    }
}

/// A render expansion limit was reached: the whole render is refused (never truncated).
///
/// # Examples
///
/// ```
/// use mda_core::error::{LimitExceeded, RenderLimit};
/// let e = LimitExceeded { limit: RenderLimit::Depth, max: 64 };
/// assert_eq!(e.limit.as_str(), "depth");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LimitExceeded {
    pub limit: RenderLimit,
    pub max: usize,
}
