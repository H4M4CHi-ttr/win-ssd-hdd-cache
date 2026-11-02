use crate::{ManifestViolation, finish, json, violation};
use std::collections::BTreeSet;
pub(crate) const MAX_BYTES: usize = 1_048_576;

pub fn validate_label_manifest(input: &str) -> Result<(), Vec<ManifestViolation>> {
    names(input).map(|_| ())
}
pub(crate) fn names(input: &str) -> Result<BTreeSet<String>, Vec<ManifestViolation>> {
    let parsed = (|| {
        let root = json::parse(input, MAX_BYTES)?;
        let root = json::object(&root, "", &["schema_version", "labels"])?;
        let version = json::unsigned(root["schema_version"], "/schema_version")?;
        let labels = json::array(root["labels"], "/labels")?;
        if labels.len() > 256 {
            return Err(violation("INVALID_JSON", ""));
        }
        let mut result = Vec::new();
        for (i, label) in labels.iter().enumerate() {
            let path = format!("/labels/{i}");
            let entry = json::object(label, &path, &["name", "description"])?;
            result.push((
                json::string(entry["name"], &format!("{path}/name"))?,
                json::string(entry["description"], &format!("{path}/description"))?,
            ));
        }
        Ok((version, result))
    })()
    .map_err(|e| vec![e])?;
    let (version, labels) = parsed;
    let mut errors = Vec::new();
    let mut names = BTreeSet::new();
    if version != 1 {
        errors.push(violation("UNSUPPORTED_SCHEMA", "/schema_version"));
    }
    for (i, (name, description)) in labels.iter().enumerate() {
        let path = format!("/labels/{i}/name");
        if !name.is_ascii() || !(1..=50).contains(&name.len()) {
            errors.push(violation("INVALID_NAME", &path));
        }
        if ![
            "agent:",
            "review:",
            "status:",
            "priority:",
            "area:",
            "type:",
            "decision:",
        ]
        .iter()
        .any(|p| name.starts_with(p))
        {
            errors.push(violation("UNKNOWN_PREFIX", &path));
        }
        if !names.insert(name.clone()) {
            errors.push(violation("DUPLICATE_NAME", &path));
        }
        if !(1..=100).contains(&description.chars().count()) {
            errors.push(violation(
                "INVALID_DESCRIPTION",
                format!("/labels/{i}/description"),
            ));
        }
    }
    finish(errors)?;
    Ok(names)
}
