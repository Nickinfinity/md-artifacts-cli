//! Shared test helper for the emitter tasks (W7 H7.pre): wrap emitted text in a real exercise, parse
//! it with the engine's parser, and read one top-level field back — the inverse oracle every
//! canonical emitter is tested against (`parse(emit(x)) == x` on that field).

use serde_json::Value;

use crate::leetcode::{LeetCodeDifficulty, LeetcodeType, SkeletonInput, parse, skeleton};

/// The skeleton of a `ty` exercise with `extra` appended after it (sections, fences), parsed.
pub(crate) fn parsed_with(ty: LeetcodeType, extra: &str) -> Value {
    let head = skeleton(&SkeletonInput {
        title: "T".into(),
        leetcode_type: ty,
        difficulty: LeetCodeDifficulty::Easy,
        algorithm: None,
        tags: vec![],
    });
    serde_json::to_value(parse(&format!("{head}{extra}"))).unwrap_or(Value::Null)
}

/// One top-level field of a parsed exercise (spec §10 camelCase key), `Null` when absent.
pub(crate) fn field(v: &Value, key: &str) -> Value {
    v.get(key).cloned().unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_config_fence_after_the_skeleton_feeds_the_model() {
        let v = parsed_with(
            LeetcodeType::Function,
            "\n```yaml leetcode\nfunction: twoSum\n```\n",
        );
        assert_eq!(field(&v, "functionName"), "twoSum");
    }
}
