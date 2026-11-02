use cache_types::status::{
    StatusCounters, StatusError, StatusErrorCode, StatusMode, StatusSnapshot, StatusState,
    encode_status, parse_status,
};

const BASE: &str = include_str!("../../../tests/fixtures/status/disabled.json");

fn disabled() -> StatusSnapshot {
    StatusSnapshot {
        state: StatusState::Disabled,
        mode: StatusMode::ReadCacheWi,
        counters: StatusCounters {
            read_hits: None,
            read_misses: None,
            origin_read_bytes: None,
            cache_written_bytes: None,
        },
        error_code: None,
    }
}

#[test]
fn all_states_roundtrip_and_canonical_output_is_exact() {
    assert_eq!(parse_status(BASE), Ok(disabled()));
    assert_eq!(encode_status(&disabled()), BASE);
    for (wire, state) in [
        ("disabled", StatusState::Disabled),
        ("validating", StatusState::Validating),
        ("shadow", StatusState::Shadow),
        ("active", StatusState::Active),
        ("quiescing", StatusState::Quiescing),
        ("degraded", StatusState::Degraded),
        ("removing", StatusState::Removing),
    ] {
        let input = BASE.replace("disabled", wire);
        let mut expected = disabled();
        expected.state = state;
        assert_eq!(parse_status(&input), Ok(expected.clone()));
        assert_eq!(encode_status(&expected), input);
    }
}

#[test]
fn null_zero_and_maximum_u64_remain_distinct() {
    let input = BASE
        .replace("\"read_hits\":null", "\"read_hits\":\"0\"")
        .replace(
            "\"read_misses\":null",
            "\"read_misses\":\"18446744073709551615\"",
        );
    let parsed = parse_status(&input).unwrap();
    assert_eq!(parsed.counters.read_hits, Some(0));
    assert_eq!(parsed.counters.read_misses, Some(u64::MAX));
    assert_eq!(parsed.counters.origin_read_bytes, None);
    assert_eq!(encode_status(&parsed), input);
}

#[test]
fn missing_nullable_fields_are_not_defaulted_to_null() {
    for fragment in [
        ",\"error_code\":null",
        "\"read_hits\":null,",
        "\"read_misses\":null,",
        "\"origin_read_bytes\":null,",
        ",\"cache_written_bytes\":null",
    ] {
        assert_eq!(
            parse_status(&BASE.replace(fragment, "")),
            Err(StatusError::InvalidSchema)
        );
    }
}

#[test]
fn unknown_and_duplicate_keys_are_rejected_at_each_level() {
    for input in [
        BASE.replacen('{', "{\"secret-SENTINEL\":1,", 1),
        BASE.replace(
            "\"state\":\"disabled\"",
            "\"state\":\"disabled\",\"state\":\"active\"",
        ),
        BASE.replace(
            "\"state\":\"disabled\"",
            "\"state\":\"disabled\",\"st\\u0061te\":\"active\"",
        ),
        BASE.replace(
            "\"read_hits\":null",
            "\"read_hits\":null,\"read_hits\":\"1\"",
        ),
        BASE.replace(
            "\"read_hits\":null",
            "\"secret-SENTINEL\":null,\"read_hits\":null",
        ),
    ] {
        let error = parse_status(&input).unwrap_err();
        assert_eq!(error, StatusError::InvalidSchema);
        assert!(!format!("{error:?} {error}").contains("SENTINEL"));
    }
}

#[test]
fn wrong_version_tokens_and_non_objects_are_rejected() {
    for version in ["0", "2", "1.0", "1e0", "-1", "true", "null", "\"1\""] {
        assert_eq!(
            parse_status(&BASE.replace(
                "\"schema_version\":1",
                &format!("\"schema_version\":{version}")
            )),
            Err(StatusError::InvalidSchema)
        );
    }
    for input in [
        "null",
        "[]",
        "[1,\"disabled\",\"read_cache_wi\",[null,null,null,null],null]",
    ] {
        assert_eq!(parse_status(input), Err(StatusError::InvalidSchema));
    }
    let nested = BASE.replace("{\"read_hits\":null,\"read_misses\":null,\"origin_read_bytes\":null,\"cache_written_bytes\":null}", "[null,null,null,null]");
    assert_eq!(parse_status(&nested), Err(StatusError::InvalidSchema));
}

