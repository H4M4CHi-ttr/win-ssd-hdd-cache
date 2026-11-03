// This integration target intentionally uses only TempInput and assert_error;
// the shared status-binary helper and root accessor are tested in other targets.
#[allow(dead_code)]
mod common;

use common::{TempInput, assert_error};
use std::process::{Command, Output};

const UNKNOWN: &str = include_str!("../../../tests/fixtures/capture-audit-v2/unknown-read.json");
const FLUSH: &str = include_str!("../../../tests/fixtures/capture-audit-v2/unknown-flush.json");
const FAILURE: &str = include_str!("../../../tests/fixtures/capture-audit-v2/unknown-failure.json");
const KNOWN: &str = include_str!("../../../tests/fixtures/capture-audit-v2/known.json");

fn audit(source: &str) -> Output {
    let input = TempInput::new(source.as_bytes());
    Command::new(env!("CARGO_BIN_EXE_cache-capture-audit"))
        .arg("--input")
        .arg(&input.path)
        .output()
        .unwrap()
}

#[test]
fn v2_unknown_information_has_json_exit_four() {
    for source in [UNKNOWN, FLUSH, FAILURE] {
        let output = audit(source);
        assert_eq!(output.status.code(), Some(4));
        assert!(output.stderr.is_empty());
        assert_eq!(
            output.stdout,
            include_bytes!("../../../tests/fixtures/capture-audit-v2/unknown-report.json")
        );
    }
}

#[test]
fn v2_known_information_has_json_exit_zero() {
    let output = audit(KNOWN);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout,
        include_bytes!("../../../tests/fixtures/capture-audit-v2/known-report.json")
    );
}

#[test]
fn v2_missing_information_is_not_a_nullable_field() {
    // Remove only the completion key; do not normalize the malformed object.
    assert_error(
        audit(&UNKNOWN.replace("\"status\":\"0\",\"information\":null", "\"status\":\"0\"")),
        2,
        "INVALID_CAPTURE",
    );
}

#[test]
fn v1_null_information_is_not_upgraded() {
    assert_error(
        audit(&UNKNOWN.replace("\"schema_version\":2", "\"schema_version\":1")),
        2,
        "INVALID_CAPTURE_VALUE",
    );
}
