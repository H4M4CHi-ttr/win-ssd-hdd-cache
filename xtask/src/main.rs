#![forbid(unsafe_code)]
use std::{path::Path, process::ExitCode};
fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new("check-planning"))
        || args.next().is_some()
    {
        eprintln!("error: USAGE");
        return ExitCode::from(2);
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace parent");
    match xtask::check_repository(root) {
        Ok(s) => {
            println!(
                "planning: ok (labels={}, tasks={}, documents={}, relative_links={})",
                s.labels, s.tasks, s.documents, s.relative_links
            );
            ExitCode::SUCCESS
        }
        Err(errors) => {
            for e in errors {
                eprintln!(
                    "{{\"file\":{},\"code\":{},\"path\":{}}}",
                    serde_json::to_string(&e.file).expect("string encoding"),
                    serde_json::to_string(&e.code).expect("string encoding"),
                    serde_json::to_string(&e.path).expect("string encoding")
                );
            }
            ExitCode::from(1)
        }
    }
}
