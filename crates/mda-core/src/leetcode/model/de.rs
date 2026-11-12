//! Hand-written `Deserialize` for the wire types whose `Serialize` is hand-written (W7 H7.pre, D-43):
//! [`OrderedMap`] (JS object semantics), [`CaseValue`] (the Int/Float rule) and [`ProjectCheck`]
//! (a flat object; `#[serde(flatten)]` and `deny_unknown_fields` do not combine), plus the
//! `ignore_any` field helper for the derived `warnings` / `unimplementedKinds` (Q-71).

use std::fmt;

use serde::de::{self, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

use super::{
    BuildCheck, CaseValue, CheckBase, CssAssertCheck, DomAssertCheck, FunctionCheck, HttpCheck,
    OrderedMap, ProjectCheck, TestCase,
};

/// Consume any value and return the type's default: derived fields a client may echo back verbatim.
pub(super) fn ignore_any<'de, D: Deserializer<'de>, T: Default>(d: D) -> Result<T, D::Error> {
    IgnoredAny::deserialize(d).map(|_| T::default())
}

impl<'de, V: Deserialize<'de>> Deserialize<'de> for OrderedMap<V> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V_<V>(std::marker::PhantomData<V>);
        impl<'de, V: Deserialize<'de>> Visitor<'de> for V_<V> {
            type Value = OrderedMap<V>;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<Self::Value, A::Error> {
                let mut pairs = Vec::new();
                while let Some(kv) = m.next_entry::<String, V>()? {
                    pairs.push(kv);
                }
                Ok(OrderedMap::from_pairs(pairs))
            }
        }
        d.deserialize_map(V_(std::marker::PhantomData))
    }
}

impl<'de> Deserialize<'de> for CaseValue {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(CaseVisitor)
    }
}

struct CaseVisitor;

impl<'de> Visitor<'de> for CaseVisitor {
    type Value = CaseValue;
    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a JSON value")
    }
    fn visit_unit<E: de::Error>(self) -> Result<CaseValue, E> {
        Ok(CaseValue::Null)
    }
    fn visit_none<E: de::Error>(self) -> Result<CaseValue, E> {
        Ok(CaseValue::Null)
    }
    fn visit_bool<E: de::Error>(self, b: bool) -> Result<CaseValue, E> {
        Ok(CaseValue::Bool(b))
    }
    fn visit_i64<E: de::Error>(self, i: i64) -> Result<CaseValue, E> {
        Ok(CaseValue::Int(i))
    }
    // An integer above i64 is a JS number: kept as a float (`Serialize` prints its integer text).
    fn visit_u64<E: de::Error>(self, u: u64) -> Result<CaseValue, E> {
        Ok(i64::try_from(u).map_or(CaseValue::Float(u as f64), CaseValue::Int))
    }
    fn visit_f64<E: de::Error>(self, x: f64) -> Result<CaseValue, E> {
        Ok(CaseValue::Float(x))
    }
    fn visit_str<E: de::Error>(self, s: &str) -> Result<CaseValue, E> {
        Ok(CaseValue::Str(s.to_owned()))
    }
    fn visit_string<E: de::Error>(self, s: String) -> Result<CaseValue, E> {
        Ok(CaseValue::Str(s))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut q: A) -> Result<CaseValue, A::Error> {
        let mut v = Vec::new();
        while let Some(x) = q.next_element()? {
            v.push(x);
        }
        Ok(CaseValue::List(v))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<CaseValue, A::Error> {
        let mut pairs = Vec::new();
        while let Some(kv) = m.next_entry::<String, CaseValue>()? {
            pairs.push(kv);
        }
        Ok(CaseValue::Object(OrderedMap::from_pairs(pairs)))
    }
}

impl<'de> Deserialize<'de> for TestCase {
    /// A `request` key makes it an HTTP case (verbatim); else exactly `input` (object) + `expected`.
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let m = OrderedMap::<CaseValue>::deserialize(d)?;
        if TestCase::is_http(&m) {
            return Ok(TestCase::Http(m));
        }
        let (mut input, mut expected) = (None, None);
        for (k, v) in m.0 {
            match (k.as_str(), v) {
                ("input", CaseValue::Object(o)) => input = Some(o),
                ("expected", v) => expected = Some(v),
                ("input", _) => return Err(de::Error::custom("case input must be an object")),
                _ => return Err(de::Error::unknown_field(&k, &["input", "expected"])),
            }
        }
        match (input, expected) {
            (Some(input), Some(expected)) => Ok(TestCase::Call { input, expected }),
            (None, _) => Err(de::Error::missing_field("input")),
            _ => Err(de::Error::missing_field("expected")),
        }
    }
}

/// The flat wire form of a check (as `ProjectCheck`'s `Serialize` writes it).
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CheckWire {
    name: String,
    kind: String,
    file: Option<String>,
    function: Option<String>,
    argv: Option<Vec<String>>,
    dir: Option<String>,
    package: Option<String>,
    cases: Vec<TestCase>,
    public_count: usize,
}

impl<'de> Deserialize<'de> for ProjectCheck {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let w = CheckWire::deserialize(d)?;
        check_from_wire(w).map_err(de::Error::custom)
    }
}

/// Each kind takes exactly its own fields: a missing one or a stray one is refused.
fn check_from_wire(w: CheckWire) -> Result<ProjectCheck, String> {
    let base = CheckBase {
        name: w.name,
        cases: w.cases,
        public_count: w.public_count,
    };
    let need = |v: Option<String>, k: &str| v.ok_or(format!("check kind {} needs {k}", w.kind));
    let none = |present: bool, k: &str| {
        if present {
            Err(format!("check kind {} takes no {k}", w.kind))
        } else {
            Ok(())
        }
    };
    let (file, function, argv, dir, package) = (w.file, w.function, w.argv, w.dir, w.package);
    match w.kind.as_str() {
        "call" => {
            none(
                argv.is_some() || dir.is_some() || package.is_some(),
                "argv/dir/package",
            )?;
            Ok(ProjectCheck::Call(FunctionCheck {
                base,
                file: need(file, "file")?,
                function: need(function, "function")?,
            }))
        }
        "build" => {
            none(
                file.is_some() || function.is_some() || package.is_some(),
                "file/function/package",
            )?;
            let argv = argv.ok_or(format!("check kind {} needs argv", w.kind))?;
            Ok(ProjectCheck::Build(BuildCheck { base, argv, dir }))
        }
        "dom-assert" | "css-assert" => {
            none(
                function.is_some() || argv.is_some() || dir.is_some(),
                "function/argv/dir",
            )?;
            let file = need(file, "file")?;
            Ok(if w.kind == "dom-assert" {
                ProjectCheck::DomAssert(DomAssertCheck {
                    base,
                    file,
                    package,
                })
            } else {
                ProjectCheck::CssAssert(CssAssertCheck {
                    base,
                    file,
                    package,
                })
            })
        }
        "http" => {
            none(
                file.is_some() || function.is_some() || argv.is_some() || dir.is_some(),
                "file/function/argv/dir",
            )?;
            Ok(ProjectCheck::Http(HttpCheck {
                base,
                package: need(package, "package")?,
            }))
        }
        other => Err(format!("unknown check kind {other}")),
    }
}

#[cfg(test)]
mod tests;
