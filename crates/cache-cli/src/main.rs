#![forbid(unsafe_code)]

mod benchmark;
mod input;
mod status;

use std::{ffi::OsString, process::ExitCode};

struct CliError {
    exit: u8,
    code: &'static str,
    context: Option<ErrorContext>,
}
#[derive(Clone, Copy)]
enum ErrorContext {
    Base,
    Candidate,
    Compare,
}
impl ErrorContext {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::Candidate => "candidate",
            Self::Compare => "compare",
        }
    }
}
impl CliError {
    const fn usage() -> Self {
        Self {
            exit: 2,
            code: "USAGE",
            context: None,
        }
    }
}

fn run(mut args: impl Iterator<Item = OsString>) -> Result<String, CliError> {
    match args.next().as_deref().and_then(std::ffi::OsStr::to_str) {
        Some("status") => status::execute(args),
        Some("benchmark-report") => benchmark::execute(args),
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
            if let Some(context) = error.context {
                eprintln!("error: {}: {}", context.as_str(), error.code);
            } else {
                eprintln!("error: {}", error.code);
            }
            ExitCode::from(error.exit)
        }
    }
}
