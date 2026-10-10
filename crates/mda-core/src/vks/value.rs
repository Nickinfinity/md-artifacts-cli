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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
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
#[derive(Debug, Clone, Eq, serde::Serialize)]
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
