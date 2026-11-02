mod common;
use cache_types::status::{OutputFormat, parse_status, render_status};
use common::{TempInput, assert_error, run, status};
use std::ffi::OsStr;
const BASE: &str = include_str!("../../../examples/status/disabled.json");
const TEXT: &str = include_str!("../../../examples/status/disabled.txt");

#[test]
fn all_states_have_exact_json_and_text_with_lf_only() {
    for state in [
        "disabled",
        "validating",
        "shadow",
        "active",
        "quiescing",
        "degraded",
        "removing",
    ] {
        let json = BASE.replace("disabled", state);
        let text = TEXT.replace("disabled", state);
        let parsed = parse_status(&json).unwrap();
        assert_eq!(render_status(&parsed, OutputFormat::Json), json);
        assert_eq!(render_status(&parsed, OutputFormat::Text), text);
        let input = TempInput::new(json.as_bytes());
        for (format, expected) in [("json", json.as_str()), ("text", text.as_str())] {
            let output = status(&input.path, format);
            assert!(output.status.success());
            assert!(output.stderr.is_empty());
            assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
        }
    }
}
#[test]
fn every_error_description_is_fixed_and_null_is_not_zero() {
    for (code, description) in [
        ("CACHE_IO", "Cache I/O failed."),
        ("CACHE_CORRUPT", "Cache data failed validation."),
        ("TARGET_CHANGED", "Target identity changed."),
        ("UNSUPPORTED", "Configuration is unsupported."),
        ("RESOURCE_LIMIT", "A resource limit was reached."),
        ("QUIESCE_PENDING", "Cache shutdown is still pending."),
    ] {
        let json = BASE
            .replace("\"error_code\":null", &format!("\"error_code\":\"{code}\""))
            .replace("\"read_hits\":null", "\"read_hits\":\"0\"");
        let expected = TEXT
            .replace("read_hits: unavailable", "read_hits: 0")
            .replace("error_code: none", &format!("error_code: {code}"))
            .replace(
                "error_description: none",
                &format!("error_description: {description}"),
            );
        assert_eq!(
            render_status(&parse_status(&json).unwrap(), OutputFormat::Text),
            expected
        );
    }
}
#[test]
fn flags_are_required_exactly_once_and_order_is_flexible() {
    let input = TempInput::new(BASE.as_bytes());
    let output = run(&[
        OsStr::new("status"),
        OsStr::new("--format"),
        OsStr::new("json"),
        OsStr::new("--input"),
        input.path.as_os_str(),
    ]);
    assert!(output.status.success());
    assert_eq!(output.stdout, BASE.as_bytes());
    assert!(output.stderr.is_empty());
    for args in [
        vec![],
        vec!["status"],
        vec!["live"],
        vec!["status", "--input"],
        vec!["status", "--format", "text"],
        vec!["status", "--input", "x", "--format", "yaml"],
        vec!["status", "--input", "x", "--format", "text", "extra"],
        vec!["status", "--input", "x", "--input", "y", "--format", "text"],
        vec![
            "status", "--input", "x", "--format", "text", "--format", "json",
        ],
        vec!["status", "--unknown", "x", "--format", "text"],
    ] {
        let args: Vec<_> = args.iter().map(OsStr::new).collect();
        assert_error(run(&args), 2, "USAGE");
    }
}
#[test]
fn schema_syntax_and_value_failures_are_nonreflecting() {
    for (input, code) in [
        (
            BASE.replace("\"schema_version\":1", "\"schema_version\":2"),
            "INVALID_SCHEMA",
        ),
        (
            BASE.replace("disabled", "PRIVATE-SENTINEL"),
            "INVALID_VALUE",
        ),
        (BASE.to_owned() + "PRIVATE-SENTINEL", "INVALID_JSON"),
        (
            BASE.replace("\"error_code\":null", "\"serial\":\"PRIVATE-SENTINEL\""),
            "INVALID_SCHEMA",
        ),
    ] {
        let file = TempInput::new(input.as_bytes());
        assert_error(status(&file.path, "json"), 2, code);
    }
}
#[test]
fn oversized_and_invalid_utf8_inputs_are_bounded_errors() {
    let file = TempInput::new(&vec![b'x'; 65_537]);
    assert_error(status(&file.path, "json"), 2, "INPUT_TOO_LARGE");
    let file = TempInput::new(&[0xff]);
    assert_error(status(&file.path, "text"), 2, "INVALID_JSON");
    let padded = format!("{BASE}{}", " ".repeat(65_536 - BASE.len()));
    let file = TempInput::new(padded.as_bytes());
    assert_eq!(status(&file.path, "json").stdout, BASE.as_bytes());
}
#[test]
fn missing_file_and_directory_return_only_fixed_io_errors() {
    let file = TempInput::new(BASE.as_bytes());
    assert_error(
        status(&file.root().join("missing-PRIVATE-SENTINEL"), "json"),
        3,
        "INPUT_IO",
    );
    assert_error(status(file.root(), "json"), 3, "INPUT_IO");
}
#[cfg(unix)]
#[test]
fn unix_non_utf8_paths_are_not_rejected_or_printed() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};
    let file = TempInput::named(
        &OsString::from_vec(b"input_\xff.json".to_vec()),
        BASE.as_bytes(),
    );
    let output = status(&file.path, "text");
    assert!(output.status.success());
    assert_eq!(output.stdout, TEXT.as_bytes());
    assert!(output.stderr.is_empty());
}
#[cfg(unix)]
#[test]
fn symlink_inputs_are_rejected_before_reading() {
    let file = TempInput::new(BASE.as_bytes());
    let link = file.root().join("link");
    std::os::unix::fs::symlink(&file.path, &link).unwrap();
    assert_error(status(&link, "json"), 3, "INPUT_IO");
}
