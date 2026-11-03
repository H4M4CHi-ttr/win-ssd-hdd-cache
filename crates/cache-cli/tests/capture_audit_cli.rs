mod common;

use common::{TempInput, assert_error};
use std::{
    ffi::OsStr,
    path::Path,
    process::{Command, Output},
};

// Small independently written process fixture. An error/short completion is an
// observation, not evidence that the capture itself is incomplete.
const BASE: &str = concat!(
    "{\"schema_version\":1,\"evidence_kind\":\"synthetic\",\"observation_point\":\"disk_class_upper\",",
    "\"session_id\":\"1\",\"windows_version\":null,\"source_revision\":null,\"producer_sha256\":null,",
    "\"capture_finalized\":true,\"start_inflight\":\"0\",\"end_inflight\":\"0\",",
    "\"events_attempted\":\"2\",\"events_dropped\":\"0\",\"records\":[",
    "{\"seq\":\"0\",\"phase\":\"submit\",\"request_id\":\"1\",\"target_id\":\"1\",\"operation\":\"read\",\"offset\":\"0\",\"length\":\"2\",\"status\":null,\"information\":null},",
    "{\"seq\":\"1\",\"phase\":\"complete\",\"request_id\":\"1\",\"target_id\":\"1\",\"operation\":null,\"offset\":null,\"length\":null,\"status\":\"3221225473\",\"information\":\"1\"}]}\n"
);
const GOLDEN: &str = concat!(
    "{\"schema_version\":1,\"evidence_kind\":\"synthetic\",\"observation_point\":\"disk_class_upper\",\"verdict\":\"internally_complete\",",
    "\"records\":\"2\",\"submits\":\"1\",\"completions\":\"1\",\"paired_requests\":\"1\",\"pending_requests\":\"0\",\"orphan_completions\":\"0\",\"reasons\":[],",
    "\"producer_verified\":false,\"authorization_verified\":false,\"coverage_verified\":false,\"kernel_runtime_qualified\":false}\n"
);

fn run(args: &[&OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_cache-capture-audit"))
        .args(args)
        .output()
        .unwrap()
}

fn audit(path: &Path) -> Output {
    run(&[OsStr::new("--input"), path.as_os_str()])
}

#[test]
fn complete_capture_has_exact_stdout_and_zero_exit_without_qualification() {
    let file = TempInput::new(BASE.as_bytes());
    let output = audit(&file.path);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, GOLDEN.as_bytes());
}

#[test]
fn incomplete_capture_has_json_and_exit_four_not_success_or_parser_failure() {
    let file = TempInput::new(
        BASE.replace("\"capture_finalized\":true", "\"capture_finalized\":false")
            .as_bytes(),
    );
    let output = audit(&file.path);
    assert_eq!(output.status.code(), Some(4));
    assert!(output.stderr.is_empty());
    let expected = GOLDEN
        .replace("internally_complete", "incomplete")
        .replace("\"reasons\":[]", "\"reasons\":[\"NOT_FINALIZED\"]");
    assert_eq!(output.stdout, expected.as_bytes());
}

#[test]
fn missing_duplicate_unknown_or_extra_arguments_are_usage_without_echo() {
    for args in [
        vec![],
        vec!["--input"],
        vec!["--input", "--input"],
        vec![
            "--input",
            "private-sentinel",
            "--input",
            "other-private-sentinel",
        ],
        vec!["--unknown", "private-sentinel"],
        vec!["private-sentinel"],
        vec!["--input", "private-sentinel", "extra"],
        vec!["--input", "private-sentinel", "--format", "json"],
        vec!["status"],
        vec!["--help"],
    ] {
        let args: Vec<_> = args.iter().map(OsStr::new).collect();
        assert_error(run(&args), 2, "USAGE");
    }
}

