use cache_types::{
    diagnostics::{PublicDiagnostics, public_diagnostics},
    status::{StatusError, StatusErrorCode, StatusMode, StatusState},
};
const BASE: &str = include_str!("../../../tests/fixtures/status/disabled.json");
const SECRET: &str = "PRIVATE-SENTINEL-73";

#[test]
fn allowlist_is_a_new_owned_value_not_the_original_payload() {
    let mut input = BASE
        .replace("\"read_hits\":null", "\"read_hits\":\"0\"")
        .replace(
            "\"cache_written_bytes\":null",
            "\"cache_written_bytes\":\"18446744073709551615\"",
        )
        .replace("\"error_code\":null", "\"error_code\":\"CACHE_IO\"");
    let output = public_diagnostics(&input).unwrap();
    input.clear();
    drop(input);
    assert_eq!(
        output,
        PublicDiagnostics {
            state: StatusState::Disabled,
            mode: StatusMode::ReadCacheWi,
            read_hits: Some(0),
            read_misses: None,
            origin_read_bytes: None,
            cache_written_bytes: Some(u64::MAX),
            error_code: Some(StatusErrorCode::CacheIo)
        }
    );
}

#[test]
fn identifiers_and_nested_secrets_are_rejected_without_reflection() {
    for key in ["path", "username", "serial", "content", "diagnostics"] {
        for value in [
            format!("\"{SECRET}\""),
            format!("{{\"nested\":[\"{SECRET}\"]}}"),
        ] {
            for input in [
                BASE.replacen('{', &format!("{{\"{key}\":{value},"), 1),
                BASE.replace(
                    "\"read_hits\":null",
                    &format!("\"{key}\":{value},\"read_hits\":null"),
                ),
            ] {
                let error = public_diagnostics(&input).unwrap_err();
                assert_eq!(error, StatusError::InvalidSchema);
                assert!(!format!("{error:?} {error}").contains(SECRET));
            }
        }
    }
}

#[test]
fn wrong_types_and_secret_values_are_not_stringified_into_output() {
    for input in [
        BASE.replace("disabled", SECRET),
        BASE.replace("read_cache_wi", SECRET),
        BASE.replace("\"read_hits\":null", &format!("\"read_hits\":\"{SECRET}\"")),
        BASE.replace(
            "\"error_code\":null",
            &format!("\"error_code\":{{\"value\":\"{SECRET}\"}}"),
        ),
        BASE.replace(
            "\"read_hits\":null",
            &format!("\"read_hits\":[\"{SECRET}\"]"),
        ),
    ] {
        let error = public_diagnostics(&input).unwrap_err();
        assert!(!format!("{error:?} {error}").contains(SECRET));
    }
}

#[test]
fn strict_size_syntax_and_required_field_guards_are_preserved() {
    assert_eq!(
        public_diagnostics(&"x".repeat(65_537)),
        Err(StatusError::InputTooLarge)
    );
    assert_eq!(
        public_diagnostics(&(BASE.to_owned() + "{}")),
        Err(StatusError::InvalidJson)
    );
    assert_eq!(
        public_diagnostics(&BASE.replace(",\"error_code\":null", "")),
        Err(StatusError::InvalidSchema)
    );
    assert_eq!(
        public_diagnostics(&BASE.replacen('{', "{\"st\\u0061te\":\"active\",", 1)),
        Err(StatusError::InvalidSchema)
    );
    let safe = public_diagnostics(BASE).unwrap();
    assert!(format!("{safe:?}").contains("read_hits: None"));
    assert!(!format!("{safe:?}").contains(SECRET));
}
