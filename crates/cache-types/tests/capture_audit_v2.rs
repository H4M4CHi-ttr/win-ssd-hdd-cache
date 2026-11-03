use cache_types::capture::{CAPTURE_MAX_BYTES, CAPTURE_MAX_RECORDS, CaptureError, audit_capture};
use serde_json::{Value, json};

// All literal outputs and schedule expectations come from the reviewed P03
// contract, not from a recording of the implementation under test.
const UNKNOWN: &str = include_str!("../../../tests/fixtures/capture-audit-v2/unknown-read.json");
const FLUSH: &str = include_str!("../../../tests/fixtures/capture-audit-v2/unknown-flush.json");
const FAILURE: &str = include_str!("../../../tests/fixtures/capture-audit-v2/unknown-failure.json");
const KNOWN: &str = include_str!("../../../tests/fixtures/capture-audit-v2/known.json");
const UNKNOWN_REPORT: &str =
    include_str!("../../../tests/fixtures/capture-audit-v2/unknown-report.json");
const KNOWN_REPORT: &str =
    include_str!("../../../tests/fixtures/capture-audit-v2/known-report.json");

fn base() -> Value {
    serde_json::from_str(UNKNOWN).unwrap()
}

fn check(input: &Value, counts: [u64; 6], reasons: &[&str]) {
    let audit = audit_capture(&input.to_string()).unwrap();
    assert_eq!(audit.is_internally_complete(), reasons.is_empty());
    let output = audit.to_json();
    assert!(output.ends_with('\n'));
    assert!(!output.contains('\r'));
    let report: Value = serde_json::from_str(&output).unwrap();
    assert_eq!(report["schema_version"], 2);
    assert_eq!(report["reasons"], json!(reasons));
    for (key, count) in [
        "records",
        "submits",
        "completions",
        "paired_requests",
        "pending_requests",
        "orphan_completions",
    ]
    .into_iter()
    .zip(counts)
    {
        assert_eq!(report[key], json!(count.to_string()), "{key}");
    }
    for key in [
        "producer_verified",
        "authorization_verified",
        "coverage_verified",
        "kernel_runtime_qualified",
    ] {
        assert_eq!(report[key], false, "{key}");
    }
}

fn schedule(input: &mut Value, records: Vec<Value>) {
    input["events_attempted"] = json!(records.len().to_string());
    input["records"] = json!(records);
    for (index, record) in input["records"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        record["seq"] = json!(index.to_string());
    }
}

fn error(input: &str, expected: CaptureError) {
    assert_eq!(audit_capture(input).unwrap_err(), expected);
}

#[test]
fn literal_v2_unknown_completions_preserve_pairing() {
    for source in [UNKNOWN, FLUSH, FAILURE] {
        assert_eq!(audit_capture(source).unwrap().to_json(), UNKNOWN_REPORT);
        check(
            &serde_json::from_str(source).unwrap(),
            [2, 1, 1, 1, 0, 0],
            &["UNKNOWN_INFORMATION"],
        );
    }
}

#[test]
fn literal_v2_known_report_is_complete_but_unqualified() {
    assert_eq!(audit_capture(KNOWN).unwrap().to_json(), KNOWN_REPORT);
    check(
        &serde_json::from_str(KNOWN).unwrap(),
        [2, 1, 1, 1, 0, 0],
        &[],
    );
}

#[test]
fn v1_null_information_and_golden_bytes_remain_frozen() {
    error(
        &UNKNOWN.replace("\"schema_version\":2", "\"schema_version\":1"),
        CaptureError::InvalidValue,
    );
    assert_eq!(
        audit_capture(include_str!(
            "../../../tests/fixtures/capture-audit/complete-synthetic.json"
        ))
        .unwrap()
        .to_json(),
        include_str!("../../../tests/fixtures/capture-audit/complete-report.json")
    );
}

