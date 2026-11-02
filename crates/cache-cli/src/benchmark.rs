use crate::{
    CliError, ErrorContext,
    input::{ReadError, read_input},
};
use cache_types::{
    benchmark::{
        BENCHMARK_MAX_BYTES, BenchmarkComparison, BenchmarkRun, BenchmarkSummary, compare_runs,
        parse_benchmark,
    },
    status::OutputFormat,
};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

pub(super) fn execute(mut args: impl Iterator<Item = OsString>) -> Result<String, CliError> {
    let mut base = None;
    let mut candidate = None;
    let mut format = None;
    while let Some(flag) = args.next() {
        match flag.to_str() {
            Some("--base") if base.is_none() => {
                base = Some(PathBuf::from(args.next().ok_or_else(CliError::usage)?))
            }
            Some("--candidate") if candidate.is_none() => {
                candidate = Some(PathBuf::from(args.next().ok_or_else(CliError::usage)?))
            }
            Some("--format") if format.is_none() => {
                format = Some(
                    match args.next().as_deref().and_then(std::ffi::OsStr::to_str) {
                        Some("json") => OutputFormat::Json,
                        Some("text") => OutputFormat::Text,
                        _ => return Err(CliError::usage()),
                    },
                )
            }
            _ => return Err(CliError::usage()),
        }
    }
    let base = base.ok_or_else(CliError::usage)?;
    let candidate = candidate.ok_or_else(CliError::usage)?;
    let format = format.ok_or_else(CliError::usage)?;
    let base = read(&base, ErrorContext::Base)?;
    let candidate = read(&candidate, ErrorContext::Candidate)?;
    let result = compare_runs(&base, &candidate).map_err(|e| CliError {
        exit: 2,
        code: e.field(),
        context: Some(ErrorContext::Compare),
    })?;
    Ok(render(&result, format))
}
fn read(path: &Path, context: ErrorContext) -> Result<BenchmarkRun, CliError> {
    let source = read_input(path, BENCHMARK_MAX_BYTES).map_err(|e| match e {
        ReadError::Io => CliError {
            exit: 3,
            code: "INPUT_IO",
            context: Some(context),
        },
        ReadError::TooLarge => CliError {
            exit: 2,
            code: "INPUT_TOO_LARGE",
            context: Some(context),
        },
        ReadError::Utf8 => CliError {
            exit: 2,
            code: "INVALID_JSON",
            context: Some(context),
        },
    })?;
    parse_benchmark(&source).map_err(|e| CliError {
        exit: 2,
        code: e.code(),
        context: Some(context),
    })
}
fn render(comparison: &BenchmarkComparison, format: OutputFormat) -> String {
    let summary_json = |s: &BenchmarkSummary| {
        format!(
            "{{\"cache_mode\":\"{}\",\"ssd_cache\":\"{}\",\"duration_ns\":\"{}\",\"p95_ns\":\"{}\",\"p99_ns\":\"{}\"}}",
            s.cache_mode.as_str(),
            s.ssd_cache.as_str(),
            s.duration_ns,
            s.latency.p95_ns,
            s.latency.p99_ns
        )
    };
    let summary_text = |name: &str, s: &BenchmarkSummary| {
        format!(
            "{name}.cache_mode: {}\n{name}.ssd_cache: {}\n{name}.duration_ns: {}\n{name}.p95_ns: {}\n{name}.p99_ns: {}\n",
            s.cache_mode.as_str(),
            s.ssd_cache.as_str(),
            s.duration_ns,
            s.latency.p95_ns,
            s.latency.p99_ns
        )
    };
    match format {
        OutputFormat::Json => format!(
            "{{\"schema_version\":1,\"base\":{},\"candidate\":{},\"speedup\":\"{:.6}\"}}\n",
            summary_json(&comparison.base),
            summary_json(&comparison.candidate),
            comparison.speedup
        ),
        OutputFormat::Text => format!(
            "{}{}speedup: {:.6}\n",
            summary_text("base", &comparison.base),
            summary_text("candidate", &comparison.candidate),
            comparison.speedup
        ),
    }
}
