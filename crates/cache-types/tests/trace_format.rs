use cache_types::trace::{
    TRACE_MAX_BYTES, TRACE_MAX_CAPACITY, TRACE_MAX_EVENTS, TRACE_MAX_PAYLOAD_BYTES, TraceEvent,
    parse_trace,
};

fn doc(capacity: usize, events: &str) -> String {
    format!(
        r#"{{"schema_version":1,"seed":"42","capacity":"{capacity}","line_size":4096,"slot_count":1,"counter_limit":"18446744073709551615","initial_hex":"{}","events":[{events}]}}"#,
        "00".repeat(capacity)
    )
}

fn error(input: &str, code: &str) {
    let err = parse_trace(input).expect_err("invalid trace was accepted");
    assert_eq!(err.code(), code);
    let rendered = format!("{err} {err:?}");
    assert!(!rendered.contains("SECRET"));
    assert!(!rendered.contains("serde"));
}

const READ: &str = r#"{"event":"read_submit","request":"1","offset":"0","length":"2"}"#;
const READ_IO: &str = r#"{"event":"io_submit","io":"1","request":"1","target":"origin_read","slot":null,"source":null}"#;
const FILL: &str = r#"{"event":"fill_begin","request":"7","line":"0"}"#;
const FILL_IO: &str = r#"{"event":"io_submit","io":"8","request":"7","target":"origin_read","slot":null,"source":null}"#;
const CACHE_WRITE: &str = r#"{"event":"io_submit","io":"9","request":"7","target":"cache_write","slot":"0","source":"8"}"#;

#[test]
fn golden_has_all_events_and_read_only_accessors() {
    let t = parse_trace(include_str!(
        "../../../tests/fixtures/trace-format/all-events.json"
    ))
    .unwrap();
    assert_eq!(t.seed(), 42);
    assert_eq!(t.capacity(), 2);
    assert_eq!(t.line_size(), 4096);
    assert_eq!(t.slot_count(), 1);
    assert_eq!(t.counter_limit(), u64::MAX);
    assert_eq!(t.initial(), &[0, 0]);
    assert_eq!(t.events().len(), 41);
    assert!(matches!(
        t.events()[0],
        TraceEvent::FillBegin {
            request: 1,
            line: 0
        }
    ));
    let names: std::collections::BTreeSet<_> = t.events().iter().map(TraceEvent::name).collect();
    assert_eq!(names.len(), 24);
}