#[test]
fn v2_unknown_information_and_actual_loss_are_independent() {
    let mut only_submit = base();
    only_submit["records"].as_array_mut().unwrap().pop();
    only_submit["events_dropped"] = json!("1");
    check(
        &only_submit,
        [1, 1, 0, 0, 1, 0],
        &["DROPPED_EVENTS", "SEQUENCE_GAP", "UNPAIRED_REQUEST"],
    );
    let mut trailing_loss = base();
    trailing_loss["events_attempted"] = json!("3");
    trailing_loss["events_dropped"] = json!("1");
    check(
        &trailing_loss,
        [2, 1, 1, 1, 0, 0],
        &["DROPPED_EVENTS", "SEQUENCE_GAP", "UNKNOWN_INFORMATION"],
    );
}

#[test]
fn v2_unknown_information_survives_duplicate_orphan_and_target_mismatch() {
    let b = base();
    let submit = b["records"][0].clone();
    let unknown = b["records"][1].clone();
    let mut known = unknown.clone();
    known["information"] = json!("2");
    for completions in [
        vec![unknown.clone(), known.clone()],
        vec![known.clone(), unknown.clone()],
    ] {
        let mut input = base();
        schedule(
            &mut input,
            vec![
                submit.clone(),
                completions[0].clone(),
                completions[1].clone(),
            ],
        );
        check(
            &input,
            [3, 1, 2, 1, 0, 0],
            &["DUPLICATE_REQUEST", "UNKNOWN_INFORMATION"],
        );
    }
    let mut orphan = base();
    schedule(
        &mut orphan,
        vec![unknown.clone(), submit.clone(), known.clone()],
    );
    orphan["end_inflight"] = json!("1");
    check(
        &orphan,
        [3, 1, 2, 0, 1, 1],
        &[
            "BOUNDARY_INFLIGHT",
            "DUPLICATE_REQUEST",
            "UNPAIRED_REQUEST",
            "UNKNOWN_INFORMATION",
        ],
    );
    let mut mismatch = base();
    let mut wrong_target = unknown;
    wrong_target["target_id"] = json!("2");
    schedule(&mut mismatch, vec![submit, wrong_target, known]);
    mismatch["end_inflight"] = json!("1");
    check(
        &mismatch,
        [3, 1, 2, 0, 1, 0],
        &[
            "BOUNDARY_INFLIGHT",
            "DUPLICATE_REQUEST",
            "TARGET_MISMATCH",
            "UNPAIRED_REQUEST",
            "UNKNOWN_INFORMATION",
        ],
    );
}

#[test]
fn v2_boundary_range_and_provenance_uncertainties_keep_order() {
    let mut input = base();
    input["records"][0]["offset"] = Value::Null;
    input["records"][0]["length"] = Value::Null;
    input["capture_finalized"] = json!(false);
    input["start_inflight"] = Value::Null;
    input["end_inflight"] = Value::Null;
    input["evidence_kind"] = json!("windows_observation");
    check(
        &input,
        [2, 1, 1, 1, 0, 0],
        &[
            "NOT_FINALIZED",
            "UNKNOWN_COUNTERS",
            "UNKNOWN_RANGE",
            "MISSING_PROVENANCE",
            "UNKNOWN_INFORMATION",
        ],
    );
    let mut input = base();
    input["records"][0]["operation"] = json!("other");
    input["records"][0]["offset"] = Value::Null;
    input["records"][0]["length"] = Value::Null;
    check(
        &input,
        [2, 1, 1, 1, 0, 0],
        &["UNCLASSIFIED_OPERATION", "UNKNOWN_INFORMATION"],
    );
}

#[test]
fn v2_required_presence_and_escaped_duplicates_are_strict() {
    for key in ["information", "status"] {
        let mut input = base();
        input["records"][1].as_object_mut().unwrap().remove(key);
        error(&input.to_string(), CaptureError::InvalidCapture);
    }
    for input in [
        UNKNOWN.replace(
            "\"information\":null",
            "\"information\":null,\"inform\\u0061tion\":null",
        ),
        UNKNOWN.replace(
            "\"schema_version\":2",
            "\"schema_version\":2,\"schema_vers\\u0069on\":2",
        ),
        UNKNOWN.replace(
            "\"schema_version\":2",
            "\"schema_version\":2,\"extra\":null",
        ),
        "[]".to_owned(),
        format!("{UNKNOWN}{{}}"),
    ] {
        error(&input, CaptureError::InvalidCapture);
    }
    let mut input = base();
    input["records"][1] = json!([]);
    error(&input.to_string(), CaptureError::InvalidCapture);
    let mut input = base();
    input["records"][1]["extra"] = Value::Null;
    error(&input.to_string(), CaptureError::InvalidCapture);
}

