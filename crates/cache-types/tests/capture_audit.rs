use cache_types::capture::{CAPTURE_MAX_BYTES, CAPTURE_MAX_RECORDS, CaptureError, audit_capture};
use serde_json::{Value, json};

const BASE: &str = include_str!("../../../tests/fixtures/capture-audit/complete-synthetic.json");
const GOLDEN: &str = include_str!("../../../tests/fixtures/capture-audit/complete-report.json");
const OBSERVED: &str = include_str!("../../../tests/fixtures/capture-audit/complete-observed.json");
fn base() -> Value {
    serde_json::from_str(BASE).unwrap()
}
fn report(input: &Value) -> Value {
    serde_json::from_str(&audit_capture(&input.to_string()).unwrap().to_json()).unwrap()
}
fn reasons(input: &Value, expected: &[&str]) {
    assert_eq!(report(input)["reasons"], json!(expected));
}
fn counts(input: &Value, expected: [u64; 6]) {
    let got = report(input);
    for (key, n) in [
        "records",
        "submits",
        "completions",
        "paired_requests",
        "pending_requests",
        "orphan_completions",
    ]
    .into_iter()
    .zip(expected)
    {
        assert_eq!(got[key], json!(n.to_string()), "{key}");
    }
}
fn set_records(input: &mut Value, records: Vec<Value>) {
    input["events_attempted"] = json!(records.len().to_string());
    input["records"] = json!(records);
    for (i, r) in input["records"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        r["seq"] = json!(i.to_string());
    }
}

#[test]
fn literal_complete_report_is_exact_deterministic_and_never_qualification() {
    let a = audit_capture(BASE).unwrap();
    assert!(a.is_internally_complete());
    assert_eq!(a.to_json(), GOLDEN);
    assert_eq!(audit_capture(BASE).unwrap().to_json(), GOLDEN);
    assert!(!GOLDEN.contains('\r'));
    counts(&base(), [4, 2, 2, 2, 0, 0]);
}

#[test]
fn hand_written_incomplete_fixtures_have_exact_counts_and_reason_order() {
    for (source, expected, codes) in [
        (
            include_str!("../../../tests/fixtures/capture-audit/missing-completion.json"),
            [3, 2, 1, 1, 1, 0],
            vec!["BOUNDARY_INFLIGHT", "UNPAIRED_REQUEST"],
        ),
        (
            include_str!("../../../tests/fixtures/capture-audit/dropped-submit.json"),
            [3, 1, 2, 1, 0, 1],
            vec!["DROPPED_EVENTS", "SEQUENCE_GAP", "UNPAIRED_REQUEST"],
        ),
        (
            include_str!("../../../tests/fixtures/capture-audit/unknown-operation-range.json"),
            [4, 2, 2, 2, 0, 0],
            vec!["UNKNOWN_RANGE", "UNCLASSIFIED_OPERATION"],
        ),
        (
            include_str!("../../../tests/fixtures/capture-audit/unfinalized.json"),
            [4, 2, 2, 2, 0, 0],
            vec!["NOT_FINALIZED"],
        ),
    ] {
        let input: Value = serde_json::from_str(source).unwrap();
        assert!(!audit_capture(source).unwrap().is_internally_complete());
        counts(&input, expected);
        reasons(&input, &codes);
    }
}

#[test]
fn plausible_windows_metadata_is_not_authenticated_or_echoed() {
    let r = audit_capture(OBSERVED).unwrap();
    assert!(r.is_internally_complete());
    let value: Value = serde_json::from_str(&r.to_json()).unwrap();
    for key in [
        "producer_verified",
        "authorization_verified",
        "coverage_verified",
        "kernel_runtime_qualified",
    ] {
        assert_eq!(value[key], false);
    }
    for secret in ["10.0.26100.1", &"a".repeat(40), &"b".repeat(64)] {
        assert!(!r.to_json().contains(secret));
    }
    let mut input: Value = serde_json::from_str(OBSERVED).unwrap();
    for key in ["windows_version", "source_revision", "producer_sha256"] {
        let saved = input[key].take();
        reasons(&input, &["MISSING_PROVENANCE"]);
        input[key] = saved;
    }
}

