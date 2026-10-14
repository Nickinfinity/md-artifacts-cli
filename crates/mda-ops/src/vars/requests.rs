//! The `vars.*` request types (split from `vars.rs`): camelCase, `deny_unknown_fields`.

use mda_core::variables::SubsetRef;
use mda_core::vks::VksValue;

fn empty_value() -> VksValue {
    VksValue::Str(String::new())
}

/// `vars.file.new` params: a blank Variables file `<dir>/<slug(title)>.md` (`dir` defaults to
/// `Variables`, must be it or under it).
///
/// # Examples
///
/// ```
/// let r: mda_ops::vars::NewFileRequest = serde_json::from_value(serde_json::json!({"title": "Dev"})).unwrap();
/// assert_eq!((r.title.as_str(), r.dir), ("Dev", None));
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NewFileRequest {
    pub title: String,
    #[serde(default)]
    pub dir: Option<String>,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// `vars.meta.tags` params: replace the file's tags (`[]` clears them).
///
/// # Examples
///
/// ```
/// let r: mda_ops::vars::TagsRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Variables/a.md", "expectedHash": "h", "tags": ["env"]})).unwrap();
/// assert_eq!(r.tags, ["env"]);
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TagsRequest {
    pub path: String,
    pub expected_hash: String,
    pub tags: Vec<String>,
}

/// `vars.subset.add` params: append an empty sub-set `heading`.
///
/// # Examples
///
/// ```
/// let r: mda_ops::vars::SubsetAddRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Variables/a.md", "expectedHash": "h", "heading": "Prod"})).unwrap();
/// assert_eq!(r.heading, "Prod");
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubsetAddRequest {
    pub path: String,
    pub expected_hash: String,
    pub heading: String,
}

/// `vars.subset.rename` params: give `subset` the new `heading`.
///
/// # Examples
///
/// ```
/// let r: mda_ops::vars::SubsetRenameRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Variables/a.md", "expectedHash": "h", "subset": {"index": 1, "heading": "Prod"}, "heading": "Live"})).unwrap();
/// assert_eq!(r.subset.index, 1);
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubsetRenameRequest {
    pub path: String,
    pub expected_hash: String,
    pub subset: SubsetRef,
    pub heading: String,
}

/// `vars.subset.describe` params: set (`""` removes) the description of `subset`.
///
/// # Examples
///
/// ```
/// let r: mda_ops::vars::SubsetDescribeRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Variables/a.md", "expectedHash": "h", "subset": {"index": 0, "heading": "Dev"}, "description": ""})).unwrap();
/// assert!(r.description.is_empty());
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubsetDescribeRequest {
    pub path: String,
    pub expected_hash: String,
    pub subset: SubsetRef,
    pub description: String,
}

/// `vars.subset.delete` params: delete `subset`.
///
/// # Examples
///
/// ```
/// let r: mda_ops::vars::SubsetDeleteRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Variables/a.md", "expectedHash": "h", "subset": {"index": 0, "heading": "Dev"}})).unwrap();
/// assert_eq!(r.subset.heading, "Dev");
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SubsetDeleteRequest {
    pub path: String,
    pub expected_hash: String,
    pub subset: SubsetRef,
}

/// `vars.var.add` params: add `name` (value defaults to `""`) to `subset` (absent = untitled block).
///
/// # Examples
///
/// ```
/// let r: mda_ops::vars::VarAddRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Variables/a.md", "expectedHash": "h", "name": "VK-a"})).unwrap();
/// assert!(r.subset.is_none());
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VarAddRequest {
    pub path: String,
    pub expected_hash: String,
    #[serde(default)]
    pub subset: Option<SubsetRef>,
    pub name: String,
    #[serde(default = "empty_value")]
    pub value: VksValue,
}

/// `vars.var.rename` params: rename `name` to `newName`, keeping its value.
///
/// # Examples
///
/// ```
/// let r: mda_ops::vars::VarRenameRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Variables/a.md", "expectedHash": "h", "name": "VK-a", "newName": "VK-b"})).unwrap();
/// assert_eq!(r.new_name, "VK-b");
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VarRenameRequest {
    pub path: String,
    pub expected_hash: String,
    #[serde(default)]
    pub subset: Option<SubsetRef>,
    pub name: String,
    pub new_name: String,
}

/// `vars.var.set` params: set `name` to `value` (a string, list or record).
///
/// # Examples
///
/// ```
/// let r: mda_ops::vars::VarSetRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Variables/a.md", "expectedHash": "h", "name": "VK-a", "value": ["x"]})).unwrap();
/// assert_eq!(r.name, "VK-a");
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VarSetRequest {
    pub path: String,
    pub expected_hash: String,
    #[serde(default)]
    pub subset: Option<SubsetRef>,
    pub name: String,
    pub value: VksValue,
}

/// `vars.var.delete` params: delete every var named `name`.
///
/// # Examples
///
/// ```
/// let r: mda_ops::vars::VarDeleteRequest = serde_json::from_value(serde_json::json!(
///     {"path": "Variables/a.md", "expectedHash": "h", "name": "VK-a"})).unwrap();
/// assert_eq!(r.name, "VK-a");
/// ```
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VarDeleteRequest {
    pub path: String,
    pub expected_hash: String,
    #[serde(default)]
    pub subset: Option<SubsetRef>,
    pub name: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, from_value, json};

    fn rejects<T: serde::de::DeserializeOwned>(mut v: Value) {
        v["bogus"] = json!(1);
        assert!(from_value::<T>(v).is_err());
    }

    #[test]
    fn every_request_rejects_an_unknown_field() {
        let p = json!({"path": "Variables/a.md", "expectedHash": "h"});
        let with = |extra: Value| {
            let mut v = p.clone();
            for (k, x) in extra.as_object().unwrap() {
                v[k] = x.clone();
            }
            v
        };
        let s = json!({"index": 0, "heading": "A"});
        rejects::<NewFileRequest>(json!({"title": "T"}));
        rejects::<TagsRequest>(with(json!({"tags": []})));
        rejects::<SubsetAddRequest>(with(json!({"heading": "A"})));
        rejects::<SubsetRenameRequest>(with(json!({"subset": s, "heading": "B"})));
        rejects::<SubsetDescribeRequest>(with(json!({"subset": s, "description": ""})));
        rejects::<SubsetDeleteRequest>(with(json!({"subset": s})));
        rejects::<VarAddRequest>(with(json!({"name": "VK-a"})));
        rejects::<VarRenameRequest>(with(json!({"name": "VK-a", "newName": "VK-b"})));
        rejects::<VarSetRequest>(with(json!({"name": "VK-a", "value": "x"})));
        rejects::<VarDeleteRequest>(with(json!({"name": "VK-a"})));
    }

    #[test]
    fn var_add_value_defaults_empty() {
        let r: VarAddRequest =
            from_value(json!({"path": "p", "expectedHash": "h", "name": "VK-a"})).unwrap();
        assert_eq!(r.value, VksValue::Str(String::new()));
    }

    #[test]
    fn var_set_value_is_required() {
        let r =
            from_value::<VarSetRequest>(json!({"path": "p", "expectedHash": "h", "name": "VK-a"}));
        assert!(r.is_err());
    }

    #[test]
    fn subset_ref_rejects_unknown_field() {
        let r = from_value::<SubsetRef>(json!({"index": 0, "heading": "A", "x": 1}));
        assert!(r.is_err());
    }

    #[test]
    fn value_rejects_number() {
        let r = from_value::<VarSetRequest>(
            json!({"path": "p", "expectedHash": "h", "name": "VK-a", "value": 1}),
        );
        assert!(r.is_err());
    }
}
