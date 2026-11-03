use cache_model::replay::{run_trace, run_trace_with_fault};
use cache_types::{
    evidence::{
        Category, Fault, Outcome, Terminal, ViolationCode, encode_evidence, parse_evidence,
    },
    trace::{Decision, parse_trace},
};

fn trace(initial: &str, events: &[String]) -> cache_types::trace::TraceV1 {
    trace_options(initial, events, u64::MAX)
}

fn trace_options(initial: &str, events: &[String], limit: u64) -> cache_types::trace::TraceV1 {
    parse_trace(&format!(
        r#"{{"schema_version":1,"seed":"42","capacity":"{}","line_size":4096,"slot_count":1,"counter_limit":"{limit}","initial_hex":"{initial}","events":[{}]}}"#,
        initial.len() / 2, events.join(",")
    )).unwrap()
}

fn read(request: u64, offset: u64, length: u64) -> String {
    format!(
        r#"{{"event":"read_submit","request":"{request}","offset":"{offset}","length":"{length}"}}"#
    )
}
fn io(io: u64, request: u64, target: &str) -> String {
    format!(
        r#"{{"event":"io_submit","io":"{io}","request":"{request}","target":"{target}","slot":null,"source":null}}"#
    )
}
fn transfer(io: u64, offset: u64, length: u64) -> String {
    format!(r#"{{"event":"io_transfer","io":"{io}","offset":"{offset}","length":"{length}"}}"#)
}
fn complete(io: u64, status: &str, reported: u64) -> String {
    format!(r#"{{"event":"io_complete","io":"{io}","status":"{status}","reported":"{reported}"}}"#)
}
fn commit(request: u64, io: u64) -> String {
    format!(r#"{{"event":"read_commit","request":"{request}","io":"{io}"}}"#)
}
fn assert_read(request: u64, data: &str, known: &str) -> String {
    format!(r#"{{"event":"assert_read","request":"{request}","data":"{data}","known":"{known}"}}"#)
}
fn baseline() -> Vec<String> {
    vec![
        read(1, 0, 2),
        io(1, 1, "origin_read"),
        transfer(1, 0, 2),
        complete(1, "ok", 2),
        commit(1, 1),
        assert_read(1, "4141", "11"),
    ]
}

#[test]
fn literal_origin_read_has_independent_observations_and_reproducible_report() {
    let input = trace("4141", &baseline());
    let result = run_trace(&input);
    assert_eq!(result, run_trace(&input));
    let report = result.evidence();
    assert_eq!(report.outcome, Outcome::Passed);
    assert_eq!(report.terminal, Terminal::Drained);
    assert_eq!(report.events_executed, 6);
    assert_eq!(report.meaningful_observations(), 4);
    assert_eq!(report.assertions_checked(), 1);
    assert_eq!(report.checks[Category::OriginResults.index()].passed, 2);
    assert_eq!(report.checks[Category::Ownership.index()].passed, 2);
    assert_eq!(result.decision(4), Some(Decision::Accepted));
    assert_eq!(result.decision(5), None);
    assert_eq!(result.caller_result(1).unwrap().data(), vec![65, 65]);
    assert_eq!(
        parse_evidence(&encode_evidence(report).unwrap()).unwrap(),
        *report
    );
}

#[test]
fn empty_and_assert_only_are_inconclusive_not_zero_test_success() {
    for events in [
        vec![],
        vec![r#"{"event":"assert_origin","offset":"0","data":"4141","known":"11"}"#.to_owned()],
    ] {
        let result = run_trace(&trace("4141", &events));
        assert_eq!(result.evidence().outcome, Outcome::Inconclusive);
        assert_eq!(result.evidence().meaningful_observations(), 0);
        assert_eq!(result.evidence().terminal, Terminal::Drained);
    }
}

#[test]
fn actual_completion_and_parent_commit_have_separate_pending_counts() {
    let events = baseline();
    let before_complete = run_trace(&trace("4141", &events[..3]));
    assert_eq!(before_complete.evidence().pending_requests, 1);
    assert_eq!(before_complete.evidence().pending_ios, 1);
    let completed = run_trace(&trace("4141", &events[..4]));
    assert_eq!(completed.evidence().pending_requests, 1);
    assert_eq!(completed.evidence().pending_ios, 0);
    assert_eq!(completed.evidence().terminal, Terminal::Pending);
    assert_eq!(completed.evidence().outcome, Outcome::Inconclusive);
    let committed_not_retired = run_trace(&trace("4141", &events[..5]));
    assert_eq!(committed_not_retired.evidence().terminal, Terminal::Drained);
}

#[test]
fn first_semantic_failure_stops_before_trailing_assertions() {
    let mut events = baseline();
    events.insert(4, complete(1, "ok", 2));
    let result = run_trace(&trace("4141", &events));
    let report = result.evidence();
    assert_eq!(report.outcome, Outcome::Failed);
    assert_eq!(report.terminal, Terminal::Stopped);
    assert_eq!(report.events_executed, 5);
    assert_eq!(report.violation.as_ref().unwrap().event_index, 4);
    assert_eq!(
        report.violation.as_ref().unwrap().code,
        ViolationCode::DuplicateCompletion
    );
    assert_eq!(report.assertions_checked(), 0);
    assert_eq!(report.checks.iter().map(|c| c.failed).sum::<u64>(), 0);
    assert_eq!(result.decision(4), None);
}

#[test]
fn incomplete_commit_is_pending_and_origin_error_is_preserved() {
    let mut events = baseline();
    events.insert(3, commit(1, 1));
    events[4] = complete(1, "io_error", 1);
    events[6] = assert_read(1, "41", "1");
    let result = run_trace(&trace("4141", &events));
    assert_eq!(result.decision(3), Some(Decision::Pending));
    assert_eq!(result.caller_result(1).unwrap().reported(), 1);
    assert_eq!(
        result.caller_result(1).unwrap().status(),
        cache_types::trace::IoStatus::IoError
    );
    assert_eq!(result.evidence().outcome, Outcome::Passed);
}

#[test]
fn coverage_and_overlap_are_backend_and_semantic_failures_not_decisions() {
    let events = vec![
        read(1, 0, 2),
        io(1, 1, "origin_read"),
        transfer(1, 0, 1),
        complete(1, "ok", 2),
    ];
    let r = run_trace(&trace("4141", &events));
    assert_eq!(
        r.evidence().violation.as_ref().unwrap().code,
        ViolationCode::ReadReportedUnsampled
    );
    assert_eq!(
        r.evidence().checks[Category::OriginResults.index()].failed,
        1
    );
    let events = vec![
        read(1, 0, 2),
        io(1, 1, "origin_read"),
        transfer(1, 0, 1),
        transfer(1, 0, 1),
    ];
    let r = run_trace(&trace("4141", &events));
    assert_eq!(
        r.evidence().violation.as_ref().unwrap().code,
        ViolationCode::TransferOverlap
    );
}

fn write(request: u64, offset: u64, data: &str, fua: bool) -> String {
    format!(
        r#"{{"event":"write_submit","request":"{request}","offset":"{offset}","data":"{data}","fua":{fua}}}"#
    )
}
fn discard(request: u64, offset: u64, length: u64) -> String {
    format!(
        r#"{{"event":"discard_submit","request":"{request}","offset":"{offset}","length":"{length}"}}"#
    )
}
fn mutation_commit(request: u64, io: u64) -> String {
    format!(r#"{{"event":"mutation_commit","request":"{request}","io":"{io}"}}"#)
}
fn persist(io: u64, offset: u64, length: u64) -> String {
    format!(r#"{{"event":"origin_persist","io":"{io}","offset":"{offset}","length":"{length}"}}"#)
}
fn control(event: &str, request: u64) -> String {
    format!(r#"{{"event":"{event}","request":"{request}"}}"#)
}
fn origin(data: &str, known: &str) -> String {
    format!(r#"{{"event":"assert_origin","offset":"0","data":"{data}","known":"{known}"}}"#)
}
fn power_loss() -> String {
    r#"{"event":"power_loss"}"#.to_owned()
}

#[test]
fn ordinary_write_success_is_not_durable_and_failed_partial_effect_is_real() {
    let events = vec![
        write(1, 0, "4242", false),
        io(1, 1, "origin_write"),
        transfer(1, 0, 2),
        complete(1, "ok", 2),
        mutation_commit(1, 1),
        origin("4242", "11"),
        power_loss(),
        origin("4141", "11"),
    ];
    let r = run_trace(&trace("4141", &events));
    assert_eq!(r.evidence().outcome, Outcome::Passed, "{:?}", r.evidence());
    assert_eq!(r.evidence().terminal, Terminal::PowerLoss);
    assert_eq!(r.evidence().abandoned_requests, 0);
    let events = vec![
        write(1, 0, "4242", false),
        io(1, 1, "origin_write"),
        transfer(1, 0, 1),
        complete(1, "io_error", 0),
        mutation_commit(1, 1),
        origin("4241", "11"),
        control("retire_request", 1),
        persist(1, 0, 1),
        power_loss(),
        origin("4241", "11"),
    ];
    let r = run_trace(&trace("4141", &events));
    assert_eq!(r.evidence().outcome, Outcome::Passed, "{:?}", r.evidence());
    assert_eq!(
        r.caller_result(1).unwrap().status(),
        cache_types::trace::IoStatus::IoError
    );
    assert_eq!(r.caller_result(1).unwrap().reported(), 0);
}

#[test]
fn old_persist_cannot_regress_newer_durable_version() {
    let events = vec![
        write(1, 0, "42", false),
        io(1, 1, "origin_write"),
        transfer(1, 0, 1),
        write(2, 0, "43", false),
        io(2, 2, "origin_write"),
        transfer(2, 0, 1),
        persist(2, 0, 1),
        persist(1, 0, 1),
        power_loss(),
        origin("43", "1"),
    ];
    let r = run_trace(&trace("41", &events));
    assert_eq!(r.evidence().outcome, Outcome::Passed, "{:?}", r.evidence());
    assert_eq!(r.evidence().abandoned_requests, 2);
    assert_eq!(r.evidence().abandoned_ios, 2);
}

#[test]
fn discard_unknownness_and_fragmented_origin_read_are_not_latest_snapshot_comparison() {
    let events = vec![
        discard(1, 0, 2),
        io(1, 1, "origin_discard"),
        transfer(1, 0, 2),
        complete(1, "ok", 2),
        write(2, 0, "42", false),
        io(2, 2, "origin_write"),
        transfer(2, 0, 1),
        origin("4200", "10"),
        read(3, 0, 2),
        io(3, 3, "origin_read"),
        transfer(3, 0, 2),
        complete(3, "ok", 2),
        commit(3, 3),
        assert_read(3, "4200", "10"),
    ];
    let r = run_trace(&trace("4141", &events));
    assert_eq!(r.evidence().violation, None);
    let events = vec![
        read(1, 0, 2),
        io(1, 1, "origin_read"),
        transfer(1, 0, 1),
        write(2, 0, "4242", false),
        io(2, 2, "origin_write"),
        transfer(2, 0, 2),
        complete(2, "ok", 2),
        transfer(1, 1, 1),
        complete(1, "ok", 2),
        commit(1, 1),
        assert_read(1, "4142", "11"),
    ];
    let r = run_trace(&trace("4141", &events));
    assert_eq!(r.evidence().violation, None);
}

#[test]
fn fua_and_flush_backend_promises_are_separate_from_candidate_safety() {
    let events = vec![
        write(1, 0, "42", true),
        io(1, 1, "origin_write"),
        transfer(1, 0, 1),
        complete(1, "ok", 1),
    ];
    let r = run_trace(&trace("41", &events));
    assert_eq!(
        r.evidence().violation.as_ref().unwrap().code,
        ViolationCode::FuaNotDurable
    );
    assert_eq!(
        r.evidence().checks[Category::OriginResults.index()].passed,
        1
    );
    assert_eq!(r.evidence().checks[Category::Durability.index()].failed, 1);
    let mut durable = events.clone();
    durable.insert(3, persist(1, 0, 1));
    assert_eq!(run_trace(&trace("41", &durable)).evidence().violation, None);
    let events = vec![
        write(1, 0, "42", false),
        control("flush_submit", 2),
        io(2, 2, "origin_flush"),
        io(1, 1, "origin_write"),
        transfer(1, 0, 1),
        complete(1, "io_error", 0),
        io(3, 2, "origin_flush"),
        complete(3, "ok", 0),
    ];
    let r = run_trace(&trace("41", &events));
    assert_eq!(r.decision(2), Some(Decision::Pending));
    assert_eq!(r.decision(6), Some(Decision::Accepted));
    assert_eq!(
        r.evidence().violation.as_ref().unwrap().code,
        ViolationCode::FlushNotDurable
    );
}

#[test]
fn newer_durable_write_covers_flush_frontier_but_later_incomplete_write_is_not_in_frontier() {
    let events = vec![
        write(1, 0, "42", false),
        io(1, 1, "origin_write"),
        transfer(1, 0, 1),
        complete(1, "ok", 1),
        control("flush_submit", 2),
        write(3, 0, "43", false),
        io(3, 3, "origin_write"),
        transfer(3, 0, 1),
        persist(3, 0, 1),
        io(2, 2, "origin_flush"),
        complete(2, "ok", 0),
        power_loss(),
        origin("43", "1"),
    ];
    let r = run_trace(&trace("41", &events));
    assert_eq!(r.decision(9), Some(Decision::Accepted));
    assert_eq!(r.evidence().outcome, Outcome::Passed, "{:?}", r.evidence());
}

#[test]
fn power_loss_is_terminal_and_reports_abandoned_not_fabricated_completion() {
    let events = vec![
        read(1, 0, 1),
        io(1, 1, "origin_read"),
        power_loss(),
        origin("41", "1"),
    ];
    let r = run_trace(&trace("41", &events));
    assert_eq!(r.evidence().terminal, Terminal::PowerLoss);
    assert_eq!(r.evidence().pending_requests, 0);
    assert_eq!(r.evidence().pending_ios, 0);
    assert_eq!(r.evidence().abandoned_requests, 1);
    assert_eq!(r.evidence().abandoned_ios, 1);
    assert!(r.caller_result(1).is_none());
    let mut events = events;
    events.push(complete(1, "ok", 0));
    let r = run_trace(&trace("41", &events));
    assert_eq!(
        r.evidence().violation.as_ref().unwrap().code,
        ViolationCode::PostPowerLoss
    );
    assert_eq!(r.evidence().abandoned_ios, 1);
    assert!(encode_evidence(r.evidence()).is_ok());
}

#[test]
fn cancel_flags_do_not_complete_or_release_and_retirement_preserves_results() {
    let mut events = baseline();
    events.insert(3, control("cancel_request", 1));
    events.insert(4, control("retire_request", 1));
    events.push(control("cancel_request", 1));
    events.push(control("retire_request", 1));
    events.push(assert_read(1, "4141", "11"));
    let r = run_trace(&trace("4141", &events));
    assert_eq!(r.decision(4), Some(Decision::Pending));
    assert_eq!(r.evidence().outcome, Outcome::Passed, "{:?}", r.evidence());
    events.push(control("cancel_request", 1));
    let r = run_trace(&trace("4141", &events));
    assert_eq!(
        r.evidence().violation.as_ref().unwrap().code,
        ViolationCode::InvalidPhase
    );
}

#[test]
fn zero_length_transfers_are_noops_and_unapplied_persist_is_semantic_error() {
    let events = vec![
        write(1, 1, "", true),
        io(1, 1, "origin_write"),
        transfer(1, 0, 0),
        transfer(1, 0, 0),
        persist(1, 0, 0),
        complete(1, "ok", 0),
        mutation_commit(1, 1),
        power_loss(),
        origin("41", "1"),
    ];
    let r = run_trace(&trace("41", &events));
    assert_eq!(r.evidence().outcome, Outcome::Passed, "{:?}", r.evidence());
    let events = vec![
        write(1, 0, "42", false),
        io(1, 1, "origin_write"),
        persist(1, 0, 1),
    ];
    assert_eq!(
        run_trace(&trace("41", &events))
            .evidence()
            .violation
            .as_ref()
            .unwrap()
            .code,
        ViolationCode::InvalidPhase
    );
}

#[test]
fn live_parent_limit_is_exact_and_rejected_parent_can_retire_without_commit() {
    let mut events: Vec<_> = (1..=1025).map(|r| read(r, 0, 0)).collect();
    events.push(control("retire_request", 1025));
    events.push(io(1, 1, "origin_read"));
    events.push(complete(1, "ok", 0));
    events.push(commit(1, 1));
    events.push(control("retire_request", 1));
    events.push(read(1026, 0, 0));
    let r = run_trace(&trace("", &events));
    assert_eq!(r.decision(1023), Some(Decision::Accepted));
    assert_eq!(r.decision(1024), Some(Decision::Rejected));
    assert_eq!(r.decision(1025), Some(Decision::Accepted));
    assert_eq!(r.decision(1030), Some(Decision::Accepted));
    assert_eq!(r.evidence().violation, None);
}

#[test]
fn result_assertion_requires_committed_parent_as_a_lifecycle_precondition() {
    let r = run_trace(&trace("41", &[read(1, 0, 1), assert_read(1, "41", "1")]));
    assert_eq!(
        r.evidence().violation.as_ref().unwrap().code,
        ViolationCode::InvalidPhase
    );
    assert_eq!(r.evidence().assertions_checked(), 0);
}

#[test]
fn retained_results_and_future_result_reservations_enforce_exact_cell_budget() {
    const MIB: u64 = 1_048_576;
    let initial = "41".repeat(MIB as usize);
    let mut events = Vec::new();
    for id in 1..=15 {
        events.extend([
            read(id, 0, MIB),
            io(id, id, "origin_read"),
            transfer(id, 0, MIB),
            complete(id, "ok", MIB),
            commit(id, id),
            control("retire_request", id),
        ]);
    }
    let at = events.len();
    events.extend([read(16, 0, MIB), io(16, 16, "origin_read"), read(17, 0, 1)]);
    let r = run_trace(&trace(&initial, &events));
    assert_eq!(r.evidence().violation, None);
    assert_eq!(r.decision(at), Some(Decision::Accepted));
    assert_eq!(r.decision(at + 1), Some(Decision::Pending));
    assert_eq!(r.decision(at + 2), Some(Decision::Rejected));
    assert_eq!(r.caller_result(1).unwrap().reported(), MIB);
    let usage = r.resource_usage();
    assert_eq!(usage.owned_cells, 16 * MIB as usize);
    assert_eq!(usage.peak_owned_cells, 16 * MIB as usize);
    assert_eq!(usage.oracle_owned_cells, usage.owned_cells);
    assert_eq!(usage.peak_oracle_owned_cells, usage.peak_owned_cells);
}

#[test]
fn retired_mutation_history_retains_owned_budget_and_persistability() {
    const MIB: u64 = 1_048_576;
    let initial = "41".repeat(MIB as usize);
    let mut events = Vec::new();
    for id in 1..=16 {
        events.extend([
            discard(id, 0, MIB),
            io(id, id, "origin_discard"),
            transfer(id, 0, MIB),
            complete(id, "io_error", 0),
            mutation_commit(id, id),
            control("retire_request", id),
        ]);
    }
    let at = events.len();
    events.extend([discard(17, 0, 1), persist(1, 0, 1), power_loss()]);
    let r = run_trace(&trace(&initial, &events));
    assert_eq!(r.evidence().violation, None);
    assert_eq!(r.decision(at), Some(Decision::Rejected));
    assert!(!r.origin()[0].known());
    assert!(r.origin()[1].known());
    let usage = r.resource_usage();
    assert_eq!(usage.owned_cells, 16 * MIB as usize);
    assert_eq!(usage.oracle_owned_cells, usage.owned_cells);
}

fn bind(line: u64) -> String {
    format!(r#"{{"event":"slot_bind","slot":"0","line":"{line}"}}"#)
}
fn fill(request: u64, line: u64) -> String {
    format!(r#"{{"event":"fill_begin","request":"{request}","line":"{line}"}}"#)
}
fn cache_io(io: u64, request: u64, source: Option<u64>) -> String {
    let (target, source) = source.map_or(("cache_read", "null".to_owned()), |id| {
        ("cache_write", format!("\"{id}\""))
    });
    format!(
        r#"{{"event":"io_submit","io":"{io}","request":"{request}","target":"{target}","slot":"0","source":{source}}}"#
    )
}
fn publish(io: u64) -> String {
    format!(r#"{{"event":"cache_publish","io":"{io}"}}"#)
}
fn cache_visible(valid: bool) -> String {
    format!(r#"{{"event":"assert_cache","slot":"0","valid":{valid}}}"#)
}
fn invalidate() -> String {
    r#"{"event":"epoch_invalidate","reason":"disable"}"#.to_owned()
}
fn resume() -> String {
    r#"{"event":"resume_cache"}"#.to_owned()
}
fn warm(line: u64, len: u64) -> Vec<String> {
    vec![
        bind(line),
        fill(1, line),
        io(1, 1, "origin_read"),
        transfer(1, 0, len),
        complete(1, "ok", len),
        cache_io(2, 1, Some(1)),
        transfer(2, 0, len),
        complete(2, "ok", len),
        publish(2),
    ]
}

#[test]
fn cold_fill_and_cached_read_survive_source_retirement() {
    let mut events = warm(0, 2);
    events.extend([
        control("retire_request", 1),
        cache_visible(true),
        read(2, 0, 2),
        cache_io(3, 2, None),
        transfer(3, 0, 2),
        complete(3, "ok", 2),
        commit(2, 3),
        assert_read(2, "4141", "11"),
    ]);
    let r = run_trace(&trace("4141", &events));
    assert_eq!(r.evidence().outcome, Outcome::Passed, "{:?}", r.evidence());
    assert_eq!(
        r.evidence().checks[Category::CacheLineage.index()].passed,
        3
    );
    assert_eq!(r.resource_usage().owned_cells, 4); // read result + child; slot is fixed media.
}

#[test]
fn stale_fill_is_rejected_but_faulty_candidate_fails_independent_lineage_check() {
    for final_value in ["42", "41"] {
        // Includes byte-equal ABA; lineage must still be stale.
        let mut events = warm(0, 1)[..5].to_vec();
        events.extend([
            write(2, 0, "42", false),
            io(3, 2, "origin_write"),
            transfer(3, 0, 1),
            complete(3, "ok", 1),
            mutation_commit(2, 3),
        ]);
        if final_value == "41" {
            events.extend([
                write(3, 0, "41", false),
                io(4, 3, "origin_write"),
                transfer(4, 0, 1),
                complete(4, "ok", 1),
                mutation_commit(3, 4),
            ]);
        }
        events.extend([
            cache_io(2, 1, Some(1)),
            transfer(2, 0, 1),
            complete(2, "ok", 1),
            publish(2),
        ]);
        let input = trace("41", &events);
        let good = run_trace(&input);
        assert_eq!(good.decision(events.len() - 1), Some(Decision::Rejected));
        assert_eq!(
            good.evidence().outcome,
            Outcome::Passed,
            "{:?}",
            good.evidence()
        );
        let bad = run_trace_with_fault(&input, Fault::StaleFill);
        assert_eq!(
            bad.evidence().violation.as_ref().unwrap().code,
            ViolationCode::StalePublication
        );
        assert_eq!(
            bad.evidence().checks[Category::CacheLineage.index()].failed,
            1
        );
        assert_eq!(bad.evidence().outcome, Outcome::Failed);
    }
}

#[test]
fn early_ack_mutant_is_caught_before_any_actual_completion() {
    let events = vec![
        write(1, 0, "42", false),
        io(1, 1, "origin_write"),
        mutation_commit(1, 1),
        transfer(1, 0, 1),
        complete(1, "ok", 1),
        mutation_commit(1, 1),
    ];
    let input = trace("41", &events);
    let good = run_trace(&input);
    assert_eq!(good.decision(2), Some(Decision::Pending));
    assert_eq!(good.evidence().outcome, Outcome::Passed);
    let bad = run_trace_with_fault(&input, Fault::EarlyAck);
    assert_eq!(
        bad.evidence().violation.as_ref().unwrap().code,
        ViolationCode::EarlyAck
    );
    assert_eq!(bad.evidence().events_executed, 3);
    assert_eq!(
        bad.evidence().checks[Category::OriginResults.index()].failed,
        1
    );
}

#[test]
fn cancelled_or_disabled_cache_writer_keeps_physical_lease_until_actual_completion() {
    for stop in [r#"{"event":"cancel_io","io":"2"}"#.to_owned(), invalidate()] {
        let mut events = warm(0, 1)[..6].to_vec();
        events.extend([
            stop,
            bind(0),
            transfer(2, 0, 1),
            complete(2, "ok", 1),
            bind(0),
        ]);
        let input = trace("41", &events);
        let good = run_trace(&input);
        assert_eq!(good.decision(7), Some(Decision::Pending));
        assert_eq!(good.decision(10), Some(Decision::Accepted));
        assert_eq!(
            good.evidence().outcome,
            Outcome::Passed,
            "{:?}",
            good.evidence()
        );
        let bad = run_trace_with_fault(&input, Fault::LateSlotReuse);
        assert_eq!(
            bad.evidence().violation.as_ref().unwrap().code,
            ViolationCode::LiveLeaseReused
        );
        assert_eq!(bad.evidence().checks[Category::Ownership.index()].failed, 1);
    }
}

#[test]
fn physical_overwrite_invalidates_old_visibility_even_if_new_writer_fails() {
    let mut events = warm(0, 1);
    events.extend([
        fill(2, 0),
        io(3, 2, "origin_read"),
        transfer(3, 0, 1),
        complete(3, "ok", 1),
        cache_io(4, 2, Some(3)),
        cache_visible(false),
        complete(4, "io_error", 0),
        cache_visible(false),
        publish(4),
        read(3, 0, 1),
        cache_io(5, 3, None),
    ]);
    let r = run_trace(&trace("41", &events));
    assert_eq!(r.evidence().violation, None);
    assert_eq!(r.decision(17), Some(Decision::Rejected));
    assert_eq!(r.decision(19), Some(Decision::Rejected));
}

#[test]
fn old_completed_write_cannot_publish_over_new_content_owner() {
    let mut events = warm(0, 1)[..8].to_vec();
    events.extend([
        fill(2, 0),
        io(3, 2, "origin_read"),
        transfer(3, 0, 1),
        complete(3, "ok", 1),
        cache_io(4, 2, Some(3)),
        transfer(4, 0, 1),
        complete(4, "ok", 1),
        publish(2),
        cache_visible(false),
        publish(4),
        cache_visible(true),
    ]);
    let r = run_trace(&trace("41", &events));
    assert_eq!(r.evidence().violation, None);
    assert_eq!(r.decision(15), Some(Decision::Rejected));
    assert_eq!(r.decision(17), Some(Decision::Accepted));
}

#[test]
fn writer_count_not_boolean_and_failed_partial_write_does_not_restore_visibility() {
    let mut events = warm(0, 2);
    events.extend([
        write(2, 0, "42", false),
        write(3, 1, "43", false),
        io(3, 2, "origin_write"),
        complete(3, "io_error", 0),
        fill(4, 0),
        read(5, 0, 1),
        cache_io(4, 5, None),
        io(5, 3, "origin_write"),
        transfer(5, 0, 1),
        complete(5, "io_error", 0),
        cache_visible(false),
        fill(6, 0),
    ]);
    let r = run_trace(&trace("4141", &events));
    assert_eq!(r.evidence().violation, None);
    assert_eq!(r.decision(13), Some(Decision::Rejected));
    assert_eq!(r.decision(15), Some(Decision::Rejected));
    assert_eq!(r.decision(20), Some(Decision::Accepted));
}

#[test]
fn cache_short_error_or_missing_coverage_cannot_become_hit_and_fallback_preserves_origin_error() {
    let mut events = warm(0, 2);
    events.extend([
        read(2, 0, 2),
        cache_io(3, 2, None),
        transfer(3, 0, 1),
        complete(3, "ok", 2),
        commit(2, 3),
        io(4, 2, "origin_read"),
        transfer(4, 0, 1),
        complete(4, "io_error", 1),
        commit(2, 4),
        assert_read(2, "41", "1"),
    ]);
    let r = run_trace(&trace("4141", &events));
    assert_eq!(r.decision(13), Some(Decision::Rejected));
    assert_eq!(r.evidence().outcome, Outcome::Passed, "{:?}", r.evidence());
    assert_eq!(
        r.caller_result(2).unwrap().status(),
        cache_types::trace::IoStatus::IoError
    );
}

#[test]
fn caller_commit_does_not_release_late_cache_child_or_mutate_immutable_result() {
    let mut events = warm(0, 1);
    events.extend([
        read(2, 0, 1),
        cache_io(3, 2, None),
        io(4, 2, "origin_read"),
        transfer(4, 0, 1),
        complete(4, "ok", 1),
        commit(2, 4),
        bind(0),
        control("retire_request", 2),
        transfer(3, 0, 1),
        complete(3, "ok", 1),
        assert_read(2, "41", "1"),
        control("retire_request", 2),
        bind(0),
    ]);
    let r = run_trace(&trace("41", &events));
    assert_eq!(r.evidence().outcome, Outcome::Passed, "{:?}", r.evidence());
    assert_eq!(r.decision(15), Some(Decision::Pending));
    assert_eq!(r.decision(16), Some(Decision::Pending));
    assert_eq!(r.decision(21), Some(Decision::Accepted));
}

#[test]
fn zero_length_cache_reads_use_containing_half_open_line_not_previous_line() {
    let r = run_trace(&trace("", &[read(1, 0, 0), cache_io(1, 1, None)]));
    assert_eq!(r.decision(1), Some(Decision::Rejected));
    let mut events = warm(0, 2);
    events.extend([
        read(2, 1, 0),
        cache_io(3, 2, None),
        complete(3, "ok", 0),
        commit(2, 3),
        assert_read(2, "", ""),
        read(3, 2, 0),
        cache_io(4, 3, None),
    ]);
    let r = run_trace(&trace("4141", &events));
    assert_eq!(r.evidence().violation, None);
    assert_eq!(r.decision(10), Some(Decision::Accepted));
    assert_eq!(r.decision(15), Some(Decision::Rejected));
    let mut events = warm(1, 1);
    events.extend([
        read(2, 4096, 0),
        cache_io(3, 2, None),
        complete(3, "ok", 0),
        commit(2, 3),
        read(3, 4095, 0),
        cache_io(4, 3, None),
    ]);
    let r = run_trace(&trace(&"41".repeat(4097), &events));
    assert_eq!(r.evidence().violation, None);
    assert_eq!(r.decision(10), Some(Decision::Accepted));
    assert_eq!(r.decision(14), Some(Decision::Rejected));
}

#[test]
fn counter_exhaustion_never_wraps_and_origin_passthrough_stays_available() {
    let mut events = warm(0, 1)[..6].to_vec();
    events.extend([
        cache_visible(false),
        bind(0),
        write(2, 0, "42", false),
        io(3, 2, "origin_write"),
        transfer(3, 0, 1),
        complete(3, "ok", 1),
        mutation_commit(2, 3),
        fill(3, 0),
        invalidate(),
        resume(),
        origin("42", "1"),
    ]);
    let r = run_trace(&trace_options("41", &events, 1));
    assert_eq!(r.evidence().violation, None);
    assert_eq!(r.decision(5), Some(Decision::Rejected));
    assert_eq!(r.decision(7), Some(Decision::Rejected));
    assert_eq!(r.decision(13), Some(Decision::Rejected));
    assert_eq!(r.decision(15), Some(Decision::Rejected));
}

#[test]
fn disabled_resume_waits_for_old_children_and_unforwarded_writers() {
    let mut events = warm(0, 1)[..6].to_vec();
    events.extend([
        invalidate(),
        resume(),
        transfer(2, 0, 1),
        complete(2, "ok", 1),
        resume(),
        cache_visible(false),
        write(2, 0, "42", false),
        invalidate(),
        resume(),
        io(3, 2, "origin_write"),
        complete(3, "io_error", 0),
        resume(),
    ]);
    let r = run_trace(&trace("41", &events));
    assert_eq!(r.evidence().violation, None);
    assert_eq!(r.decision(7), Some(Decision::Pending));
    assert_eq!(r.decision(10), Some(Decision::Accepted));
    assert_eq!(r.decision(14), Some(Decision::Pending));
    assert_eq!(r.decision(17), Some(Decision::Accepted));
}

#[test]
fn mutation_child_reservation_survives_shared_cache_children_and_retry_uses_new_id() {
    let mut events = warm(0, 1);
    events.extend([control("retire_request", 1), write(2, 0, "", false)]);
    for id in 3..=1025 {
        events.extend([read(id, 0, 0), cache_io(id, id, None)]);
    }
    let at = events.len();
    events.extend([
        io(2000, 3, "origin_read"),
        io(2001, 2, "origin_write"),
        io(2002, 3, "origin_read"),
        complete(3, "ok", 0),
        io(2003, 3, "origin_read"),
    ]);
    let r = run_trace(&trace("41", &events));
    assert_eq!(r.evidence().violation, None);
    assert_eq!(r.decision(at - 1), Some(Decision::Accepted));
    assert_eq!(r.decision(at), Some(Decision::Pending));
    assert_eq!(r.decision(at + 1), Some(Decision::Accepted));
    assert_eq!(r.decision(at + 2), Some(Decision::Pending));
    assert_eq!(r.decision(at + 4), Some(Decision::Accepted));
    assert_eq!(r.evidence().pending_ios, 1024);
}

#[test]
fn cache_sources_must_be_complete_successful_and_known_and_cache_coverage_is_separate() {
    let mut events = warm(0, 2)[..4].to_vec();
    events.extend([
        cache_io(2, 1, Some(1)),
        complete(1, "ok", 2),
        cache_io(3, 1, Some(1)),
        transfer(3, 0, 1),
        complete(3, "ok", 2),
        publish(3),
    ]);
    let r = run_trace(&trace("4141", &events));
    assert_eq!(r.evidence().violation, None);
    assert_eq!(r.decision(4), Some(Decision::Pending));
    assert_eq!(r.decision(6), Some(Decision::Accepted));
    assert_eq!(r.decision(9), Some(Decision::Rejected));
    for completion in [complete(1, "io_error", 2), complete(1, "ok", 1)] {
        let mut events = warm(0, 2)[..4].to_vec();
        events.extend([completion, cache_io(2, 1, Some(1))]);
        let r = run_trace(&trace("4141", &events));
        assert_eq!(r.decision(5), Some(Decision::Rejected));
        assert_eq!(r.evidence().violation, None);
    }
    let events = vec![
        discard(1, 0, 1),
        io(1, 1, "origin_discard"),
        transfer(1, 0, 1),
        complete(1, "ok", 1),
        bind(0),
        fill(2, 0),
        io(2, 2, "origin_read"),
        transfer(2, 0, 1),
        complete(2, "ok", 1),
        cache_io(3, 2, Some(2)),
    ];
    let r = run_trace(&trace("41", &events));
    assert_eq!(r.evidence().violation, None);
    assert_eq!(r.decision(9), Some(Decision::Rejected));
}

#[test]
fn hit_final_validation_detects_mutation_or_rebind_after_actual_read_completion() {
    for mutate in [true, false] {
        let mut events = warm(0, 1);
        events.extend([
            read(2, 0, 1),
            cache_io(3, 2, None),
            transfer(3, 0, 1),
            complete(3, "ok", 1),
        ]);
        if mutate {
            events.extend([
                write(3, 0, "42", false),
                io(4, 3, "origin_write"),
                transfer(4, 0, 1),
                complete(4, "ok", 1),
                mutation_commit(3, 4),
            ]);
        } else {
            events.push(bind(0));
        }
        let at = events.len();
        events.extend([
            commit(2, 3),
            io(5, 2, "origin_read"),
            transfer(5, 0, 1),
            complete(5, "ok", 1),
            commit(2, 5),
            assert_read(2, if mutate { "42" } else { "41" }, "1"),
        ]);
        let r = run_trace(&trace("41", &events));
        assert_eq!(r.decision(at), Some(Decision::Rejected));
        assert_eq!(r.evidence().outcome, Outcome::Passed, "{:?}", r.evidence());
    }
}

#[test]
fn duplicate_publish_is_semantic_and_cross_line_hit_falls_back() {
    let mut events = warm(0, 1);
    events.push(publish(2));
    let r = run_trace(&trace("41", &events));
    assert_eq!(
        r.evidence().violation.as_ref().unwrap().code,
        ViolationCode::InvalidPhase
    );
    let mut events = warm(0, 4096);
    events.extend([
        read(2, 4095, 2),
        cache_io(3, 2, None),
        io(4, 2, "origin_read"),
        transfer(4, 0, 2),
        complete(4, "ok", 2),
        commit(2, 4),
        assert_read(2, "4141", "11"),
    ]);
    let r = run_trace(&trace(&"41".repeat(4097), &events));
    assert_eq!(r.decision(10), Some(Decision::Rejected));
    assert_eq!(r.evidence().outcome, Outcome::Passed);
}

#[test]
fn evidence_counts_evaluation_points_and_stops_before_later_stages() {
    use cache_types::evidence::Check;
    let cases = [
        (
            vec![read(1, 0, 1), io(1, 1, "origin_read")],
            Fault::None,
            Outcome::Inconclusive,
            Terminal::Pending,
            None,
            [(0, 0), (0, 0), (1, 0), (0, 0), (0, 0), (0, 0)],
        ),
        (
            vec![read(1, 0, 1), io(1, 1, "origin_read"), complete(1, "ok", 1)],
            Fault::None,
            Outcome::Failed,
            Terminal::Stopped,
            Some(ViolationCode::ReadReportedUnsampled),
            [(0, 1), (0, 0), (1, 0), (0, 0), (0, 0), (0, 0)],
        ),
        (
            vec![
                write(1, 0, "42", true),
                io(1, 1, "origin_write"),
                transfer(1, 0, 1),
                complete(1, "ok", 1),
            ],
            Fault::None,
            Outcome::Failed,
            Terminal::Stopped,
            Some(ViolationCode::FuaNotDurable),
            [(1, 0), (0, 0), (1, 0), (0, 1), (0, 0), (0, 0)],
        ),
        (
            vec![
                write(1, 0, "42", false),
                io(1, 1, "origin_write"),
                mutation_commit(1, 1),
            ],
            Fault::EarlyAck,
            Outcome::Failed,
            Terminal::Stopped,
            Some(ViolationCode::EarlyAck),
            [(0, 1), (0, 0), (1, 0), (0, 0), (0, 0), (0, 0)],
        ),
        (
            vec![write(1, 0, "42", false), power_loss(), origin("42", "1")],
            Fault::None,
            Outcome::Failed,
            Terminal::Stopped,
            Some(ViolationCode::AssertOrigin),
            [(0, 0), (0, 0), (0, 0), (0, 0), (0, 1), (0, 0)],
        ),
    ];
    for (events, fault, outcome, terminal, code, checks) in cases {
        let result = run_trace_with_fault(&trace("41", &events), fault);
        let report = result.evidence();
        assert_eq!(report.outcome, outcome, "{report:?}");
        assert_eq!(report.terminal, terminal);
        assert_eq!(report.violation.as_ref().map(|v| v.code), code);
        assert_eq!(
            report.checks,
            checks.map(|(passed, failed)| Check { passed, failed })
        );
        assert_eq!(report.events_executed, events.len() as u64);
        assert_eq!(
            parse_evidence(&encode_evidence(report).unwrap()).unwrap(),
            *report
        );
        if code == Some(ViolationCode::AssertOrigin) {
            assert_eq!(report.abandoned_requests, 1);
            assert_eq!(report.abandoned_ios, 0);
            assert_eq!(report.pending_requests, 0);
            assert_eq!(report.meaningful_observations(), 0);
        }
    }
}

#[test]
fn retained_guard_counts_once_after_each_successful_nonassert_event() {
    let mut events = baseline();
    events.extend([
        control("retire_request", 1),
        invalidate(),
        resume(),
        power_loss(),
    ]);
    let result = run_trace(&trace("4141", &events));
    let report = result.evidence();
    assert_eq!(report.outcome, Outcome::Passed, "{report:?}");
    assert_eq!(report.checks[Category::Ownership.index()].passed, 8);
    assert_eq!(report.checks[Category::OriginResults.index()].passed, 2);
    assert_eq!(report.checks[Category::Durability.index()].passed, 0);
    assert_eq!(report.meaningful_observations(), 10);
    assert_eq!(report.assertions_checked(), 1);
    assert_eq!(
        parse_evidence(&encode_evidence(report).unwrap()).unwrap(),
        *report
    );
}

#[test]
fn cache_counts_admission_publication_and_final_claim_as_separate_points() {
    let mut events = warm(0, 1);
    events.extend([
        read(2, 0, 1),
        cache_io(3, 2, None),
        transfer(3, 0, 1),
        complete(3, "ok", 1),
        commit(2, 3),
        assert_read(2, "41", "1"),
    ]);
    let result = run_trace(&trace("41", &events));
    let report = result.evidence();
    assert_eq!(report.outcome, Outcome::Passed, "{report:?}");
    assert_eq!(report.checks[Category::Ownership.index()].passed, 5);
    assert_eq!(report.checks[Category::OriginResults.index()].passed, 1);
    assert_eq!(report.checks[Category::CacheLineage.index()].passed, 3);
    assert_eq!(report.meaningful_observations(), 9);
    assert_eq!(report.assertions_checked(), 1);
    assert_eq!(
        parse_evidence(&encode_evidence(report).unwrap()).unwrap(),
        *report
    );
}
