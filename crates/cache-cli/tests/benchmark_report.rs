mod common;
use cache_types::benchmark::{CompareError, OsCache, compare_runs, parse_benchmark};
use common::{TempInput, assert_error, run};
use std::{ffi::OsStr, path::Path, process::Output};
const BASE: &str = include_str!("../../../examples/benchmarks/base.json");
const CANDIDATE: &str = include_str!("../../../examples/benchmarks/candidate.json");
const JSON: &str = include_str!("../../../examples/benchmarks/comparison.json");
const TEXT: &str = include_str!("../../../examples/benchmarks/comparison.txt");
fn report(base: &Path, candidate: &Path, format: &str) -> Output {
    run(&[
        OsStr::new("benchmark-report"),
        OsStr::new("--base"),
        base.as_os_str(),
        OsStr::new("--candidate"),
        candidate.as_os_str(),
        OsStr::new("--format"),
        OsStr::new(format),
    ])
}

#[test]
fn speedup_and_percentiles_are_exact_for_the_literal_fixture() {
    let comparison = compare_runs(
        &parse_benchmark(BASE).unwrap(),
        &parse_benchmark(CANDIDATE).unwrap(),
    )
    .unwrap();
    assert_eq!(comparison.speedup, 1.25);
    assert_eq!(comparison.base.latency.p95_ns, 100);
    assert_eq!(comparison.candidate.latency.p99_ns, 80);
}
#[test]
fn golden_reports_show_both_treatments_and_no_identifier_or_path() {
    let base = TempInput::new(BASE.as_bytes());
    let candidate = TempInput::new(CANDIDATE.as_bytes());
    for (format, expected) in [("json", JSON), ("text", TEXT)] {
        let output = report(&base.path, &candidate.path, format);
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
    let output = run(&[
        OsStr::new("benchmark-report"),
        OsStr::new("--format"),
        OsStr::new("text"),
        OsStr::new("--candidate"),
        candidate.path.as_os_str(),
        OsStr::new("--base"),
        base.path.as_os_str(),
    ]);
    assert_eq!(output.stdout, TEXT.as_bytes());
}
#[test]
fn each_required_matching_condition_refuses_comparison_in_frozen_order() {
    let base = parse_benchmark(BASE).unwrap();
    type Mutation = (fn(&mut cache_types::benchmark::BenchmarkRun), CompareError);
    let mutations: [Mutation; 6] = [
        (
            |r| r.workload_id = "different".into(),
            CompareError::WorkloadId,
        ),
        (
            |r| r.environment_id = "different".into(),
            CompareError::EnvironmentId,
        ),
        (
            |r| r.toolchain_id = "different".into(),
            CompareError::ToolchainId,
        ),
        (|r| r.working_set_bytes += 1, CompareError::WorkingSetBytes),
        (|r| r.os_cache = OsCache::Warm, CompareError::OsCache),
        (|r| r.repetitions += 1, CompareError::Repetitions),
    ];
    for (mutate, expected) in mutations {
        let mut candidate = parse_benchmark(CANDIDATE).unwrap();
        mutate(&mut candidate);
        assert_eq!(compare_runs(&base, &candidate), Err(expected));
    }
    let mut candidate = base.clone();
    candidate.workload_id = "other".into();
    candidate.environment_id = "other".into();
    assert_eq!(
        compare_runs(&base, &candidate),
        Err(CompareError::WorkloadId)
    );
}
#[test]
fn public_values_are_revalidated_before_comparison_and_statistics() {
    let base = parse_benchmark(BASE).unwrap();
    let mut candidate = parse_benchmark(CANDIDATE).unwrap();
    candidate.duration_ns = 0;
    assert_eq!(
        compare_runs(&base, &candidate),
        Err(CompareError::InvalidCandidate)
    );
    let mut invalid = base.clone();
    invalid.latency_ns.clear();
    assert_eq!(
        compare_runs(&invalid, &candidate),
        Err(CompareError::InvalidBase)
    );
    let mut unknown = base.clone();
    unknown.os_cache = OsCache::Unknown;
    assert_eq!(compare_runs(&unknown, &unknown), Err(CompareError::OsCache));
    candidate = parse_benchmark(CANDIDATE).unwrap();
    candidate.duration_ns = u64::MAX;
    let mut tiny = base;
    tiny.duration_ns = 1;
    let c = compare_runs(&tiny, &candidate).unwrap();
    assert!(c.speedup.is_finite() && c.speedup > 0.0);
}
#[test]
fn errors_use_fixed_field_names_only() {
    for (error, name) in [
        (CompareError::InvalidBase, "base"),
        (CompareError::InvalidCandidate, "candidate"),
        (CompareError::WorkloadId, "workload_id"),
        (CompareError::EnvironmentId, "environment_id"),
        (CompareError::ToolchainId, "toolchain_id"),
        (CompareError::WorkingSetBytes, "working_set_bytes"),
        (CompareError::OsCache, "os_cache"),
        (CompareError::Repetitions, "repetitions"),
    ] {
        assert_eq!(error.field(), name);
        assert_eq!(error.to_string(), name);
    }
}
#[test]
fn base_is_read_and_parsed_before_candidate_then_comparison() {
    let invalid = TempInput::new(b"not-json-SECRET");
    let valid = TempInput::new(BASE.as_bytes());
    let candidate = TempInput::new(CANDIDATE.as_bytes());
    let missing = valid.root().join("missing-SECRET");
    assert_error(
        report(&invalid.path, &missing, "json"),
        2,
        "base: INVALID_JSON",
    );
    assert_error(report(&missing, &invalid.path, "json"), 3, "base: INPUT_IO");
    assert_error(
        report(&valid.path, &missing, "json"),
        3,
        "candidate: INPUT_IO",
    );
    assert_error(
        report(&valid.path, &invalid.path, "json"),
        2,
        "candidate: INVALID_JSON",
    );
    let changed = TempInput::new(CANDIDATE.replace("lab-A", "secret-environment").as_bytes());
    assert_error(
        report(&valid.path, &changed.path, "json"),
        2,
        "compare: environment_id",
    );
    let zero = TempInput::new(
        BASE.replace("\"duration_ns\":\"1000000000\"", "\"duration_ns\":\"0\"")
            .as_bytes(),
    );
    assert_error(
        report(&zero.path, &candidate.path, "json"),
        2,
        "base: INVALID_VALUE",
    );
}
#[test]
fn invalid_utf8_and_oversized_benchmark_inputs_are_bounded() {
    let valid = TempInput::new(BASE.as_bytes());
    let invalid = TempInput::new(&[0xff]);
    let large = TempInput::new(&vec![b'x'; 16_777_217]);
    assert_error(
        report(&valid.path, &invalid.path, "text"),
        2,
        "candidate: INVALID_JSON",
    );
    assert_error(
        report(&large.path, &valid.path, "json"),
        2,
        "base: INPUT_TOO_LARGE",
    );
}
#[test]
fn report_arguments_cannot_fall_back_to_live_sources() {
    for args in [
        vec!["benchmark-report"],
        vec!["benchmark-report", "--base", "x", "--format", "json"],
        vec![
            "benchmark-report",
            "--base",
            "x",
            "--candidate",
            "y",
            "--format",
            "yaml",
        ],
        vec![
            "benchmark-report",
            "--base",
            "x",
            "--base",
            "y",
            "--candidate",
            "z",
            "--format",
            "json",
        ],
        vec![
            "benchmark-report",
            "--base",
            "x",
            "--candidate",
            "y",
            "--format",
            "json",
            "extra",
        ],
        vec![
            "benchmark-report",
            "--base",
            "x",
            "--candidate",
            "y",
            "--format",
            "json",
            "--format",
            "text",
        ],
    ] {
        let args: Vec<_> = args.iter().map(OsStr::new).collect();
        assert_error(run(&args), 2, "USAGE");
    }
}
