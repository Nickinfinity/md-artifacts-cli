//! The LeetCode wire model (`spec/LEETCODE_FILE_FORMAT.md` §10): the JSON a parsed exercise leaves
//! the engine as. Shapes and names are the TypeScript extension's `ParsedLeetCode` and friends, not
//! the fence keys. Every type also deserializes (W7 D-43: `leetcode.write` takes the model); `warnings` and
//! `unimplementedKinds` are accepted and ignored on input (Q-71).

mod body;
mod config;
mod de;
mod exercise;
mod layout;
mod project;

pub use body::*;
pub use config::*;
pub use exercise::*;
pub use layout::*;
pub use project::*;
