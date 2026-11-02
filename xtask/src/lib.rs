#![forbid(unsafe_code)]
mod backlog;
mod json;
mod labels;
mod links;
mod repository;
mod workflow;
pub use backlog::validate_backlog;
pub use labels::validate_label_manifest;
pub use links::validate_markdown_links;
pub use repository::check_repository;
pub use workflow::validate_host_workflow;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ManifestViolation {
    pub code: String,
    pub path: String,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RepositoryViolation {
    pub file: String,
    pub code: String,
    pub path: String,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanningSummary {
    pub labels: usize,
    pub tasks: usize,
    pub documents: usize,
    pub relative_links: usize,
}

pub(crate) fn violation(code: &str, path: impl Into<String>) -> ManifestViolation {
    ManifestViolation {
        code: code.into(),
        path: path.into(),
    }
}
pub(crate) fn finish(mut errors: Vec<ManifestViolation>) -> Result<(), Vec<ManifestViolation>> {
    errors.sort();
    errors.dedup();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
