//! The LeetCode wire model (`spec/LEETCODE_FILE_FORMAT.md` §10): the JSON a parsed exercise leaves
//! the engine as. Shapes and names are the TypeScript extension's `ParsedLeetCode` and friends, not
//! the fence keys. Every type is `Serialize`-only: nothing in the engine deserializes them (D-41).

mod body;
mod config;
mod exercise;
mod project;

pub use body::*;
pub use config::*;
pub use exercise::*;
pub use project::*;