#[test]
fn numeric_types_and_noncanonical_decimal_strings_are_rejected() {
    for value in ["0", "-1", "1.0", "true", "false", "[]", "{}"] {
        assert_eq!(
            parse_status(&BASE.replace("\"read_hits\":null", &format!("\"read_hits\":{value}"))),
            Err(StatusError::InvalidSchema)
        );
    }
    for value in [
        "",
        "01",
        "+1",
        "-1",
        " 1",
        "1 ",
        "1e3",
        "1.0",
        "１",
        "18446744073709551616",
    ] {
        assert_eq!(
            parse_status(
                &BASE.replace("\"read_hits\":null", &format!("\"read_hits\":\"{value}\""))
            ),
            Err(StatusError::InvalidValue)
        );
    }
}

#[test]
fn only_known_enum_values_and_error_codes_are_accepted() {
    for (wire, code) in [
        ("CACHE_IO", StatusErrorCode::CacheIo),
        ("CACHE_CORRUPT", StatusErrorCode::CacheCorrupt),
        ("TARGET_CHANGED", StatusErrorCode::TargetChanged),
        ("UNSUPPORTED", StatusErrorCode::Unsupported),
        ("RESOURCE_LIMIT", StatusErrorCode::ResourceLimit),
        ("QUIESCE_PENDING", StatusErrorCode::QuiescePending),
    ] {
        let input = BASE.replace("\"error_code\":null", &format!("\"error_code\":\"{wire}\""));
        let parsed = parse_status(&input).unwrap();
        assert_eq!(parsed.error_code, Some(code));
        assert_eq!(encode_status(&parsed), input);
    }
    for input in [
        BASE.replace("disabled", "SENTINEL"),
        BASE.replace("read_cache_wi", "writeback"),
        BASE.replace("\"error_code\":null", "\"error_code\":\"SENTINEL\""),
    ] {
        assert_eq!(parse_status(&input), Err(StatusError::InvalidValue));
    }
}

#[test]
fn syntax_trailing_documents_and_bom_are_rejected() {
    for input in [
        format!("{BASE}{{}}"),
        format!("{BASE}junk"),
        format!("\u{feff}{BASE}"),
        "{".into(),
    ] {
        assert_eq!(parse_status(&input), Err(StatusError::InvalidJson));
    }
    assert_eq!(parse_status(&format!(" \n{BASE}\t\r\n")), Ok(disabled()));
}

#[test]
fn input_limit_is_utf8_bytes_and_checked_before_parsing() {
    let at_limit = format!("{BASE}{}", " ".repeat(65_536 - BASE.len()));
    assert_eq!(parse_status(&at_limit), Ok(disabled()));
    assert_eq!(
        parse_status(&(at_limit + " ")),
        Err(StatusError::InputTooLarge)
    );
    assert_eq!(
        parse_status(&"あ".repeat(21_846)),
        Err(StatusError::InputTooLarge)
    );
}

#[test]
fn public_errors_are_fixed_codes_without_parser_payloads() {
    for (error, code) in [
        (StatusError::InputTooLarge, "INPUT_TOO_LARGE"),
        (StatusError::InvalidJson, "INVALID_JSON"),
        (StatusError::InvalidSchema, "INVALID_SCHEMA"),
        (StatusError::InvalidValue, "INVALID_VALUE"),
    ] {
        assert_eq!(error.code(), code);
        assert_eq!(error.to_string(), code);
    }
}

#[test]
fn valid_but_unrepresentable_numeric_tokens_are_schema_errors() {
    for token in ["1e400", "-1e400", "1e99999999999999999999"] {
        for (key, original) in [
            ("schema_version", "1"),
            ("read_hits", "null"),
            ("error_code", "null"),
            ("state", "\"disabled\""),
            ("mode", "\"read_cache_wi\""),
        ] {
            let input = BASE.replace(
                &format!("\"{key}\":{original}"),
                &format!("\"{key}\":{token}"),
            );
            assert_eq!(
                parse_status(&input),
                Err(StatusError::InvalidSchema),
                "{key}: {token}"
            );
        }
    }
    for token in ["1e+", "1e", "1e-", "1."] {
        let input = BASE.replace(
            "\"schema_version\":1",
            &format!("\"schema_version\":{token}"),
        );
        assert_eq!(parse_status(&input), Err(StatusError::InvalidJson));
    }
}
