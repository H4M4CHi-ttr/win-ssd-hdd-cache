//! Default-off experimental collector; no default or implicit live behavior.
#![deny(unsafe_code)]

mod capture;
mod input;
#[cfg(all(windows, feature = "experimental-live"))]
#[allow(unsafe_code)]
mod win32;

use cache_probe_core::{MAX_ARCHIVE_BYTES, audit_archive, project_v2};
use std::{ffi::OsString, io, path::PathBuf, process::ExitCode};

const HELP: &str = "Experimental P04 evidence only: always unqualified.\n\
Usage:\n  cache-probe-collector audit --input <ordinary-file>\n  cache-probe-collector project --input <ordinary-file>\n  cache-probe-collector collect --experimental-live --duration-ms <1..30000>\n\
Live capture requires a separately authorized lab and an experimental-live build.\n";

enum Mode {
    Help,
    Audit(PathBuf),
    Project(PathBuf),
    Collect(u32),
}

fn parse(args: &[OsString]) -> Result<Mode, &'static str> {
    if args.len() == 1 && args[0] == "--help" {
        return Ok(Mode::Help);
    }
    if args.len() == 3 && args[1] == "--input" && !args[2].is_empty() && args[2] != "-" {
        if args[0] == "audit" {
            return Ok(Mode::Audit(PathBuf::from(&args[2])));
        }
        if args[0] == "project" {
            return Ok(Mode::Project(PathBuf::from(&args[2])));
        }
    }
    if args.len() == 4
        && args[0] == "collect"
        && args[1] == "--experimental-live"
        && args[2] == "--duration-ms"
        && let Some(text) = args[3].to_str()
        && !text.is_empty()
        && !text.starts_with('0')
        && text.bytes().all(|b| b.is_ascii_digit())
        && let Ok(value @ 1..=30_000) = text.parse::<u32>()
    {
        return Ok(Mode::Collect(value));
    }
    Err("PROBE_USAGE")
}

fn run(args: &[OsString]) -> Result<u8, (&'static str, u8)> {
    let mode = parse(args).map_err(|code| (code, 2))?;
    match mode {
        Mode::Help => {
            output(HELP.as_bytes())?;
            Ok(0)
        }
        Mode::Collect(duration) => live(duration),
        Mode::Audit(ref path) | Mode::Project(ref path) => {
            let bytes =
                input::read_input(path, MAX_ARCHIVE_BYTES).map_err(|error| match error {
                    input::ReadError::Io => ("PROBE_INPUT_IO", 3),
                    input::ReadError::TooLarge => ("PROBE_TOO_LARGE", 2),
                })?;
            let text = match mode {
                Mode::Audit(_) => audit_archive(&bytes).map(|report| report.to_json()),
                Mode::Project(_) => project_v2(&bytes),
                _ => unreachable!(),
            }
            .map_err(|error| (error.code(), 2))?;
            output(text.as_bytes())?;
            Ok(4)
        }
    }
}

fn output(bytes: &[u8]) -> Result<(), (&'static str, u8)> {
    capture::emit_archive(&mut io::stdout().lock(), bytes).map_err(|error| (error.code(), 3))
}

#[cfg(not(all(windows, feature = "experimental-live")))]
fn live(_duration: u32) -> Result<u8, (&'static str, u8)> {
    Err(("PROBE_LIVE_DISABLED", 2))
}

#[cfg(all(windows, feature = "experimental-live"))]
fn live(duration: u32) -> Result<u8, (&'static str, u8)> {
    // Pipe validation precedes both randomness and the fixed CDO open.
    if !win32::stdout_is_pipe() {
        return Err(("PROBE_STDOUT_PIPE_REQUIRED", 2));
    }
    let session =
        capture::fresh_session(&mut win32::SystemRng).map_err(|error| (error.code(), 2))?;
    let mut device = win32::Device::open().map_err(|_| ("PROBE_DEVICE_UNAVAILABLE", 3))?;
    let bytes = capture::collect(&mut device, &mut win32::Clock::new(), session, duration)
        .map_err(|error| {
            (
                error.code(),
                if error == capture::CollectError::DeviceUnavailable {
                    3
                } else {
                    2
                },
            )
        })?;
    output(&bytes)?;
    Ok(4)
}

fn main() -> ExitCode {
    match run(&std::env::args_os().skip(1).collect::<Vec<_>>()) {
        Ok(code) => ExitCode::from(code),
        Err((code, exit)) => {
            eprintln!("{code}");
            ExitCode::from(exit)
        }
    }
}
