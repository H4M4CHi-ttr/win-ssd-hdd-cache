#![forbid(unsafe_code)]

mod input;
mod status;

use std::{ffi::OsString, process::ExitCode};

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

fn run(mut args: impl Iterator<Item = OsString>) -> Result<String, CliError> {
    match args.next().as_deref().and_then(std::ffi::OsStr::to_str) {
        Some("status") => status::execute(args),
        _ => Err(CliError::usage()),
    }
}

fn main() -> ExitCode {
    match run(std::env::args_os().skip(1)) {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("error: {}", error.code);
            ExitCode::from(error.exit)
        }
    }
}