#[test]
fn syntax_version_and_value_errors_have_fixed_nonreflecting_stderr() {
    for (source, code) in [
        (BASE.to_owned() + "PRIVATE-SENTINEL", "INVALID_CAPTURE"),
        (
            BASE.replace("\"schema_version\":1", "\"schema_version\":2"),
            "UNSUPPORTED_CAPTURE_VERSION",
        ),
        (
            BASE.replace(
                "\"evidence_kind\":\"synthetic\"",
                "\"evidence_kind\":\"PRIVATE-SENTINEL\"",
            ),
            "INVALID_CAPTURE_VALUE",
        ),
        (
            BASE.replace(
                "\"session_id\":\"1\"",
                "\"session_id\":\"1\",\"private_path\":\"PRIVATE-SENTINEL\"",
            ),
            "INVALID_CAPTURE",
        ),
        (
            BASE.replace(
                "\"schema_version\":1",
                "\"schema_version\":1,\"schema_vers\\u0069on\":1",
            ),
            "INVALID_CAPTURE",
        ),
        (
            "{\"schema_version\":1,\"events\":[]}".to_owned(),
            "INVALID_CAPTURE",
        ),
    ] {
        let file = TempInput::new(source.as_bytes());
        assert_error(audit(&file.path), 2, code);
    }
}

#[test]
fn exact_byte_bound_is_read_but_plus_one_and_utf8_fail_without_output() {
    let source = format!("{BASE}{}", " ".repeat(1_048_576 - BASE.len()));
    let file = TempInput::new(source.as_bytes());
    let output = audit(&file.path);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, GOLDEN.as_bytes());
    let file = TempInput::new((source + " ").as_bytes());
    assert_error(audit(&file.path), 2, "CAPTURE_TOO_LARGE");
    let file = TempInput::new(&[0xff, 0xfe]);
    assert_error(audit(&file.path), 2, "INVALID_CAPTURE");
}

#[test]
fn record_bound_failure_is_not_mistaken_for_incomplete_evidence() {
    let prefix = BASE.split_once("\"records\":[").unwrap().0;
    let record = "{\"seq\":\"0\",\"phase\":\"submit\",\"request_id\":\"1\",\"target_id\":\"1\",\"operation\":\"read\",\"offset\":\"0\",\"length\":\"2\",\"status\":null,\"information\":null}";
    let source = format!("{prefix}\"records\":[{}]}}", vec![record; 4_097].join(","));
    assert!(source.len() < 1_048_576);
    let file = TempInput::new(source.as_bytes());
    assert_error(audit(&file.path), 2, "TOO_MANY_CAPTURE_RECORDS");
}

#[test]
fn missing_file_and_directory_are_fixed_io_errors_without_path_echo() {
    let file = TempInput::new(BASE.as_bytes());
    assert_error(
        audit(&file.root().join("missing-PRIVATE-SENTINEL")),
        3,
        "INPUT_IO",
    );
    assert_error(audit(file.root()), 3, "INPUT_IO");
}

#[test]
fn original_status_binary_remains_available_and_unchanged() {
    let source = include_str!("../../../examples/status/disabled.json");
    let file = TempInput::new(source.as_bytes());
    let output = common::run(&[
        OsStr::new("status"),
        OsStr::new("--input"),
        file.path.as_os_str(),
        OsStr::new("--format"),
        OsStr::new("json"),
    ]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, source.as_bytes());
}

#[cfg(unix)]
#[test]
fn non_utf8_file_name_is_supported_but_non_utf8_option_is_usage() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let file = TempInput::named(
        &OsString::from_vec(b"input_\xff.json".to_vec()),
        BASE.as_bytes(),
    );
    let output = audit(&file.path);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, GOLDEN.as_bytes());
    let bad_option = OsString::from_vec(b"--private-\xff".to_vec());
    assert_error(
        run(&[bad_option.as_os_str(), file.path.as_os_str()]),
        2,
        "USAGE",
    );
}

#[cfg(unix)]
#[test]
fn existing_symlink_is_refused_without_following_its_contents() {
    let file = TempInput::new(BASE.as_bytes());
    let link = file.root().join("private-link");
    std::os::unix::fs::symlink(&file.path, &link).unwrap();
    assert_error(audit(&link), 3, "INPUT_IO");
}

#[cfg(windows)]
#[test]
fn forbidden_windows_namespaces_are_rejected_before_open_without_echo() {
    // These are already rejected by input.rs before any metadata/open call.
    // This exercises the new binary's guard/error wiring, not live device I/O.
    for path in [
        r"\\.\PhysicalDrive0",
        r"\\.\pipe\anything",
        r"\\?\GLOBALROOT\Device\Harddisk0",
        r"\\server\share\input.json",
        r"\\?\UNC\server\share\input.json",
        r"C:\NUL.json",
    ] {
        assert_error(audit(Path::new(path)), 3, "INPUT_IO");
    }
}
