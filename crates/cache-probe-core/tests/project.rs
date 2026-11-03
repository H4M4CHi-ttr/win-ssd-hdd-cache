mod common;
use cache_probe_core::project_v2;
use cache_types::capture::audit_capture;
use common::*;
use serde_json::{Value, json};

fn audit(bytes: &[u8]) -> Value {
    serde_json::from_str(
        &audit_capture(&project_v2(bytes).unwrap())
            .unwrap()
            .to_json(),
    )
    .unwrap()
}

#[test]
fn read_pair_projects_literal_unverified_boundaries() {
    let b = read_pair();
    assert_eq!(
        project_v2(&b).unwrap(),
        include_str!("../../../tests/fixtures/capture-probe-v1/project-read-pair.json")
    );
    assert_eq!(
        audit(&b)["reasons"],
        json!(["UNKNOWN_COUNTERS", "UNKNOWN_RANGE", "MISSING_PROVENANCE"])
    );
    assert_eq!(audit(&b)["paired_requests"], "1");
}

#[test]
fn flush_pair_projects_null_information() {
    let b = flush_pair();
    assert_eq!(
        project_v2(&b).unwrap(),
        include_str!("../../../tests/fixtures/capture-probe-v1/project-flush-pair.json")
    );
    assert_eq!(
        audit(&b)["reasons"],
        json!([
            "UNKNOWN_COUNTERS",
            "MISSING_PROVENANCE",
            "UNKNOWN_INFORMATION"
        ])
    );
    assert_eq!(audit(&b)["paired_requests"], "1");
    assert_eq!(audit(&b)["pending_requests"], "0");
}

#[test]
fn missing_terminal_never_infers_counter_or_closure() {
    let value: Value = serde_json::from_str(&project_v2(&read_pair()[..FOOTER]).unwrap()).unwrap();
    assert_eq!(value["capture_finalized"], false);
    for field in [
        "start_inflight",
        "end_inflight",
        "events_attempted",
        "events_dropped",
        "windows_version",
        "source_revision",
        "producer_sha256",
    ] {
        assert_eq!(value[field], Value::Null);
    }
}

#[test]
fn project_keeps_duplicate_orphan_status_and_original_order() {
    let mut b = flush_pair();
    set64(&mut b, COMPLETE + 8, 9);
    set32(&mut b, COMPLETE + 64, 0xc000_0001);
    let value: Value = serde_json::from_str(&project_v2(&b).unwrap()).unwrap();
    assert_eq!(value["records"][1]["request_id"], "9");
    assert_eq!(value["records"][1]["status"], "3221225473");
    assert_eq!(value["records"][1]["information"], Value::Null);
    assert_eq!(value["capture_finalized"], false);
    assert_eq!(audit(&b)["orphan_completions"], "1");
    let mut b = read_pair();
    set64(&mut b, COMPLETE, 0);
    let value: Value = serde_json::from_str(&project_v2(&b).unwrap()).unwrap();
    assert_eq!(value["records"][0]["seq"], "0");
    assert_eq!(value["records"][1]["seq"], "0");
    assert_eq!(value["capture_finalized"], false);
}

#[test]
fn project_never_upgrades_unknown_minor_or_address_mapping() {
    let mut b = read_pair();
    set32(&mut b, SUBMIT + 28, 1);
    set32(&mut b, SUBMIT + 40, 0);
    set64(&mut b, SUBMIT + 48, 0);
    set64(&mut b, SUBMIT + 56, 0);
    set32(&mut b, COMPLETE + 68, 0);
    set64(&mut b, COMPLETE + 72, 0);
    let value: Value = serde_json::from_str(&project_v2(&b).unwrap()).unwrap();
    assert_eq!(value["records"][0]["operation"], "other");
    assert_eq!(value["records"][0]["offset"], Value::Null);
    assert_eq!(value["records"][0]["length"], Value::Null);
    assert_eq!(value["windows_version"], Value::Null);
}

#[test]
fn project_derived_fault_does_not_trust_footer() {
    let mut b = read_pair();
    set64(&mut b, STOP + 48, 4);
    let value: Value = serde_json::from_str(&project_v2(&b).unwrap()).unwrap();
    assert_eq!(value["capture_finalized"], false);
    assert_eq!(value["events_attempted"], "2");
    assert_eq!(
        audit(&b)["reasons"],
        json!([
            "NOT_FINALIZED",
            "UNKNOWN_COUNTERS",
            "UNKNOWN_RANGE",
            "MISSING_PROVENANCE"
        ])
    );
}

#[test]
fn lying_footer_count_invalidates_terminal_projection() {
    let mut b = read_pair();
    set32(&mut b, FOOTER + 20, 1);
    let value: Value = serde_json::from_str(&project_v2(&b).unwrap()).unwrap();
    assert_eq!(value["events_attempted"], Value::Null);
    assert_eq!(value["events_dropped"], Value::Null);
    assert_eq!(value["capture_finalized"], false);
}