#[test]
fn first_submit_target_is_immutable_across_duplicate_submissions() {
    let mut input = base();
    let first = input["records"][0].clone();
    let mut duplicate = first.clone();
    duplicate["target_id"] = json!("2");
    let mut complete = input["records"][3].clone();
    complete["target_id"] = json!("2");
    set_records(&mut input, vec![first, duplicate, complete]);
    input["end_inflight"] = json!("1");
    counts(&input, [3, 2, 1, 0, 1, 0]);
    reasons(
        &input,
        &[
            "BOUNDARY_INFLIGHT",
            "DUPLICATE_REQUEST",
            "TARGET_MISMATCH",
            "UNPAIRED_REQUEST",
        ],
    );
    input["records"][2]["target_id"] = json!("1");
    input["end_inflight"] = json!("0");
    counts(&input, [3, 2, 1, 1, 0, 0]);
    reasons(&input, &["DUPLICATE_REQUEST"]);
}

#[test]
fn orphan_completion_consumes_identity_and_later_records_do_not_repair_it() {
    let mut input = base();
    let complete = input["records"][3].clone();
    let submit = input["records"][0].clone();
    set_records(&mut input, vec![complete.clone(), submit, complete]);
    input["end_inflight"] = json!("1");
    counts(&input, [3, 1, 2, 0, 1, 1]);
    reasons(
        &input,
        &["BOUNDARY_INFLIGHT", "DUPLICATE_REQUEST", "UNPAIRED_REQUEST"],
    );
}

#[test]
fn target_mismatch_completion_is_not_repaired_by_repeated_completion() {
    let mut input = base();
    let submit = input["records"][0].clone();
    let complete = input["records"][3].clone();
    let mut wrong = complete.clone();
    wrong["target_id"] = json!("2");
    set_records(&mut input, vec![submit, wrong, complete]);
    input["end_inflight"] = json!("1");
    counts(&input, [3, 1, 2, 0, 1, 0]);
    reasons(
        &input,
        &[
            "BOUNDARY_INFLIGHT",
            "DUPLICATE_REQUEST",
            "TARGET_MISMATCH",
            "UNPAIRED_REQUEST",
        ],
    );
}

#[test]
fn duplicate_after_completion_neither_reopens_nor_double_pairs_request() {
    let mut input = base();
    let submit = input["records"][0].clone();
    let complete = input["records"][3].clone();
    set_records(
        &mut input,
        vec![submit.clone(), complete.clone(), submit, complete],
    );
    counts(&input, [4, 2, 2, 1, 0, 0]);
    reasons(&input, &["DUPLICATE_REQUEST"]);
}

#[test]
fn sequence_reorder_and_duplicate_are_order_and_gap_not_sorted_success() {
    let mut input = base();
    input["records"].as_array_mut().unwrap().swap(0, 1);
    reasons(&input, &["SEQUENCE_ORDER", "SEQUENCE_GAP"]);
    let mut input = base();
    input["records"][3]["seq"] = json!("2");
    reasons(&input, &["SEQUENCE_ORDER", "SEQUENCE_GAP"]);
}

#[test]
fn attempted_ordinals_and_loss_counters_include_trailing_loss() {
    let mut input = base();
    input["events_dropped"] = json!("1");
    reasons(&input, &["DROPPED_EVENTS", "COUNT_MISMATCH"]);
    input["events_attempted"] = json!("5");
    reasons(&input, &["DROPPED_EVENTS", "SEQUENCE_GAP"]);
    input["events_dropped"] = json!(u64::MAX.to_string());
    input["events_attempted"] = json!("4");
    reasons(&input, &["DROPPED_EVENTS", "COUNT_MISMATCH"]);
}

