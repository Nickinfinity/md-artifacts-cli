//! TS `utils/safe-json.ts` (a bare `JSON.parse` in a try): `None` on any error. serde_json's built-in
//! recursion limit is the depth cap (127 nested containers parse, 128 do not); `preserve_order` keeps JS key order and duplicate-key
//! semantics (first position, last value).

use serde_json::Value;

/// Parse JSON or `None`.
#[allow(dead_code)] // reason: seam, used by T4.2, T4.3, T4.4, T4.7
pub(crate) fn safe_json(s: &str) -> Option<Value> {
    serde_json::from_str::<Value>(s).ok()
}

#[cfg(test)]
mod tests {
    use super::safe_json;

    fn nested(depth: usize) -> String {
        format!("{}{}", "[".repeat(depth), "]".repeat(depth))
    }

    #[test]
    fn depth_cap_is_serde_json_recursion_limit() {
        assert!(safe_json(&nested(127)).is_some());
        assert!(safe_json(&nested(128)).is_none());
    }

    #[test]
    fn duplicate_keys_keep_first_position_last_value() {
        let v = safe_json(r#"{"a":1,"b":2,"a":3}"#).unwrap();
        assert_eq!(serde_json::to_string(&v).unwrap(), r#"{"a":3,"b":2}"#);
    }

    #[test]
    fn invalid_is_none() {
        assert!(safe_json("{").is_none());
        assert!(safe_json("").is_none());
    }
}