#[test]
fn v2_null_status_and_nonnull_submit_information_are_invalid() {
    for (record, key, value, expected) in [
        (1, "status", Value::Null, CaptureError::InvalidValue),
        (1, "status", json!("4294967296"), CaptureError::InvalidValue),
        (0, "information", json!("0"), CaptureError::InvalidValue),
        (1, "information", json!(2), CaptureError::InvalidCapture),
        (1, "information", json!("00"), CaptureError::InvalidValue),
    ] {
        let mut input = base();
        input["records"][record][key] = value;
        error(&input.to_string(), expected);
    }
}

#[test]
fn v2_known_information_extremes_are_not_normalized() {
    for information in ["0", "1", "2", "18446744073709551615"] {
        for status in ["0", "3221225473", "4294967295"] {
            let mut input = base();
            input["records"][1]["information"] = json!(information);
            input["records"][1]["status"] = json!(status);
            check(&input, [2, 1, 1, 1, 0, 0], &[]);
        }
    }
}

#[test]
fn v2_versions_and_error_precedence_are_closed() {
    for token in ["2.0", "2e0", "true", "\"2\"", "-0", "18446744073709551616"] {
        error(
            &KNOWN.replace(
                "\"schema_version\":2",
                &format!("\"schema_version\":{token}"),
            ),
            CaptureError::InvalidCapture,
        );
    }
    for version in [0_u64, 3, u64::MAX] {
        error(
            &KNOWN.replace(
                "\"schema_version\":2",
                &format!("\"schema_version\":{version}"),
            ),
            CaptureError::UnsupportedVersion,
        );
    }
    let malformed = "{\"schema_version\":3,\"records\":[]}";
    error(malformed, CaptureError::InvalidCapture);
    error(
        &format!("{malformed}{}", " ".repeat(CAPTURE_MAX_BYTES + 1)),
        CaptureError::TooLarge,
    );
    let mut input = base();
    let submit = input["records"][0].clone();
    schedule(&mut input, vec![submit; 4097]);
    input["schema_version"] = json!(3);
    assert!(input.to_string().len() < CAPTURE_MAX_BYTES);
    error(&input.to_string(), CaptureError::UnsupportedVersion);
}

#[test]
fn v2_byte_and_record_limits_accept_exact_reject_plus_one() {
    assert_eq!(CAPTURE_MAX_BYTES, 1_048_576);
    assert_eq!(CAPTURE_MAX_RECORDS, 4096);
    let padded = format!("{KNOWN}{}", " ".repeat(CAPTURE_MAX_BYTES - KNOWN.len()));
    assert_eq!(padded.len(), CAPTURE_MAX_BYTES);
    assert_eq!(audit_capture(&padded).unwrap().to_json(), KNOWN_REPORT);
    error(&(padded + " "), CaptureError::TooLarge);

    let mut input = base();
    let submit = input["records"][0].clone();
    schedule(&mut input, vec![submit.clone(); 4096]);
    input["end_inflight"] = json!("1");
    assert!(input.to_string().len() < CAPTURE_MAX_BYTES);
    check(
        &input,
        [4096, 4096, 0, 0, 1, 0],
        &["BOUNDARY_INFLIGHT", "DUPLICATE_REQUEST", "UNPAIRED_REQUEST"],
    );
    schedule(&mut input, vec![submit; 4097]);
    error(&input.to_string(), CaptureError::TooManyRecords);
}

#[test]
fn v2_sequence_defects_are_never_sorted_or_repaired() {
    let mut input = base();
    input["records"][1]["seq"] = json!("0");
    check(
        &input,
        [2, 1, 1, 1, 0, 0],
        &["SEQUENCE_ORDER", "SEQUENCE_GAP", "UNKNOWN_INFORMATION"],
    );
    input["records"][1]["seq"] = json!("18446744073709551615");
    input["events_attempted"] = Value::Null;
    check(
        &input,
        [2, 1, 1, 1, 0, 0],
        &["UNKNOWN_COUNTERS", "SEQUENCE_GAP", "UNKNOWN_INFORMATION"],
    );
}
