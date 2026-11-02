use crate::{ManifestViolation, finish, violation};
use pulldown_cmark::{Event, Parser, Tag};
use std::path::Path;
pub(crate) const MAX_BYTES: usize = 2_097_152;

pub fn validate_markdown_links(
    root: &Path,
    document: &Path,
    source: &str,
) -> Result<usize, Vec<ManifestViolation>> {
    if source.len() > MAX_BYTES {
        return Err(vec![violation("RESOURCE_LIMIT", "")]);
    }
    let root = root
        .canonicalize()
        .map_err(|_| vec![violation("INPUT_IO", "")])?;
    let document = if document.is_absolute() {
        document.to_owned()
    } else {
        root.join(document)
    };
    let document = document
        .canonicalize()
        .map_err(|_| vec![violation("INPUT_IO", "")])?;
    if !document.starts_with(&root) {
        return Err(vec![violation("INPUT_IO", "")]);
    }
    let parent = document
        .parent()
        .ok_or_else(|| vec![violation("INPUT_IO", "")])?;
    let mut errors = Vec::new();
    let mut count = 0;
    let mut index = 0;
    for event in Parser::new(source) {
        let destination = match event {
            Event::Start(Tag::Link { dest_url, .. } | Tag::Image { dest_url, .. }) => dest_url,
            _ => continue,
        };
        let pointer = format!("/links/{index}");
        index += 1;
        let destination = destination.as_ref();
        if destination.is_empty() || destination.starts_with('#') || destination.starts_with("//") {
            continue;
        }
        if absolute_local(destination) {
            errors.push(violation("INVALID_LINK", pointer));
            continue;
        }
        if scheme(destination) {
            continue;
        }
        let path = destination.split(['#', '?']).next().unwrap_or("");
        if path.is_empty() {
            continue;
        }
        let Some(path) = decode_percent(path) else {
            errors.push(violation("INVALID_LINK", pointer));
            continue;
        };
        if absolute_local(&path) || path.contains(['\0', '\\']) {
            errors.push(violation("INVALID_LINK", pointer));
            continue;
        }
        match parent.join(path).canonicalize() {
            Ok(target) if target.starts_with(&root) && (target.is_file() || target.is_dir()) => {
                count += 1
            }
            _ => errors.push(violation("INVALID_LINK", pointer)),
        }
    }
    finish(errors)?;
    Ok(count)
}
fn absolute_local(value: &str) -> bool {
    let b = value.as_bytes();
    value.starts_with(['/', '\\']) || (b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':')
}
fn scheme(value: &str) -> bool {
    let Some((scheme, _)) = value.split_once(':') else {
        return false;
    };
    !scheme.is_empty()
        && scheme.as_bytes()[0].is_ascii_alphabetic()
        && scheme
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'-' | b'.'))
}
fn decode_percent(value: &str) -> Option<String> {
    let b = value.as_bytes();
    let mut result = Vec::with_capacity(b.len());
    let mut i = 0;
    let hex = |b: u8| match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    };
    while i < b.len() {
        if b[i] == b'%' {
            result.push(hex(*b.get(i + 1)?)? * 16 + hex(*b.get(i + 2)?)?);
            i += 3;
        } else {
            result.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(result).ok()
}
