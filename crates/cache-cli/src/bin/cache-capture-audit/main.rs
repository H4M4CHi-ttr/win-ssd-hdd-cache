#![forbid(unsafe_code)]
//! Offline projection audit only. Exit 0 is not D01 PASS or authorization.

#[path = "../../input.rs"]
mod input;

use cache_types::capture::{CAPTURE_MAX_BYTES, audit_capture};
use input::{ReadError, read_input};
use std::{ffi::OsString, path::PathBuf, process::ExitCode};

struct CliError {
    exit: u8,
    code: &'static str,
}

impl CliError {
    const fn usage() -> Self {
        Self {
            exit: 2,
            code: "USAGE",
        }
    }
}

fn run(mut args: impl Iterator<Item = OsString>) -> Result<(String, u8), CliError> {
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--input")) {
        return Err(CliError::usage());
    }
    let path = args.next().ok_or_else(CliError::usage)?;
    if path.is_empty()
        || path.to_str().is_some_and(|value| value.starts_with("--"))
        || args.next().is_some()
    {
        return Err(CliError::usage());
    }
    let source =
        read_input(&PathBuf::from(path), CAPTURE_MAX_BYTES).map_err(|error| match error {
            ReadError::Io => CliError {
                exit: 3,
                code: "INPUT_IO",
            },
            ReadError::TooLarge => CliError {
                exit: 2,
                code: "CAPTURE_TOO_LARGE",
            },
            ReadError::Utf8 => CliError {
                exit: 2,
                code: "INVALID_CAPTURE",
            },
        })?;
    let audit = audit_capture(&source).map_err(|error| CliError {
        exit: 2,
        code: error.code(),
    })?;
    let exit = if audit.is_internally_complete() { 0 } else { 4 };
    Ok((audit.to_json(), exit))
}

fn main() -> ExitCode {
    match run(std::env::args_os().skip(1)) {
        Ok((output, exit)) => {
            print!("{output}");
            ExitCode::from(exit)
        }
        Err(error) => {
            eprintln!("error: {}", error.code);
            ExitCode::from(error.exit)
        }
    }
}
