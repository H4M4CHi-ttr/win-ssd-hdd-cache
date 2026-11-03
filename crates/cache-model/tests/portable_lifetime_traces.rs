//! F11: trace-v1 lifetime semantics only; no Windows, timer or DMA simulation.
use cache_model::replay::{ReplayResult, run_trace, run_trace_with_fault};
use cache_types::{
    evidence::{
        Category, Fault, Outcome, Terminal, ViolationCode, ViolationKind, encode_evidence,
        parse_evidence,
    },
    trace::{Decision, IoStatus, parse_trace},
};

const CANCEL: &str =
    include_str!("../../../tests/fixtures/portable-lifetime/cancel-held-lease.json");
const EPOCH: &str = include_str!("../../../tests/fixtures/portable-lifetime/epoch-held-lease.json");
const LATE: &str =
    include_str!("../../../tests/fixtures/portable-lifetime/fallback-late-cache.json");
const CACHE_ERROR: &str =
    include_str!("../../../tests/fixtures/portable-lifetime/failed-cache-overwrite.json");
const ORIGIN_ERROR: &str =
    include_str!("../../../tests/fixtures/portable-lifetime/failed-origin-result.json");

fn reference(
    input: &str,
    seed: u64,
    events: u64,
    assertions: u64,
    cache_checks: u64,
    retained_cells: usize,
) -> ReplayResult {
    let trace = parse_trace(input).expect("fixed trace-v1 fixture");
    assert_eq!(trace.seed(), seed);
    assert_eq!(trace.events().len() as u64, events);
    let result = run_trace(&trace);
    assert_eq!(result, run_trace(&trace));
    let report = result.evidence();
    assert_eq!(report.outcome, Outcome::Passed, "{report:?}");
    assert_eq!(report.terminal, Terminal::Drained);
    assert_eq!(report.fault, Fault::None);
    assert_eq!(report.events_executed, events);
    assert_eq!(report.events_total, events);
    assert_eq!(report.assertions_checked(), assertions);
    assert_eq!(report.violation, None);
    assert_eq!(report.pending_requests, 0);
    assert_eq!(report.pending_ios, 0);
    assert_eq!(report.abandoned_requests, 0);
    assert_eq!(report.abandoned_ios, 0);
    assert!(report.checks[Category::OriginResults.index()].passed > 0);
    assert!(report.checks[Category::Ownership.index()].passed > 0);
    assert_eq!(
        report.checks[Category::CacheLineage.index()].passed,
        cache_checks
    );
    assert_eq!(report.checks[Category::Durability.index()].passed, 0);
    assert_eq!(report.checks[Category::Shadow.index()].passed, 0);
    assert!(report.checks.iter().all(|check| check.failed == 0));
    assert_eq!(report.source_revision, None);
    assert_eq!(report.trace_digest, None);
    let encoded = encode_evidence(report).unwrap();
    assert_eq!(parse_evidence(&encoded).unwrap(), *report);
    assert!(encoded.ends_with('\n'));
    assert!(!encoded.contains('\r'));
    assert!(encoded.contains("\"kernel_runtime_tested\":false"));
    assert!(encoded.contains("\"production_qualified\":false"));
    // All parents retire, but immutable caller data/mutation history remains.
    // This is logical cell accounting, not real allocator or DMA quiescence.
    assert_eq!(result.resource_usage().owned_cells, retained_cells);
    assert_eq!(result.resource_usage().oracle_owned_cells, retained_cells);
    result
}

fn read_result(r: &ReplayResult, request: u64, status: IoStatus, data: &[u8]) {
    let actual = r.caller_result(request).expect("committed origin result");
    assert_eq!(actual.status(), status);
    assert_eq!(actual.reported(), data.len() as u64);
    assert_eq!(actual.data(), data);
    assert_eq!(actual.known(), vec![true; data.len()]);
}

#[test]
fn cancel_flags_hold_physical_write_lease_until_actual_cancelled_completion() {
    let r = reference(CANCEL, 301, 30, 8, 0, 1);
    assert_eq!(r.decision(8), Some(Decision::Pending));
    assert_eq!(r.decision(10), Some(Decision::Pending));
    assert_eq!(r.decision(14), Some(Decision::Accepted));
    assert_eq!(r.decision(16), Some(Decision::Rejected));
    assert_eq!(r.decision(19), Some(Decision::Accepted));
    assert_eq!(r.decision(22), Some(Decision::Rejected));
    read_result(&r, 2, IoStatus::Ok, b"A");
}

