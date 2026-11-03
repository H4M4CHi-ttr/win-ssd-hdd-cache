use cache_types::evidence::{
    Category, EVIDENCE_MAX_BYTES, Fault, Outcome, Terminal, ViolationCode, encode_evidence,
    parse_evidence,
};

fn baseline() -> String {
    concat!(
        "{\"schema_version\":1,\"trace_schema_version\":1,\"oracle_contract_version\":1,",
        "\"mode\":\"reference\",\"fault\":\"none\",\"seed\":\"42\",",
        "\"source_revision\":null,\"trace_digest\":null,\"events_total\":\"6\",",
        "\"events_executed\":\"6\",\"outcome\":\"passed\",\"terminal\":\"drained\",",
        "\"meaningful_observations\":\"4\",\"assertions_checked\":\"1\",",
        "\"pending_requests\":\"0\",\"pending_ios\":\"0\",",
        "\"abandoned_requests\":\"0\",\"abandoned_ios\":\"0\",\"violation\":null,",
        "\"checks\":{",
        "\"origin_results\":{\"status\":\"passed\",\"passed\":\"2\",\"failed\":\"0\"},",
        "\"cache_lineage\":{\"status\":\"not_observed\",\"passed\":\"0\",\"failed\":\"0\"},",
        "\"ownership\":{\"status\":\"passed\",\"passed\":\"2\",\"failed\":\"0\"},",
        "\"durability\":{\"status\":\"not_observed\",\"passed\":\"0\",\"failed\":\"0\"},",
        "\"trace_assertions\":{\"status\":\"passed\",\"passed\":\"1\",\"failed\":\"0\"},",
        "\"shadow\":{\"status\":\"not_applicable\",\"passed\":\"0\",\"failed\":\"0\"}},",
        "\"kernel_runtime_tested\":false,\"production_qualified\":false}\n"
    )
    .to_owned()
}

fn reject(input: &str) {
    let error = parse_evidence(input).expect_err("contradictory report was accepted");
    assert!(!format!("{error} {error:?}").contains("SECRET"));
    assert!(!format!("{error} {error:?}").contains("serde"));
}

#[test]
fn literal_baseline_round_trips_with_fixed_order_and_newline() {
    let raw = baseline();
    let report = parse_evidence(&raw).unwrap();
    assert_eq!(report.fault, Fault::None);
    assert_eq!(report.seed, 42);
    assert_eq!(report.outcome, Outcome::Passed);
    assert_eq!(report.terminal, Terminal::Drained);
    assert_eq!(report.meaningful_observations(), 4);
    assert_eq!(report.assertions_checked(), 1);
    assert_eq!(report.checks[Category::OriginResults.index()].passed, 2);
    assert_eq!(encode_evidence(&report).unwrap(), raw);
}

#[test]
fn required_nulls_objects_duplicates_and_unknown_keys_are_strict() {
    let base = baseline();
    for field in ["source_revision", "trace_digest", "violation"] {
        reject(&base.replace(&format!("\"{field}\":null,"), ""));
        reject(&base.replace(&format!("\"{field}\":null"), &format!("\"{field}\":[]")));
    }
    for input in [
        "[]".to_owned(),
        "null".to_owned(),
        base.replacen('{', "{\"schema_version\":1,", 1),
        base.replacen('{', "{\"SECRET\":42,", 1),
        base.replace("\"seed\":\"42\"", "\"seed\":\"42\",\"se\\u0065d\":\"42\""),
        base.replace("\"checks\":{", "\"checks\":{\"SECRET\":{},"),
        base.replace(
            "\"status\":\"passed\"",
            "\"status\":\"passed\",\"status\":\"passed\"",
        ),
        base.replace("\"status\":\"passed\"", "\"status\":{\"passed\":null}"),
        base.replace("\"passed\":\"2\"", "\"passed\":\"2\",\"SECRET\":0"),
        base.replace(
            "\"origin_results\":{\"status\":\"passed\",\"passed\":\"2\",\"failed\":\"0\"}",
            "\"origin_results\":[\"passed\",\"2\",\"0\"]",
        ),
        format!("{base}{{}}"),
    ] {
        reject(&input);
    }
}

#[test]
fn all_versions_require_exact_integer_tokens_and_false_guarantees() {
    for field in [
        "schema_version",
        "trace_schema_version",
        "oracle_contract_version",
    ] {
        for token in [
            "0",
            "2",
            "-0",
            "1.0",
            "1e0",
            "true",
            "\"1\"",
            "null",
            "18446744073709551616",
        ] {
            reject(&baseline().replace(&format!("\"{field}\":1"), &format!("\"{field}\":{token}")));
        }
    }
    for field in ["kernel_runtime_tested", "production_qualified"] {
        for token in ["true", "0", "\"false\"", "null"] {
            reject(&baseline().replace(
                &format!("\"{field}\":false"),
                &format!("\"{field}\":{token}"),
            ));
        }
    }
}

