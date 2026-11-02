//! Preserve raw numeric spelling and duplicate object keys without error-text parsing.
use crate::{ManifestViolation, violation};
use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, Visitor},
};
use serde_json::value::RawValue;
use std::{collections::BTreeMap, fmt};

pub(crate) enum Node {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<Node>),
    Object(Vec<(String, Node)>),
}
pub(crate) type Result<T> = std::result::Result<T, ManifestViolation>;
pub(crate) type Object<'a> = BTreeMap<&'a str, &'a Node>;
pub(crate) fn pointer(parent: &str, key: &str) -> String {
    format!("{parent}/{}", key.replace('~', "~0").replace('/', "~1"))
}

struct Pairs<'a>(Vec<(String, &'a RawValue)>);
impl<'de> Deserialize<'de> for Pairs<'de> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct PairsVisitor;
        impl<'de> Visitor<'de> for PairsVisitor {
            type Value = Pairs<'de>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("object")
            }
            fn visit_map<M: MapAccess<'de>>(
                self,
                mut map: M,
            ) -> std::result::Result<Self::Value, M::Error> {
                let mut pairs = Vec::new();
                while let Some(entry) = map.next_entry::<String, &'de RawValue>()? {
                    pairs.push(entry);
                }
                Ok(Pairs(pairs))
            }
        }
        d.deserialize_map(PairsVisitor)
    }
}
pub(crate) fn parse(input: &str, limit: usize) -> Result<Node> {
    if input.len() > limit {
        return Err(violation("INVALID_JSON", ""));
    }
    let raw: &RawValue = serde_json::from_str(input).map_err(|_| violation("INVALID_JSON", ""))?;
    node(raw, 0)
}
fn node(raw: &RawValue, depth: usize) -> Result<Node> {
    let text = raw.get();
    if depth >= 64 && matches!(text.as_bytes().first(), Some(b'[' | b'{')) {
        return Err(violation("INVALID_JSON", ""));
    }
    let invalid = |_| violation("INVALID_JSON", "");
    match text.as_bytes().first() {
        Some(b'{') => {
            let Pairs(values) = serde_json::from_str::<Pairs<'_>>(text).map_err(invalid)?;
            values
                .into_iter()
                .map(|(key, value)| Ok((key, node(value, depth + 1)?)))
                .collect::<Result<Vec<_>>>()
                .map(Node::Object)
        }
        Some(b'[') => {
            let values: Vec<&RawValue> = serde_json::from_str(text).map_err(invalid)?;
            values
                .into_iter()
                .map(|v| node(v, depth + 1))
                .collect::<Result<Vec<_>>>()
                .map(Node::Array)
        }
        Some(b'"') => serde_json::from_str(text)
            .map(Node::String)
            .map_err(invalid),
        Some(b't') => Ok(Node::Bool(true)),
        Some(b'f') => Ok(Node::Bool(false)),
        Some(b'n') => Ok(Node::Null),
        Some(b'-' | b'0'..=b'9') => Ok(Node::Number(text.into())),
        _ => Err(violation("INVALID_JSON", "")),
    }
}
pub(crate) fn object<'a>(node: &'a Node, path: &str, fields: &[&str]) -> Result<Object<'a>> {
    let Node::Object(pairs) = node else {
        return Err(violation("INVALID_JSON", path));
    };
    let mut object = BTreeMap::new();
    for (key, value) in pairs {
        let pointer = pointer(path, key);
        if object.insert(key.as_str(), value).is_some() {
            return Err(violation("INVALID_JSON", pointer));
        }
        if !fields.contains(&key.as_str()) {
            return Err(violation("UNKNOWN_FIELD", pointer));
        }
    }
    for key in fields {
        if !object.contains_key(key) {
            return Err(violation("INVALID_JSON", pointer(path, key)));
        }
    }
    Ok(object)
}
pub(crate) fn array<'a>(node: &'a Node, path: &str) -> Result<&'a [Node]> {
    match node {
        Node::Array(a) => Ok(a),
        _ => Err(violation("INVALID_JSON", path)),
    }
}
pub(crate) fn string(node: &Node, path: &str) -> Result<String> {
    match node {
        Node::String(s) => Ok(s.clone()),
        _ => Err(violation("INVALID_JSON", path)),
    }
}
pub(crate) fn strings(node: &Node, path: &str) -> Result<Vec<String>> {
    array(node, path)?
        .iter()
        .enumerate()
        .map(|(i, n)| string(n, &pointer(path, &i.to_string())))
        .collect()
}
pub(crate) fn boolean(node: &Node, path: &str) -> Result<bool> {
    match node {
        Node::Bool(b) => Ok(*b),
        _ => Err(violation("INVALID_JSON", path)),
    }
}
pub(crate) fn unsigned(node: &Node, path: &str) -> Result<u64> {
    if let Node::Number(s) = node
        && !s.is_empty()
        && s.bytes().all(|b| b.is_ascii_digit())
        && let Ok(n) = s.parse()
    {
        return Ok(n);
    }
    Err(violation("INVALID_JSON", path))
}
