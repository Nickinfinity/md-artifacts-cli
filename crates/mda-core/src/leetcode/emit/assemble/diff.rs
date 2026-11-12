//! What differs between the parse of the layout's text and the request (Q-70). Both sides go through
//! `to_value`, never `==` on the model: `Float(200.0)` and `Int(200)` print alike.

use serde_json::Value;

use crate::error::NotSerializableReason as Why;
use crate::leetcode::{LayoutField, LeetCodeExercise};

/// The keys frontmatter carries (`skeleton()`'s form); one change re-emits the whole block run.
pub(super) const FM_KEYS: &[&str] = crate::leetcode::config::CANONICAL_ORDER;

/// The old and new model as JSON values.
pub(super) struct Diff {
    old: Value,
    new: Value,
    /// Bytes of the new model as JSON: the output bound's yardstick.
    pub new_len: usize,
}

/// `cases`/`publicCount` follow the `## Tests` fences, not the config (T7.5a ruling 5).
fn strip_checks(v: &mut Value) {
    if let Some(Value::Array(a)) = v.get_mut("checks") {
        for c in a {
            if let Value::Object(m) = c {
                m.remove("cases");
                m.remove("publicCount");
            }
        }
    }
}

fn part<'a>(v: &'a Value, key: &str, idx: Option<usize>) -> Option<&'a Value> {
    let v = v.get(key)?;
    match idx {
        Some(i) => v.get(i),
        None => Some(v),
    }
}

impl Diff {
    pub fn new(old: &LeetCodeExercise, new: &LeetCodeExercise) -> Result<Self, Why> {
        let json = |e: &LeetCodeExercise| {
            let mut v = serde_json::to_value(e).map_err(|_| Why::Other)?;
            strip_checks(&mut v);
            Ok::<_, Why>(v)
        };
        let new = json(new)?;
        Ok(Self {
            old: json(old)?,
            new_len: new.to_string().len(),
            new,
        })
    }

    /// A field as its §10 JSON key and the item index, if any.
    pub fn key(f: LayoutField) -> (String, Option<usize>) {
        match serde_json::to_value(f) {
            Ok(Value::String(s)) => (s, None),
            Ok(Value::Object(m)) => m
                .into_iter()
                .next()
                .map_or((String::new(), None), |(k, v)| {
                    (k, v.as_u64().and_then(|n| usize::try_from(n).ok()))
                }),
            _ => (String::new(), None),
        }
    }

    /// Does the field (that index, for a list field) differ?
    pub fn changed(&self, f: LayoutField) -> bool {
        let (k, i) = Self::key(f);
        part(&self.old, &k, i) != part(&self.new, &k, i)
    }

    pub fn key_changed(&self, key: &str) -> bool {
        self.old.get(key) != self.new.get(key)
    }

    pub fn fm(&self) -> bool {
        FM_KEYS.iter().any(|k| self.key_changed(k))
    }
}
