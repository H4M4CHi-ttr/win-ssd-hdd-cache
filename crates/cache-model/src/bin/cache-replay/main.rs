#![forbid(unsafe_code)]
//! Unprivileged offline runner. No device discovery, network or implicit input.
mod input;

use cache_model::replay::run_trace_with_fault;
use cache_types::{
    evidence::{Fault, Outcome, encode_evidence},
    trace::{TRACE_MAX_BYTES, TraceError, parse_trace},
};
use std::{ffi::OsString, io::Write, path::PathBuf, process::ExitCode};

enum Error {
    Fixed(&'static str),
    Trace(TraceError),
}

fn run(args: impl Iterator<Item = OsString>) -> Result<(String, u8), Error> {
    let mut args = args;
    let mut path = None;
    let mut fault = None;
    while let Some(option) = args.next() {
        match option.to_str() {
            Some("--trace") if path.is_none() => {
                path = Some(PathBuf::from(args.next().ok_or(Error::Fixed("USAGE"))?));
            }
            Some("--fault") if fault.is_none() => {
                fault = Some(
                    args.next()
                        .and_then(|v| v.to_str().and_then(|s| s.parse::<Fault>().ok()))
                        .ok_or(Error::Fixed("USAGE"))?,
                );
            }
            _ => return Err(Error::Fixed("USAGE")),
        }
    }
    let path = path.ok_or(Error::Fixed("USAGE"))?;
    let input = input::read(&path, TRACE_MAX_BYTES).map_err(Error::Fixed)?;
    let trace = parse_trace(&input).map_err(Error::Trace)?;
    drop(input);
    // A typed trace does not establish its raw-input digest, and the caller's
    // cwd/HEAD/environment do not authenticate the executable's source commit.
    // Keep both provenance fields null; this command is not a release gate.
    let result = run_trace_with_fault(&trace, fault.unwrap_or(Fault::None));
    let output =
        encode_evidence(result.evidence()).map_err(|_| Error::Fixed("EVIDENCE_INVALID"))?;
    let exit = match result.evidence().outcome {
        Outcome::Passed => 0,
        Outcome::Inconclusive => 2,
        Outcome::Failed => 3,
    };
    Ok((output, exit))
}

/// Shared by the Cargo binary and the documented `--example replay` entry.
pub fn main() -> ExitCode {
    let result = run(std::env::args_os().skip(1)).and_then(|(output, exit)| {
        std::io::stdout()
            .lock()
            .write_all(output.as_bytes())
            .map_err(|_| Error::Fixed("OUTPUT_IO"))?;
        Ok(exit)
    });
    match result {
        Ok(exit) => ExitCode::from(exit),
        Err(error) => {
            let mut stderr = std::io::stderr().lock();
            // Neither error variant contains paths, payloads or serde messages.
            match error {
                Error::Fixed(code) => {
                    let _ = writeln!(stderr, "error: {code}");
                }
                Error::Trace(error) => {
                    let _ = writeln!(stderr, "error: {error}");
                }
            }
            ExitCode::from(1)
        }
    }
}
