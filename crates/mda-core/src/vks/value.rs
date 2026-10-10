//! The structured value model (D-3): every scalar is a string at every depth (D-2).

use serde::ser::SerializeMap;

/// A variable's value: a string, a list, or a record.
///
/// # Examples
///
/// ```
/// use mda_core::vks::VksValue;
/// assert_eq!(serde_json::to_string(&VksValue::Str("a".into())).unwrap(), r#""a""#);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum VksValue {
    Str(String),
    List(VksList),
    Record(VksRecord),
}

/// A list holds only strings or only records (`vks.mixed_list` otherwise).
///
/// # Examples
///
/// ```
/// use mda_core::vks::VksList;
/// let l = VksList::Strings(vec!["a".into()]);
/// assert_eq!(serde_json::to_string(&l).unwrap(), r#"["a"]"#);
/// ```
#[derive(Debug, Clone, Eq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum VksList {
    Strings(Vec<String>),
    Records(Vec<VksRecord>),
}

/// Equal when the items are; an empty list is one value whichever variant holds it (spec §9.5).
impl PartialEq for VksList {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Strings(a), Self::Strings(b)) => a == b,
            (Self::Records(a), Self::Records(b)) => a == b,
            (Self::Strings(s), Self::Records(r)) | (Self::Records(r), Self::Strings(s)) => {
                s.is_empty() && r.is_empty()
            }
        }
    }
}

/// An ordered record; serialized as a JSON object in source order.
///
/// # Examples
///
/// ```
/// use mda_core::vks::{VksRecord, VksValue};
/// let r = VksRecord(vec![("z".into(), VksValue::Str("1".into())), ("a".into(), VksValue::Str("2".into()))]);
/// assert_eq!(serde_json::to_string(&r).unwrap(), r#"{"z":"1","a":"2"}"#);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VksRecord(pub Vec<(String, VksValue)>);

impl serde::Serialize for VksRecord {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            m.serialize_entry(k, v)?;
        }
        m.end()
    }
}

/// Reads a JSON object in source order (the model's input side, W-10).
impl<'de> serde::Deserialize<'de> for VksRecord {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_map(RecordVisitor)
    }
}

struct RecordVisitor;

impl<'de> serde::de::Visitor<'de> for RecordVisitor {
    type Value = VksRecord;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a record (JSON object)")
    }

    /// A duplicate key is a shape error (`op.bad_request`), like the reader's `vks.duplicate_key`.
    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut m: A) -> Result<VksRecord, A::Error> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        while let Some((k, v)) = m.next_entry::<String, VksValue>()? {
            if !seen.insert(k.clone()) {
                return Err(serde::de::Error::custom("duplicate key"));
            }
            out.push((k, v));
        }
        Ok(VksRecord(out))
    }
}

impl VksValue {
    /// The flat default a plain-token client sees (spec §9.3): a string is itself, a list of
    /// strings its first item, anything else `""`.
    ///
    /// # Examples
    ///
    /// ```
    /// use mda_core::vks::{VksList, VksValue};
    /// assert_eq!(VksValue::Str("a".into()).default_value(), "a");
    /// let l = VksValue::List(VksList::Strings(vec!["x".into(), "y".into()]));
    /// assert_eq!(l.default_value(), "x");
    /// ```
    pub fn default_value(&self) -> &str {
        match self {
            Self::Str(s) => s,
            Self::List(VksList::Strings(v)) => v.first().map_or("", String::as_str),
            Self::List(VksList::Records(_)) | Self::Record(_) => "",
        }
    }
}
