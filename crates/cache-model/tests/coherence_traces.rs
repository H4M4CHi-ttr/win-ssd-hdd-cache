//! F07 literal regressions. Expected values come from trace v1, not replay output.
use cache_model::replay::{ReplayResult, run_trace, run_trace_with_fault};
use cache_types::{
    evidence::{
        Category, Fault, Outcome, Terminal, ViolationCode, ViolationKind, encode_evidence,
        parse_evidence,
    },
    trace::{Decision, IoStatus, parse_trace},
};

const COLD: &str = include_str!("../../../tests/fixtures/coherence/t01-cold-fill-hit.json");
const PARTIAL: &str = include_str!("../../../tests/fixtures/coherence/t02-partial-write.json");
const PARTIAL_ERROR: &str =
    include_str!("../../../tests/fixtures/coherence/t02-partial-error.json");
const CROSS_LINE: &str =
    include_str!("../../../tests/fixtures/coherence/t03-cross-line-write.json");
const STALE: &str = include_str!("../../../tests/fixtures/coherence/t04-stale-fill.json");
const ABA: &str = include_str!("../../../tests/fixtures/coherence/t04-aba-fill.json");
const WRITERS: &str =
    include_str!("../../../tests/fixtures/coherence/t05-overlapping-writers.json");
const DISCARD: &str =
    include_str!("../../../tests/fixtures/coherence/t11-discard-partial-rewrite.json");

fn reference(
    input: &str,
    seed: u64,
    events: u64,
    assertions: u64,
    cache_checks: u64,
) -> ReplayResult {
    let trace = parse_trace(input).expect("the literal fixture must be a valid trace");
    assert_eq!(trace.seed(), seed);
    assert_eq!(trace.events().len() as u64, events);
    let result = run_trace(&trace);
    assert_eq!(
        result,
        run_trace(&trace),
        "same event order must be deterministic"
    );
    let report = result.evidence();
    assert_eq!(report.outcome, Outcome::Passed, "{report:?}");
    assert_eq!(report.fault, Fault::None);
    assert_eq!(report.terminal, Terminal::Drained);
    assert_eq!(report.violation, None);
    assert_eq!(report.events_total, events);
    assert_eq!(report.events_executed, events);
    assert_eq!(report.assertions_checked(), assertions);
    assert_eq!(report.pending_requests, 0);
    assert_eq!(report.pending_ios, 0);
    assert_eq!(report.abandoned_requests, 0);
    assert_eq!(report.abandoned_ios, 0);
    for category in [Category::OriginResults, Category::Ownership] {
        assert!(
            report.checks[category.index()].passed > 0,
            "{category:?} was not observed"
        );
    }
    // Oracle v1 §9.1 counts only accepted publication/read/commit claims.
    // Correct T04 traces reject all such claims, so their exact count is zero.
    assert_eq!(
        report.checks[Category::CacheLineage.index()].passed,
        cache_checks
    );
    for check in report.checks {
        assert_eq!(check.failed, 0);
    }
    // These fixtures make no power-loss, durability, shadow or product claim.
    assert_eq!(report.checks[Category::Durability.index()].passed, 0);
    assert_eq!(report.checks[Category::Shadow.index()].passed, 0);
    assert_eq!(report.source_revision, None);
    assert_eq!(report.trace_digest, None);
    let encoded = encode_evidence(report).unwrap();
    assert_eq!(parse_evidence(&encoded).unwrap(), *report);
    assert!(encoded.ends_with('\n'));
    assert!(!encoded.contains('\r'));
    assert!(encoded.contains("\"kernel_runtime_tested\":false"));
    assert!(encoded.contains("\"production_qualified\":false"));
    result
}

fn read_result(result: &ReplayResult, request: u64, data: &[u8], known: &[bool]) {
    let actual = result
        .caller_result(request)
        .expect("explicit committed read");
    assert_eq!(actual.status(), IoStatus::Ok);
    assert_eq!(actual.reported(), data.len() as u64);
    assert_eq!(actual.data(), data);
    assert_eq!(actual.known(), known);
}

#[test]
fn t01_cold_read_falls_back_then_a_complete_fill_serves_a_known_hit() {
    let r = reference(COLD, 101, 28, 5, 3);
    assert_eq!(r.decision(1), Some(Decision::Rejected));
    assert_eq!(r.decision(16), Some(Decision::Accepted));
    assert_eq!(r.decision(19), Some(Decision::Accepted));
    read_result(&r, 1, b"AA", &[true, true]);
    read_result(&r, 3, b"AA", &[true, true]);
}