#[test]
fn counters_are_canonical_u64_and_provenance_is_explicit_nullable_hex() {
    for value in [
        "",
        "00",
        "01",
        "-1",
        "+1",
        "1.0",
        "1e0",
        " 1",
        "18446744073709551616",
        "SECRET",
    ] {
        reject(&baseline().replace("\"seed\":\"42\"", &format!("\"seed\":\"{value}\"")));
    }
    assert!(
        parse_evidence(&baseline().replace("\"seed\":\"42\"", "\"seed\":\"18446744073709551615\""))
            .is_ok()
    );
    reject(&baseline().replace("\"seed\":\"42\"", "\"seed\":42"));
    for (field, length) in [("source_revision", 40), ("trace_digest", 64)] {
        let valid = baseline().replace(
            &format!("\"{field}\":null"),
            &format!("\"{field}\":\"{}\"", "a".repeat(length)),
        );
        assert!(parse_evidence(&valid).is_ok());
        for text in [
            "A".repeat(length),
            "a".repeat(length - 1),
            "g".repeat(length),
            "SECRET".to_owned(),
        ] {
            reject(&baseline().replace(
                &format!("\"{field}\":null"),
                &format!("\"{field}\":\"{text}\""),
            ));
        }
    }
}

#[test]
fn report_count_status_outcome_and_phase_contradictions_are_rejected() {
    for (before, after) in [
        (
            "\"meaningful_observations\":\"4\"",
            "\"meaningful_observations\":\"5\"",
        ),
        (
            "\"assertions_checked\":\"1\"",
            "\"assertions_checked\":\"2\"",
        ),
        ("\"events_total\":\"6\"", "\"events_total\":\"5\""),
        ("\"events_total\":\"6\"", "\"events_total\":\"10001\""),
        ("\"events_executed\":\"6\"", "\"events_executed\":\"5\""),
        ("\"outcome\":\"passed\"", "\"outcome\":\"inconclusive\""),
        ("\"outcome\":\"passed\"", "\"outcome\":\"failed\""),
        ("\"terminal\":\"drained\"", "\"terminal\":\"stopped\""),
        ("\"terminal\":\"drained\"", "\"terminal\":\"pending\""),
        ("\"pending_requests\":\"0\"", "\"pending_requests\":\"1\""),
        ("\"abandoned_ios\":\"0\"", "\"abandoned_ios\":\"1\""),
        ("\"status\":\"passed\"", "\"status\":\"not_observed\""),
        (
            "\"status\":\"not_applicable\"",
            "\"status\":\"not_observed\"",
        ),
        ("\"fault\":\"none\"", "\"fault\":\"stale_fill\""),
    ] {
        reject(&baseline().replace(before, after));
    }
    let mut report = parse_evidence(&baseline()).unwrap();
    report.checks[Category::OriginResults.index()].passed = u64::MAX;
    assert!(encode_evidence(&report).is_err());
    report = parse_evidence(&baseline()).unwrap();
    report.source_revision = Some("SECRET\"injection".to_owned());
    assert!(encode_evidence(&report).is_err());
}

#[test]
fn empty_pending_and_power_loss_are_distinct_valid_verdicts() {
    let mut report = parse_evidence(&baseline()).unwrap();
    report.terminal = Terminal::Pending;
    report.outcome = Outcome::Inconclusive;
    report.pending_ios = 1;
    assert!(encode_evidence(&report).is_ok());
    report.pending_ios = 0;
    report.terminal = Terminal::PowerLoss;
    report.outcome = Outcome::Passed;
    report.abandoned_ios = 1;
    assert!(encode_evidence(&report).is_ok());
    report.abandoned_ios = 0;
    report.terminal = Terminal::Drained;
    report.outcome = Outcome::Inconclusive;
    report.events_total = 0;
    report.events_executed = 0;
    report.checks.fill(Default::default());
    let raw = encode_evidence(&report).unwrap();
    assert_eq!(parse_evidence(&raw).unwrap(), report);
}

