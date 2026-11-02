use crate::{
    CliError,
    input::{ReadError, read_input},
};
use cache_types::status::{OutputFormat, STATUS_MAX_BYTES, parse_status, render_status};
use std::{ffi::OsString, path::PathBuf};

pub(super) fn execute(mut args: impl Iterator<Item = OsString>) -> Result<String, CliError> {
    let mut input = None;
    let mut format = None;
    while let Some(flag) = args.next() {
        match flag.to_str() {
            Some("--input") if input.is_none() => {
                input = Some(PathBuf::from(args.next().ok_or_else(CliError::usage)?))
            }
            Some("--format") if format.is_none() => {
                format = Some(
                    match args.next().as_deref().and_then(std::ffi::OsStr::to_str) {
                        Some("json") => OutputFormat::Json,
                        Some("text") => OutputFormat::Text,
                        _ => return Err(CliError::usage()),
                    },
                );
            }
            _ => return Err(CliError::usage()),
        }
    }
    let input = input.ok_or_else(CliError::usage)?;
    let format = format.ok_or_else(CliError::usage)?;
    let source = read_input(&input, STATUS_MAX_BYTES).map_err(|e| match e {
        ReadError::Io => CliError {
            exit: 3,
            code: "INPUT_IO",
        },
        ReadError::TooLarge => CliError {
            exit: 2,
            code: "INPUT_TOO_LARGE",
        },
        ReadError::Utf8 => CliError {
            exit: 2,
            code: "INVALID_JSON",
        },
    })?;
    let snapshot = parse_status(&source).map_err(|e| CliError {
        exit: 2,
        code: e.code(),
    })?;
    Ok(render_status(&snapshot, format))
}