#[test]
fn internally_tagged_buffer_does_not_accept_externally_tagged_enum_maps() {
    let cases = [
        (format!("{READ},{READ_IO}"), "target", "origin_read"),
        (
            format!(
                "{READ},{READ_IO},{}",
                r#"{"event":"io_complete","io":"1","status":"ok","reported":"0"}"#
            ),
            "status",
            "ok",
        ),
        (
            r#"{"event":"epoch_invalidate","reason":"disable"}"#.to_owned(),
            "reason",
            "disable",
        ),
        (
            format!(
                "{READ},{}",
                r#"{"event":"assert_result","request":"1","status":"ok","reported":"0"}"#
            ),
            "status",
            "ok",
        ),
        (
            format!(
                "{READ},{}",
                r#"{"event":"assert_decision","index":"0","decision":"accepted"}"#
            ),
            "decision",
            "accepted",
        ),
    ];
    for (events, field, value) in cases {
        assert!(parse_trace(&doc(2, &events)).is_ok());
        for token in [
            format!(r#"{{"{value}":null}}"#),
            format!(r#"["{value}"]"#),
            "null".to_owned(),
            "{}".to_owned(),
        ] {
            error(
                &doc(
                    2,
                    &events.replace(
                        &format!(r#""{field}":"{value}""#),
                        &format!(r#""{field}":{token}"#),
                    ),
                ),
                "INVALID_SCHEMA",
            );
        }
    }
}

#[test]
fn empty_zero_capacity_and_eof_zero_length_are_valid() {
    assert!(parse_trace(&doc(0, "")).is_ok());
    assert!(parse_trace(&doc(0, &READ.replace("\"2\"", "\"0\""))).is_ok());
    assert!(
        parse_trace(&doc(
            2,
            &READ.replace(
                "\"offset\":\"0\",\"length\":\"2\"",
                "\"offset\":\"2\",\"length\":\"0\""
            )
        ))
        .is_ok()
    );
}

#[test]
fn integer_fields_reject_strings_bool_float_exponent_and_negative_tokens() {
    for (field, valid) in [
        ("schema_version", "1"),
        ("line_size", "4096"),
        ("slot_count", "1"),
    ] {
        for token in [
            "true",
            "false",
            "null",
            "-0",
            "-1",
            "1.0",
            "1e0",
            "\"1\"",
            "18446744073709551616",
        ] {
            error(
                &doc(2, "").replace(
                    &format!("\"{field}\":{valid}"),
                    &format!("\"{field}\":{token}"),
                ),
                "INVALID_SCHEMA",
            );
        }
    }
}

#[test]
fn unknown_version_and_geometry_are_rejected() {
    error(
        &doc(0, "").replace("\"schema_version\":1", "\"schema_version\":2"),
        "INVALID_SCHEMA",
    );
    for line in ["0", "512", "4095", "65537"] {
        error(
            &doc(0, "").replace("\"line_size\":4096", &format!("\"line_size\":{line}")),
            "INVALID_VALUE",
        );
    }
    for slots in ["0", "257"] {
        error(
            &doc(0, "").replace("\"slot_count\":1", &format!("\"slot_count\":{slots}")),
            "INVALID_VALUE",
        );
    }
    assert!(parse_trace(&doc(0, "").replace("\"slot_count\":1", "\"slot_count\":256")).is_ok());
}

#[test]
fn decimal_strings_are_canonical_bounded_and_typed() {
    for value in [
        "",
        "00",
        "01",
        "+1",
        "-1",
        " 1",
        "1 ",
        "1.0",
        "1e0",
        "18446744073709551616",
        "SECRET",
    ] {
        error(
            &doc(0, "").replace("\"seed\":\"42\"", &format!("\"seed\":\"{value}\"")),
            "INVALID_VALUE",
        );
    }
    error(
        &doc(0, "").replace("\"seed\":\"42\"", "\"seed\":42"),
        "INVALID_SCHEMA",
    );
    error(
        &doc(0, "").replace(
            "\"counter_limit\":\"18446744073709551615\"",
            "\"counter_limit\":\"0\"",
        ),
        "INVALID_VALUE",
    );
    assert!(
        parse_trace(&doc(0, "").replace("\"seed\":\"42\"", "\"seed\":\"18446744073709551615\""))
            .is_ok()
    );
}

#[test]
fn objects_are_not_positional_arrays() {
    error("[]", "INVALID_SCHEMA");
    error(&doc(0, "[]"), "INVALID_SCHEMA");
    error(&doc(0, "\"resume_cache\""), "INVALID_SCHEMA");
}

#[test]
fn unknown_and_duplicate_keys_are_not_erased() {
    let base = doc(2, READ);
    for input in [
        base.replacen("{", "{\"extra\":1,", 1),
        base.replacen("{", "{\"schema_version\":1,", 1),
        base.replacen("{", "{\"schema_\\u0076ersion\":1,", 1),
        doc(2, &READ.replace("{", "{\"extra\":1,")),
        doc(2, &READ.replace("{", "{\"request\":\"1\",")),
        doc(2, &READ.replace("{", "{\"event\":\"read_submit\",")),
        doc(0, r#"{"event":"resume_cache","extra":1}"#),
        doc(0, r#"{"event":"SECRET"}"#),
    ] {
        error(&input, "INVALID_SCHEMA");
    }
}

#[test]
fn required_nullable_fields_cannot_be_missing() {
    for io in [
        READ_IO.replace(",\"slot\":null", ""),
        READ_IO.replace(",\"source\":null", ""),
        READ_IO.replace("\"slot\":null", "\"slot\":false"),
    ] {
        error(&doc(2, &format!("{READ},{io}")), "INVALID_SCHEMA");
    }
    assert!(parse_trace(&doc(2, &format!("{READ},{READ_IO}"))).is_ok());
}

#[test]
fn malformed_json_bom_and_trailing_documents_are_rejected() {
    error("{", "INVALID_JSON");
    error(&(doc(0, "") + "{}"), "INVALID_JSON");
    error(&("\u{feff}".to_owned() + &doc(0, "")), "INVALID_JSON");
}

#[test]
fn raw_input_limit_exact_and_plus_one_precedes_decoding() {
    let mut input = doc(0, "");
    input.extend(std::iter::repeat_n(' ', TRACE_MAX_BYTES - input.len()));
    assert!(parse_trace(&input).is_ok());
    input.push(' ');
    error(&input, "INPUT_TOO_LARGE");
    let huge_invalid = "!".repeat(TRACE_MAX_BYTES + 1);
    error(&huge_invalid, "INPUT_TOO_LARGE");
}

#[test]
fn capacity_limit_exact_and_plus_one() {
    assert!(parse_trace(&doc(TRACE_MAX_CAPACITY, "")).is_ok());
    error(
        &doc(0, "").replace(
            "\"capacity\":\"0\"",
            &format!("\"capacity\":\"{}\"", TRACE_MAX_CAPACITY + 1),
        ),
        "LIMIT_EXCEEDED",
    );
    error(&doc(2, "").replace("\"0000\"", "\"00\""), "INVALID_VALUE");
}

#[test]
fn event_limit_exact_and_plus_one() {
    let events = vec![r#"{"event":"resume_cache"}"#; TRACE_MAX_EVENTS].join(",");
    assert!(parse_trace(&doc(0, &events)).is_ok());
    error(
        &doc(0, &(events + r#",{"event":"resume_cache"}"#)),
        "LIMIT_EXCEEDED",
    );
}

#[test]
fn aggregate_payload_includes_initial_write_and_both_assert_kinds() {
    let cap = TRACE_MAX_CAPACITY;
    assert_eq!(TRACE_MAX_PAYLOAD_BYTES, 16 * cap);
    let data = "00".repeat(cap);
    let known = "1".repeat(cap);
    for mode in ["write", "origin", "read"] {
        let mut events = vec![format!(
            r#"{{"event":"read_submit","request":"99","offset":"0","length":"{cap}"}}"#
        )];
        for id in 1..16 {
            events.push(match mode {
                "write" => format!(r#"{{"event":"write_submit","request":"{id}","offset":"0","data":"{data}","fua":false}}"#),
                "origin" => format!(r#"{{"event":"assert_origin","offset":"0","data":"{data}","known":"{known}"}}"#),
                _ => format!(r#"{{"event":"assert_read","request":"99","data":"{data}","known":"{known}"}}"#),
            });
        }
        let mut input = doc(cap, &events.join(","));
        assert!(input.len() < TRACE_MAX_BYTES);
        assert!(parse_trace(&input).is_ok());
        input.truncate(input.len() - 2);
        input.push_str(
            r#",{"event":"write_submit","request":"100","offset":"0","data":"00","fua":false}]}"#,
        );
        error(&input, "LIMIT_EXCEEDED");
    }
}

#[test]
fn hex_grammar_masks_and_unknown_placeholders() {
    for hex in ["0", "0A", "0x", "zz", "0x00", "00 00"] {
        error(
            &doc(2, "").replace("\"0000\"", &format!("\"{hex}\"")),
            "INVALID_VALUE",
        );
    }
    for (data, known) in [
        ("0000", "1"),
        ("0000", "111"),
        ("0000", "1x"),
        ("0100", "01"),
    ] {
        error(
            &doc(
                2,
                &format!(
                    r#"{{"event":"assert_origin","offset":"0","data":"{data}","known":"{known}"}}"#
                ),
            ),
            "INVALID_VALUE",
        );
    }
    assert!(
        parse_trace(&doc(
            2,
            r#"{"event":"assert_origin","offset":"0","data":"0000","known":"01"}"#
        ))
        .is_ok()
    );
}

#[test]
fn ranges_and_child_relative_offsets_are_checked_without_wrap() {
    for read in [
        READ.replace("\"offset\":\"0\"", "\"offset\":\"3\""),
        READ.replace("\"length\":\"2\"", "\"length\":\"18446744073709551615\""),
    ] {
        error(&doc(2, &read), "INVALID_VALUE");
    }
    let prefix = format!("{READ},{READ_IO}");
    for event in [
        r#"{"event":"io_transfer","io":"1","offset":"1","length":"2"}"#,
        r#"{"event":"io_complete","io":"1","status":"ok","reported":"3"}"#,
    ] {
        error(&doc(2, &format!("{prefix},{event}")), "INVALID_VALUE");
    }
    assert!(
        parse_trace(&doc(
            2,
            &format!(r#"{prefix},{{"event":"io_transfer","io":"1","offset":"2","length":"0"}}"#)
        ))
        .is_ok()
    );
}

#[test]
fn final_short_line_and_slot_bounds() {
    let fill = FILL.replace("\"line\":\"0\"", "\"line\":\"1\"");
    assert!(parse_trace(&doc(4097, &format!("{fill},{FILL_IO}"))).is_ok());
    error(&doc(4096, &fill), "INVALID_VALUE");
    error(&doc(0, FILL), "INVALID_VALUE");
    error(
        &doc(2, r#"{"event":"slot_bind","slot":"1","line":"0"}"#),
        "INVALID_VALUE",
    );
    error(
        &doc(
            2,
            r#"{"event":"slot_bind","slot":"0","line":"18446744073709551615"}"#,
        ),
        "INVALID_VALUE",
    );
    error(
        &doc(
            4097,
            &format!(
                r#"{fill},{FILL_IO},{{"event":"io_transfer","io":"8","offset":"0","length":"2"}}"#
            ),
        ),
        "INVALID_VALUE",
    );
}

#[test]
fn ids_are_positive_unique_per_namespace_not_monotonic_or_counter_limited() {
    error(
        &doc(2, &READ.replace("\"request\":\"1\"", "\"request\":\"0\"")),
        "INVALID_VALUE",
    );
    error(&doc(2, &format!("{READ},{READ}")), "INVALID_REFERENCE");
    error(
        &doc(2, &format!("{READ},{READ_IO},{READ_IO}")),
        "INVALID_REFERENCE",
    );
    assert!(
        parse_trace(&doc(2, &format!("{READ},{READ_IO}")).replace(
            "\"counter_limit\":\"18446744073709551615\"",
            "\"counter_limit\":\"1\""
        ))
        .is_ok()
    );
    let high = READ.replace("\"request\":\"1\"", "\"request\":\"18446744073709551615\"");
    assert!(parse_trace(&doc(2, &format!("{high},{READ}"))).is_ok());
}

#[test]
fn forward_and_unknown_references_are_rejected() {
    error(&doc(2, READ_IO), "INVALID_REFERENCE");
    error(&doc(2, &format!("{READ_IO},{READ}")), "INVALID_REFERENCE");
    for event in [
        r#"{"event":"cancel_request","request":"1"}"#,
        r#"{"event":"cancel_io","io":"1"}"#,
        r#"{"event":"retire_request","request":"1"}"#,
    ] {
        error(&doc(2, event), "INVALID_REFERENCE");
    }
}

#[test]
fn parent_target_and_nullable_arity_are_static_contracts() {
    error(
        &doc(
            2,
            &format!("{READ},{}", READ_IO.replace("origin_read", "origin_write")),
        ),
        "INVALID_REFERENCE",
    );
    error(
        &doc(
            2,
            &format!(
                "{READ},{}",
                READ_IO.replace("\"slot\":null", "\"slot\":\"0\"")
            ),
        ),
        "INVALID_VALUE",
    );
    error(
        &doc(
            2,
            &format!("{READ},{}", READ_IO.replace("origin_read", "cache_read")),
        ),
        "INVALID_VALUE",
    );
    error(
        &doc(
            2,
            &format!(
                "{FILL},{FILL_IO},{}",
                CACHE_WRITE.replace("\"source\":\"8\"", "\"source\":null")
            ),
        ),
        "INVALID_VALUE",
    );
}

#[test]
fn cache_source_must_be_prior_origin_read_of_same_fill_not_complete_yet() {
    let good = format!("{FILL},{FILL_IO},{CACHE_WRITE}");
    assert!(parse_trace(&doc(2, &good)).is_ok());
    error(
        &doc(2, &format!("{FILL},{CACHE_WRITE},{FILL_IO}")),
        "INVALID_REFERENCE",
    );
    error(
        &doc(
            2,
            &format!(
                "{READ},{READ_IO},{FILL},{}",
                CACHE_WRITE.replace("\"source\":\"8\"", "\"source\":\"1\"")
            ),
        ),
        "INVALID_REFERENCE",
    );
    error(
        &doc(
            2,
            &format!(
                "{FILL},{FILL_IO},{}",
                CACHE_WRITE.replace("\"source\":\"8\"", "\"source\":\"9\"")
            ),
        ),
        "INVALID_REFERENCE",
    );
}

#[test]
fn commit_publish_persist_and_asserts_check_static_reference_kinds() {
    let prefix = format!("{READ},{READ_IO},{FILL},{FILL_IO},{CACHE_WRITE}");
    for event in [
        r#"{"event":"mutation_commit","request":"1","io":"1"}"#,
        r#"{"event":"read_commit","request":"1","io":"8"}"#,
        r#"{"event":"cache_publish","io":"1"}"#,
        r#"{"event":"origin_persist","io":"1","offset":"0","length":"1"}"#,
        r#"{"event":"assert_read","request":"7","data":"","known":""}"#,
        r#"{"event":"assert_result","request":"7","status":"ok","reported":"0"}"#,
    ] {
        error(&doc(2, &format!("{prefix},{event}")), "INVALID_REFERENCE");
    }
    assert!(parse_trace(&doc(2, &format!(r#"{prefix},{{"event":"read_commit","request":"1","io":"1"}},{{"event":"cache_publish","io":"9"}}"#))).is_ok());
}

#[test]
fn assert_data_may_be_short_but_not_longer_than_request() {
    assert!(
        parse_trace(&doc(
            2,
            &format!(r#"{READ},{{"event":"assert_read","request":"1","data":"00","known":"1"}}"#)
        ))
        .is_ok()
    );
    error(
        &doc(
            2,
            &format!(
                r#"{READ},{{"event":"assert_result","request":"1","status":"ok","reported":"3"}}"#
            ),
        ),
        "INVALID_VALUE",
    );
    error(
        &doc(
            2,
            &format!(
                r#"{},{{"event":"assert_read","request":"1","data":"0000","known":"11"}}"#,
                READ.replace("\"length\":\"2\"", "\"length\":\"1\"")
            ),
        ),
        "INVALID_VALUE",
    );
}

#[test]
fn assert_index_must_point_to_prior_non_assert_event() {
    let a = r#"{"event":"assert_decision","index":"0","decision":"accepted"}"#;
    assert!(parse_trace(&doc(2, &format!("{READ},{a}"))).is_ok());
    error(&doc(2, a), "INVALID_REFERENCE");
    error(
        &doc(
            2,
            &format!("{READ},{}", a.replace("\"index\":\"0\"", "\"index\":\"1\"")),
        ),
        "INVALID_REFERENCE",
    );
    error(
        &doc(
            2,
            &format!(
                "{READ},{a},{}",
                a.replace("\"index\":\"0\"", "\"index\":\"1\"")
            ),
        ),
        "INVALID_REFERENCE",
    );
}

#[test]
fn phase_violations_are_parser_success_not_mutant_detection() {
    let complete = r#"{"event":"io_complete","io":"1","status":"ok","reported":"2"}"#;
    let transfer = r#"{"event":"io_transfer","io":"1","offset":"0","length":"2"}"#;
    let retry = READ_IO.replace("\"io\":\"1\"", "\"io\":\"2\"");
    for events in [
        format!("{READ},{READ_IO},{complete},{complete}"),
        format!("{READ},{READ_IO},{complete},{transfer}"),
        format!("{READ},{READ_IO},{transfer},{transfer}"),
        format!("{READ},{READ_IO},{retry}"),
        format!(r#"{{"event":"power_loss"}},{READ}"#),
        format!(
            r#"{READ},{{"event":"retire_request","request":"1"}},{{"event":"cancel_request","request":"1"}}"#
        ),
    ] {
        assert!(parse_trace(&doc(2, &events)).is_ok());
    }
}

#[test]
fn cross_line_cache_read_and_unbound_slot_are_runtime_decisions() {
    let read = READ.replace("\"offset\":\"0\"", "\"offset\":\"4095\"");
    let io = READ_IO
        .replace("origin_read", "cache_read")
        .replace("\"slot\":null", "\"slot\":\"0\"");
    assert!(parse_trace(&doc(4097, &format!("{read},{io}"))).is_ok());
}

#[test]
fn flush_count_and_transfer_shape_and_persist_are_static_only() {
    let prefix = r#"{"event":"flush_submit","request":"1"},{"event":"io_submit","io":"1","request":"1","target":"origin_flush","slot":null,"source":null}"#;
    assert!(parse_trace(&doc(0, &format!(r#"{prefix},{{"event":"io_complete","io":"1","status":"ok","reported":"0"}},{{"event":"mutation_commit","request":"1","io":"1"}}"#))).is_ok());
    error(
        &doc(
            2,
            &format!(r#"{prefix},{{"event":"io_complete","io":"1","status":"ok","reported":"1"}}"#),
        ),
        "INVALID_VALUE",
    );
    error(
        &doc(
            2,
            &format!(r#"{prefix},{{"event":"io_transfer","io":"1","offset":"0","length":"0"}}"#),
        ),
        "INVALID_REFERENCE",
    );
    let write = r#"{"event":"write_submit","request":"1","offset":"1","data":"00","fua":false},{"event":"io_submit","io":"1","request":"1","target":"origin_write","slot":null,"source":null}"#;
    assert!(
        parse_trace(&doc(
            2,
            &format!(r#"{write},{{"event":"origin_persist","io":"1","offset":"0","length":"1"}}"#)
        ))
        .is_ok()
    );
    error(
        &doc(
            2,
            &format!(r#"{write},{{"event":"origin_persist","io":"1","offset":"1","length":"1"}}"#),
        ),
        "INVALID_VALUE",
    );
}

#[test]
fn enum_and_bool_fields_do_not_accept_lookalike_values() {
    error(
        &doc(2, r#"{"event":"epoch_invalidate","reason":"SECRET"}"#),
        "INVALID_SCHEMA",
    );
    error(
        &doc(
            2,
            &format!(
                r#"{READ},{READ_IO},{{"event":"io_complete","io":"1","status":"timeout","reported":"0"}}"#
            ),
        ),
        "INVALID_SCHEMA",
    );
    error(
        &doc(
            2,
            r#"{"event":"write_submit","request":"1","offset":"0","data":"00","fua":1}"#,
        ),
        "INVALID_SCHEMA",
    );
    error(
        &doc(2, r#"{"event":"assert_cache","slot":"0","valid":"true"}"#),
        "INVALID_SCHEMA",
    );
}

#[test]
fn errors_are_fixed_redacted_and_indexed() {
    let t = doc(
        2,
        &format!(r#"{READ},{{"event":"cancel_request","request":"44"}}"#),
    );
    let e = parse_trace(&t).unwrap_err();
    assert_eq!(e.code(), "INVALID_REFERENCE");
    assert_eq!(e.event_index(), Some(1));
    assert_eq!(format!("{e}"), "INVALID_REFERENCE at event 1");
    error(
        &doc(2, &READ.replace("\"1\"", "\"SECRET\"")),
        "INVALID_VALUE",
    );
}