#[test]
fn epoch_invalidation_waits_for_old_child_before_cold_resume_and_reuse() {
    let r = reference(EPOCH, 302, 32, 9, 0, 1);
    for index in [7, 9, 11] {
        assert_eq!(r.decision(index), Some(Decision::Pending));
    }
    assert_eq!(r.decision(15), Some(Decision::Rejected));
    for index in [17, 19] {
        assert_eq!(r.decision(index), Some(Decision::Accepted));
    }
    assert_eq!(r.decision(24), Some(Decision::Rejected));
    read_result(&r, 2, IoStatus::Ok, b"A");
}

#[test]
fn committed_origin_b_is_not_overwritten_by_late_cache_a_or_double_completed() {
    let r = reference(LATE, 303, 37, 8, 2, 2);
    assert_eq!(r.decision(10), Some(Decision::Accepted));
    assert_eq!(r.decision(20), Some(Decision::Accepted));
    assert_eq!(r.decision(22), Some(Decision::Pending));
    assert_eq!(r.decision(24), Some(Decision::Pending));
    assert_eq!(r.decision(30), Some(Decision::Accepted));
    assert_eq!(r.decision(32), Some(Decision::Accepted));
    read_result(&r, 2, IoStatus::Ok, b"B");
    assert_eq!(r.origin()[0].value(), b'B');
    let write = r.caller_result(3).unwrap();
    assert_eq!(write.status(), IoStatus::Ok);
    assert_eq!(write.reported(), 1);
}

#[test]
fn failed_partial_same_slot_overwrite_never_restores_previous_visibility() {
    let r = reference(CACHE_ERROR, 304, 33, 7, 1, 2);
    assert_eq!(r.decision(14), Some(Decision::Accepted));
    assert_eq!(r.decision(18), Some(Decision::Rejected));
    assert_eq!(r.decision(22), Some(Decision::Rejected));
    read_result(&r, 3, IoStatus::Ok, b"AA");
}

#[test]
fn cache_error_fallback_preserves_origin_error_reported_prefix_and_known_bytes() {
    let r = reference(ORIGIN_ERROR, 305, 23, 3, 2, 1);
    assert_eq!(r.decision(10), Some(Decision::Accepted));
    assert_eq!(r.decision(13), Some(Decision::Rejected));
    assert_eq!(r.decision(18), Some(Decision::Accepted));
    read_result(&r, 2, IoStatus::IoError, b"A");
}

fn detects_live_reuse(input: &str, bind_index: u64) {
    let trace = parse_trace(input).unwrap();
    assert_eq!(run_trace(&trace).evidence().outcome, Outcome::Passed);
    let result = run_trace_with_fault(&trace, Fault::LateSlotReuse);
    let report = result.evidence();
    assert_eq!(report.outcome, Outcome::Failed);
    assert_eq!(report.terminal, Terminal::Stopped);
    assert_eq!(report.fault, Fault::LateSlotReuse);
    let violation = report
        .violation
        .as_ref()
        .expect("independent ownership failure");
    assert_eq!(violation.code, ViolationCode::LiveLeaseReused);
    assert_eq!(violation.code.kind(), ViolationKind::SafetyViolation);
    assert_eq!(violation.event_index, bind_index);
    assert_eq!(report.events_executed, bind_index + 1);
    assert_eq!(report.assertions_checked(), 0);
    assert_eq!(report.checks[Category::Ownership.index()].failed, 1);
    assert_eq!(report.pending_ios, 1); // failure did not fabricate actual completion.
    assert_eq!(
        parse_evidence(&encode_evidence(report).unwrap()).unwrap(),
        *report
    );
}

#[test]
fn same_cancel_fixture_detects_live_lease_reuse_before_any_assertion() {
    detects_live_reuse(CANCEL, 8);
}

#[test]
fn same_epoch_fixture_detects_live_lease_reuse_before_any_assertion() {
    detects_live_reuse(EPOCH, 7);
}
