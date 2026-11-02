use crate::{PlanningSummary, RepositoryViolation, backlog, labels, links};
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

pub fn check_repository(root: &Path) -> Result<PlanningSummary, Vec<RepositoryViolation>> {
    let root = root
        .canonicalize()
        .map_err(|_| vec![error(".", "INPUT_IO", "")])?;
    let mut errors = Vec::new();
    let known = match read_text(&root, ".github/labels.json", labels::MAX_BYTES) {
        Ok(text) => match labels::names(&text) {
            Ok(names) => Some(names),
            Err(e) => {
                errors.extend(
                    e.into_iter()
                        .map(|e| error(".github/labels.json", &e.code, &e.path)),
                );
                None
            }
        },
        Err(e) => {
            errors.push(e);
            None
        }
    };
    let task_count = match read_text(&root, "docs/backlog.json", backlog::MAX_BYTES) {
        Ok(text) => {
            if let Some(known) = &known {
                match backlog::count(&text, known) {
                    Ok(n) => n,
                    Err(e) => {
                        errors.extend(
                            e.into_iter()
                                .map(|e| error("docs/backlog.json", &e.code, &e.path)),
                        );
                        0
                    }
                }
            } else {
                0
            }
        }
        Err(e) => {
            errors.push(e);
            0
        }
    };
    let mut documents = Vec::new();
    if let Err(e) = walk(&root, &root, 0, false, &mut documents) {
        errors.push(e);
    }
    for directory in ["docs", ".github", "examples"] {
        let path = root.join(directory);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {
                if let Err(e) = walk(&root, &path, 0, true, &mut documents) {
                    errors.push(e);
                }
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => errors.push(error(directory, "INPUT_IO", "")),
        }
    }
    documents.sort();
    documents.dedup();
    if documents.is_empty() {
        errors.push(error(".", "INPUT_IO", ""));
    }
    let mut relative_links = 0;
    for document in &documents {
        let file = relative(&root, document);
        match read_text(&root, &file, links::MAX_BYTES) {
            Ok(source) => match links::validate_markdown_links(&root, document, &source) {
                Ok(n) => relative_links += n,
                Err(e) => errors.extend(e.into_iter().map(|e| error(&file, &e.code, &e.path))),
            },
            Err(e) => errors.push(e),
        }
    }
    errors.sort();
    errors.dedup();
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(PlanningSummary {
        labels: known.map_or(0, |n| n.len()),
        tasks: task_count,
        documents: documents.len(),
        relative_links,
    })
}
fn error(file: &str, code: &str, path: &str) -> RepositoryViolation {
    RepositoryViolation {
        file: file.into(),
        code: code.into(),
        path: path.into(),
    }
}
fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
fn read_text(root: &Path, relative: &str, limit: usize) -> Result<String, RepositoryViolation> {
    let fail = |code| error(relative, code, "");
    let mut path = root.to_owned();
    for component in Path::new(relative).components() {
        path.push(component);
        let m = fs::symlink_metadata(&path).map_err(|_| fail("INPUT_IO"))?;
        if m.file_type().is_symlink() {
            return Err(fail("INPUT_IO"));
        }
    }
    let m = fs::symlink_metadata(&path).map_err(|_| fail("INPUT_IO"))?;
    if !m.is_file() {
        return Err(fail("INPUT_IO"));
    }
    if m.len() > limit as u64 {
        return Err(fail("RESOURCE_LIMIT"));
    }
    let file = File::open(path).map_err(|_| fail("INPUT_IO"))?;
    if !file.metadata().map_err(|_| fail("INPUT_IO"))?.is_file() {
        return Err(fail("INPUT_IO"));
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| fail("INPUT_IO"))?;
    if bytes.len() > limit {
        return Err(fail("RESOURCE_LIMIT"));
    }
    String::from_utf8(bytes).map_err(|_| fail("INPUT_IO"))
}
fn walk(
    root: &Path,
    directory: &Path,
    depth: usize,
    recurse: bool,
    documents: &mut Vec<PathBuf>,
) -> Result<(), RepositoryViolation> {
    let name = relative(root, directory);
    if depth > 32 {
        return Err(error(&name, "RESOURCE_LIMIT", ""));
    }
    let mut entries = fs::read_dir(directory)
        .map_err(|_| error(&name, "INPUT_IO", ""))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| error(&name, "INPUT_IO", ""))?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        let kind = entry
            .file_type()
            .map_err(|_| error(&relative(root, &path), "INPUT_IO", ""))?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() && recurse {
            walk(root, &path, depth + 1, true, documents)?;
        } else if kind.is_file() && path.extension().is_some_and(|e| e == "md") {
            if documents.len() == 2048 {
                return Err(error(&name, "RESOURCE_LIMIT", ""));
            }
            documents.push(path);
        }
    }
    Ok(())
}