#[test]
fn t02_partial_write_invalidates_the_entire_requested_line_before_origin_fallback() {
    let r = reference(PARTIAL, 102, 28, 5, 1);
    assert_eq!(r.decision(17), Some(Decision::Rejected));
    read_result(&r, 3, b"ABA", &[true, true, true]);
    let write = r.caller_result(2).unwrap();
    assert_eq!(write.status(), IoStatus::Ok);
    assert_eq!(write.reported(), 1);
}

#[test]
fn t02_failed_partial_write_keeps_actual_effects_and_the_original_error_count() {
    let r = reference(PARTIAL_ERROR, 202, 28, 5, 1);
    assert_eq!(r.decision(17), Some(Decision::Rejected));
    read_result(&r, 3, b"BA", &[true, true]);
    let write = r.caller_result(2).unwrap();
    assert_eq!(write.status(), IoStatus::IoError);
    assert_eq!(write.reported(), 0);
}

#[test]
fn t03_two_bytes_at_4095_invalidate_both_lines_and_cross_line_hit_is_refused() {
    let r = reference(CROSS_LINE, 103, 40, 7, 2);
    assert_eq!(r.decision(28), Some(Decision::Rejected));
    read_result(&r, 4, b"BC", &[true, true]);
    assert_eq!(r.origin().len(), 4097);
    assert!(
        r.origin()[..4095]
            .iter()
            .all(|c| c.known() && c.value() == b'A')
    );
    assert_eq!(r.origin()[4095].value(), b'B');
    assert_eq!(r.origin()[4096].value(), b'C');
}

#[test]
fn t04_late_a_fill_after_b_mutation_cannot_publish_or_supply_a_hit() {
    let r = reference(STALE, 104, 28, 5, 0);
    assert_eq!(r.decision(13), Some(Decision::Rejected));
    assert_eq!(r.decision(17), Some(Decision::Rejected));
    read_result(&r, 3, b"B", &[true]);
}

#[test]
fn t04_byte_equal_aba_does_not_restore_the_old_fill_lineage() {
    let r = reference(ABA, 204, 34, 5, 0);
    assert_eq!(r.decision(18), Some(Decision::Rejected));
    assert_eq!(r.decision(22), Some(Decision::Rejected));
    read_result(&r, 4, b"A", &[true]);
}

#[test]
fn t05_one_writer_completion_does_not_admit_fill_or_hit_while_another_remains() {
    let r = reference(WRITERS, 105, 49, 9, 4);
    assert_eq!(r.decision(16), Some(Decision::Rejected));
    assert_eq!(r.decision(19), Some(Decision::Rejected));
    assert_eq!(r.decision(25), Some(Decision::Accepted));
    assert_eq!(r.decision(33), Some(Decision::Accepted));
    assert_eq!(r.decision(35), Some(Decision::Accepted));
    read_result(&r, 5, b"C", &[true]);
}

#[test]
fn t11_discard_unknownness_survives_partial_rewrite_and_refuses_cache_population() {
    let r = reference(DISCARD, 111, 43, 8, 1);
    assert_eq!(r.decision(26), Some(Decision::Rejected));
    assert_eq!(r.decision(30), Some(Decision::Rejected));
    read_result(&r, 5, &[b'B', 0], &[true, false]);
    assert!(r.origin()[0].known());
    assert!(!r.origin()[1].known()); // zero is an unknown placeholder, not zero-fill evidence.
}

fn stale_mutant(input: &str, publish_index: u64) {
    let trace = parse_trace(input).unwrap();
    assert_eq!(run_trace(&trace).evidence().outcome, Outcome::Passed);
    let failed = run_trace_with_fault(&trace, Fault::StaleFill);
    let report = failed.evidence();
    assert_eq!(report.outcome, Outcome::Failed);
    assert_eq!(report.fault, Fault::StaleFill);
    assert_eq!(report.terminal, Terminal::Stopped);
    let violation = report
        .violation
        .as_ref()
        .expect("a specific oracle safety failure");
    assert_eq!(violation.code, ViolationCode::StalePublication);
    assert_eq!(violation.code.kind(), ViolationKind::SafetyViolation);
    assert_eq!(violation.event_index, publish_index);
    assert_eq!(report.events_executed, publish_index + 1);
    assert_eq!(report.checks[Category::CacheLineage.index()].failed, 1);
    assert_eq!(report.assertions_checked(), 0); // detection precedes fixture assertions.
    assert_eq!(
        parse_evidence(&encode_evidence(report).unwrap()).unwrap(),
        *report
    );
}

#[test]
fn t04_same_stale_trace_detects_stale_fill_with_independent_safety_code() {
    stale_mutant(STALE, 13);
}

#[test]
fn t04_same_aba_trace_detects_stale_lineage_even_when_bytes_are_equal() {
    stale_mutant(ABA, 18);
}