#[test]
fn unknown_boundaries_and_empty_capture_cannot_pass() {
    for key in [
        "start_inflight",
        "end_inflight",
        "events_attempted",
        "events_dropped",
    ] {
        let mut input = base();
        input[key] = Value::Null;
        reasons(&input, &["UNKNOWN_COUNTERS"]);
    }
    let mut input = base();
    input["start_inflight"] = json!("1");
    reasons(&input, &["BOUNDARY_INFLIGHT"]);
    input["start_inflight"] = json!("0");
    input["end_inflight"] = json!("1");
    reasons(&input, &["BOUNDARY_INFLIGHT", "COUNT_MISMATCH"]);
    input["end_inflight"] = json!("0");
    set_records(&mut input, vec![]);
    reasons(&input, &["EMPTY_CAPTURE"]);
    counts(&input, [0; 6]);
    input["events_attempted"] = json!("1");
    input["events_dropped"] = json!("1");
    reasons(&input, &["EMPTY_CAPTURE", "DROPPED_EVENTS", "SEQUENCE_GAP"]);
}

#[test]
fn sequence_overflow_never_wraps_to_a_contiguous_capture() {
    let mut input = base();
    input["records"][3]["seq"] = json!(u64::MAX.to_string());
    input["events_attempted"] = Value::Null;
    reasons(&input, &["UNKNOWN_COUNTERS", "SEQUENCE_GAP"]);
    input["events_attempted"] = json!("4");
    reasons(&input, &["SEQUENCE_GAP"]);
}

#[test]
fn strict_objects_missing_nulls_unknowns_and_escaped_duplicates_are_rejected() {
    let record = base()["records"][0].to_string();
    let inputs = [
        "[]".to_owned(),
        "null".to_owned(),
        format!("{BASE}{{}}"),
        BASE.replacen(
            "\"schema_version\":1",
            "\"schema_version\":1,\"schema_vers\\u0069on\":1",
            1,
        ),
        BASE.replacen("\"records\":[", "\"records\":[],\"rec\\u006frds\":[", 1),
        BASE.replacen("\"seq\":\"0\"", "\"seq\":\"0\",\"s\\u0065q\":\"0\"", 1),
        BASE.replacen("\"source_revision\":null,", "", 1),
        BASE.replacen("\"status\":null,", "", 1),
        BASE.replacen(
            "\"session_id\":\"1\"",
            "\"session_id\":\"1\",\"private_path\":\"secret\"",
            1,
        ),
        BASE.replacen("\"seq\":\"0\"", "\"seq\":\"0\",\"payload\":\"secret\"", 1),
    ];
    for input in inputs {
        assert_ne!(input, BASE);
        assert_eq!(
            audit_capture(&input).unwrap_err(),
            CaptureError::InvalidCapture
        );
    }
    let mut input = base();
    input["records"][0] = json!([record]);
    assert_eq!(
        audit_capture(&input.to_string()).unwrap_err(),
        CaptureError::InvalidCapture
    );
}

#[test]
fn version_tokens_and_numeric_string_lexemes_are_strict() {
    for token in ["true", "\"1\"", "1.0", "1e0", "-0", "18446744073709551616"] {
        let source = BASE.replacen(
            "\"schema_version\":1",
            &format!("\"schema_version\":{token}"),
            1,
        );
        assert_eq!(
            audit_capture(&source).unwrap_err(),
            CaptureError::InvalidCapture
        );
    }
    let mut input = base();
    input["schema_version"] = json!(2);
    assert_eq!(
        audit_capture(&input.to_string()).unwrap_err(),
        CaptureError::UnsupportedVersion
    );
    for text in [
        "",
        "00",
        "+1",
        "-1",
        "1.0",
        "1e0",
        " 1",
        "18446744073709551616",
    ] {
        let mut input = base();
        input["events_attempted"] = json!(text);
        assert_eq!(
            audit_capture(&input.to_string()).unwrap_err(),
            CaptureError::InvalidValue
        );
    }
    for value in [json!(1), json!(true), json!([])] {
        let mut input = base();
        input["records"][0]["seq"] = value;
        assert_eq!(
            audit_capture(&input.to_string()).unwrap_err(),
            CaptureError::InvalidCapture
        );
    }
}