#[test]
fn stopped_semantic_and_safety_failures_have_exact_indices_and_fixed_codes() {
    let failed = baseline()
        .replace("\"outcome\":\"passed\"", "\"outcome\":\"failed\"")
        .replace("\"terminal\":\"drained\"", "\"terminal\":\"stopped\"")
        .replace("\"violation\":null", "\"violation\":{\"kind\":\"semantic_error\",\"code\":\"INVALID_PHASE\",\"event_index\":\"5\"}");
    let mut report = parse_evidence(&failed).unwrap();
    assert_eq!(
        report.violation.as_ref().unwrap().code,
        ViolationCode::InvalidPhase
    );
    for (before, after) in [
        ("\"event_index\":\"5\"", "\"event_index\":\"6\""),
        ("\"event_index\":\"5\"", "\"event_index\":null"),
        ("\"event_index\":\"5\"", "\"event_index\":\"05\""),
        (
            "\"kind\":\"semantic_error\"",
            "\"kind\":\"safety_violation\"",
        ),
        ("\"code\":\"INVALID_PHASE\"", "\"code\":\"SECRET\""),
        (
            "\"code\":\"INVALID_PHASE\"",
            "\"code\":{\"INVALID_PHASE\":null}",
        ),
        (
            "\"kind\":\"semantic_error\"",
            "\"kind\":\"semantic_error\",\"kind\":\"semantic_error\"",
        ),
    ] {
        reject(&failed.replace(before, after));
    }
    report.fault = Fault::EarlyAck;
    report.violation.as_mut().unwrap().code = ViolationCode::EarlyAck;
    assert!(encode_evidence(&report).is_err()); // Safety failure must have a failed observation.
    report.checks[Category::OriginResults.index()].failed = 1;
    let raw = encode_evidence(&report).unwrap();
    assert!(raw.contains("\"mode\":\"mutant\""));
    assert_eq!(parse_evidence(&raw).unwrap().outcome, Outcome::Failed);
}

#[test]
fn size_limit_and_parse_errors_never_echo_input() {
    reject("{SECRET");
    let raw = " ".repeat(EVIDENCE_MAX_BYTES + 1);
    assert_eq!(parse_evidence(&raw).unwrap_err().code(), "INPUT_TOO_LARGE");
    let raw = format!(
        "{}{}",
        baseline(),
        " ".repeat(EVIDENCE_MAX_BYTES - baseline().len())
    );
    assert!(parse_evidence(&raw).is_ok());
}

#[test]
fn failure_can_follow_power_loss_and_keep_abandoned_counts() {
    let mut report = parse_evidence(&baseline()).unwrap();
    report.terminal = Terminal::Stopped;
    report.outcome = Outcome::Failed;
    report.abandoned_requests = 2;
    report.abandoned_ios = 1;
    report.violation = Some(cache_types::evidence::Violation {
        code: ViolationCode::AssertOrigin,
        event_index: 5,
    });
    report.checks[Category::TraceAssertions.index()].failed = 1;
    assert_eq!(
        parse_evidence(&encode_evidence(&report).unwrap()).unwrap(),
        report
    );
}

#[test]
fn zero_meaningful_assert_failure_is_failed_and_mutant_need_not_always_fail() {
    let mut report = parse_evidence(&baseline()).unwrap();
    report.fault = Fault::StaleFill;
    assert!(encode_evidence(&report).is_ok());
    report.checks.fill(Default::default());
    report.events_total = 3;
    report.events_executed = 1;
    report.terminal = Terminal::Stopped;
    report.outcome = Outcome::Failed;
    report.violation = Some(cache_types::evidence::Violation {
        code: ViolationCode::AssertOrigin,
        event_index: 0,
    });
    report.checks[Category::TraceAssertions.index()].failed = 1;
    assert!(encode_evidence(&report).is_ok());
    report.violation.as_mut().unwrap().code = ViolationCode::InvalidPhase;
    report.checks[Category::TraceAssertions.index()].failed = 0;
    assert!(encode_evidence(&report).is_ok());
}

#[test]
fn two_ownership_checks_per_event_and_prior_stage_success_are_legal() {
    let mut report = parse_evidence(&baseline()).unwrap();
    report.checks[Category::Ownership.index()].passed = 10;
    assert!(encode_evidence(&report).is_ok());
    report.terminal = Terminal::Stopped;
    report.outcome = Outcome::Failed;
    report.violation = Some(cache_types::evidence::Violation {
        code: ViolationCode::FuaNotDurable,
        event_index: 5,
    });
    report.checks[Category::Durability.index()].failed = 1;
    assert!(encode_evidence(&report).is_ok());
    report.checks[Category::Ownership.index()].failed = 1;
    assert!(encode_evidence(&report).is_err()); // Stop at the first failed stage.
}

#[test]
fn durable_history_mismatch_is_backend_durability_not_origin_result_failure() {
    let mut report = parse_evidence(&baseline()).unwrap();
    report.terminal = Terminal::Stopped;
    report.outcome = Outcome::Failed;
    report.violation = Some(cache_types::evidence::Violation {
        code: ViolationCode::DurableHistoryMismatch,
        event_index: 5,
    });
    report.checks[Category::Durability.index()].failed = 1;
    assert_eq!(
        parse_evidence(&encode_evidence(&report).unwrap()).unwrap(),
        report
    );
    report.checks[Category::Durability.index()].failed = 0;
    report.checks[Category::OriginResults.index()].failed = 1;
    assert!(encode_evidence(&report).is_err());
}