#[test]
fn phase_required_nulls_and_range_arithmetic_are_validated_without_io_inference() {
    for (idx, key, value) in [
        (0, "status", json!("0")),
        (3, "operation", json!("read")),
        (0, "length", Value::Null),
        (0, "request_id", json!("0")),
        (3, "status", json!("4294967296")),
        (0, "phase", json!("apply")),
    ] {
        let mut input = base();
        input["records"][idx][key] = value;
        assert_eq!(
            audit_capture(&input.to_string()).unwrap_err(),
            CaptureError::InvalidValue,
            "{key}"
        );
    }
    let mut input = base();
    input["records"][0]["offset"] = json!(u64::MAX.to_string());
    assert_eq!(
        audit_capture(&input.to_string()).unwrap_err(),
        CaptureError::InvalidValue
    );
    input["records"][0]["length"] = json!("0");
    assert!(
        audit_capture(&input.to_string())
            .unwrap()
            .is_internally_complete()
    );
}

#[test]
fn provenance_shape_is_strict_but_synthetic_null_is_not_forged() {
    for (key, value) in [
        ("source_revision", json!("A".repeat(40))),
        ("producer_sha256", json!("0".repeat(63))),
        ("windows_version", json!("10.0.026100.1")),
        ("windows_version", json!("10.0.26100")),
        ("windows_version", json!("10.0.4294967296.1")),
        ("observation_point", json!("physical_truth")),
        ("evidence_kind", json!("verified")),
    ] {
        let mut input = base();
        input[key] = value;
        assert_eq!(
            audit_capture(&input.to_string()).unwrap_err(),
            CaptureError::InvalidValue
        );
    }
    assert!(audit_capture(BASE).unwrap().is_internally_complete());
}

#[test]
fn byte_limit_exact_plus_one_and_error_precedence_are_frozen() {
    assert_eq!(CAPTURE_MAX_BYTES, 1_048_576);
    let source = format!("{BASE}{}", " ".repeat(CAPTURE_MAX_BYTES - BASE.len()));
    assert!(audit_capture(&source).unwrap().is_internally_complete());
    assert_eq!(
        audit_capture(&(source + " ")).unwrap_err(),
        CaptureError::TooLarge
    );
    assert_eq!(
        audit_capture(&"!".repeat(CAPTURE_MAX_BYTES + 1)).unwrap_err(),
        CaptureError::TooLarge
    );
    let input = format!("{}0{}", "[".repeat(200), "]".repeat(200));
    assert_eq!(
        audit_capture(&input).unwrap_err(),
        CaptureError::InvalidCapture
    );
}

#[test]
fn record_limit_exact_plus_one_remains_bounded_even_with_duplicate_ids() {
    assert_eq!(CAPTURE_MAX_RECORDS, 4096);
    let mut input = base();
    let record = input["records"][0].clone();
    set_records(&mut input, vec![record.clone(); 4096]);
    input["end_inflight"] = json!("1");
    let source = input.to_string();
    assert!(source.len() < CAPTURE_MAX_BYTES);
    counts(&input, [4096, 4096, 0, 0, 1, 0]);
    reasons(
        &input,
        &["BOUNDARY_INFLIGHT", "DUPLICATE_REQUEST", "UNPAIRED_REQUEST"],
    );
    set_records(&mut input, vec![record; 4097]);
    assert_eq!(
        audit_capture(&input.to_string()).unwrap_err(),
        CaptureError::TooManyRecords
    );
    input["schema_version"] = json!(2);
    assert_eq!(
        audit_capture(&input.to_string()).unwrap_err(),
        CaptureError::UnsupportedVersion
    );
}
