use cache_types::benchmark::{
    BenchmarkCacheMode, BenchmarkError, BenchmarkRun, LatencySummary, OsCache, SsdCache,
    StatsError, encode_benchmark, parse_benchmark, summarize_ns,
};

const BASE: &str = include_str!("../../../tests/fixtures/benchmark/base.json");

fn baseline() -> BenchmarkRun {
    BenchmarkRun {
        workload_id: "git-status".into(),
        environment_id: "lab-A".into(),
        toolchain_id: "rust-1.99.1".into(),
        working_set_bytes: 1_048_576,
        os_cache: OsCache::Cold,
        cache_mode: BenchmarkCacheMode::Off,
        ssd_cache: SsdCache::None,
        duration_ns: 1_000_000_000,
        latency_ns: vec![1, 2, 3, 4, 100],
        cpu_time_ns: None,
        peak_nonpaged_bytes: None,
        ssd_written_bytes: None,
        repetitions: 5,
    }
}
fn replace(key: &str, original: &str, replacement: &str) -> String {
    BASE.replacen(
        &format!("\"{key}\":{original}"),
        &format!("\"{key}\":{replacement}"),
        1,
    )
}

#[test]
fn nearest_rank_is_exact_and_input_unchanged() {
    let values = [100, 4, 1, 3, 2];
    assert_eq!(
        summarize_ns(&values),
        Ok(LatencySummary {
            count: 5,
            minimum_ns: 1,
            p50_ns: 3,
            p95_ns: 100,
            p99_ns: 100,
            maximum_ns: 100,
        })
    );
    assert_eq!(values, [100, 4, 1, 3, 2]);
    let hundred: Vec<_> = (1..=100).rev().collect();
    let s = summarize_ns(&hundred).unwrap();
    assert_eq!((s.p50_ns, s.p95_ns, s.p99_ns), (50, 95, 99));
    let two = summarize_ns(&[9, 2]).unwrap();
    assert_eq!((two.p50_ns, two.p95_ns, two.p99_ns), (2, 9, 9));
}
#[test]
fn empty_single_zero_and_maximum_do_not_overflow_or_impute() {
    assert_eq!(summarize_ns(&[]), Err(StatsError::Empty));
    for value in [0, 1, u64::MAX] {
        assert_eq!(
            summarize_ns(&[value]),
            Ok(LatencySummary {
                count: 1,
                minimum_ns: value,
                p50_ns: value,
                p95_ns: value,
                p99_ns: value,
                maximum_ns: value
            })
        );
    }
}
#[test]
fn golden_codec_is_canonical_and_roundtrips_every_treatment() {
    assert_eq!(parse_benchmark(BASE), Ok(baseline()));
    assert_eq!(encode_benchmark(&baseline()), Ok(BASE.into()));
    for os_cache in [OsCache::Cold, OsCache::Warm, OsCache::Unknown] {
        for (mode, ssd) in [
            (BenchmarkCacheMode::Off, SsdCache::None),
            (BenchmarkCacheMode::Shadow, SsdCache::Cold),
            (BenchmarkCacheMode::Shadow, SsdCache::Warm),
            (BenchmarkCacheMode::Active, SsdCache::Cold),
            (BenchmarkCacheMode::Active, SsdCache::Warm),
        ] {
            let mut run = baseline();
            run.os_cache = os_cache;
            run.cache_mode = mode;
            run.ssd_cache = ssd;
            assert_eq!(parse_benchmark(&encode_benchmark(&run).unwrap()), Ok(run));
        }
    }
}
#[test]
fn null_zero_max_and_sample_count_are_distinct() {
    let mut run = baseline();
    run.latency_ns = vec![0, u64::MAX];
    run.cpu_time_ns = Some(0);
    run.peak_nonpaged_bytes = Some(u64::MAX);
    run.duration_ns = u64::MAX;
    run.working_set_bytes = u64::MAX;
    let text = encode_benchmark(&run).unwrap();
    assert!(text.contains("\"cpu_time_ns\":\"0\",\"peak_nonpaged_bytes\":\"18446744073709551615\",\"ssd_written_bytes\":null"));
    assert_eq!(parse_benchmark(&text), Ok(run));
}
#[test]
fn missing_nullable_duplicate_and_unknown_fields_are_schema_errors() {
    for key in ["cpu_time_ns", "peak_nonpaged_bytes", "ssd_written_bytes"] {
        assert_eq!(
            parse_benchmark(&BASE.replace(&format!(",\"{key}\":null"), "")),
            Err(BenchmarkError::InvalidSchema)
        );
    }
    for input in [
        BASE.replacen('{', "{\"SENTINEL\":1,", 1),
        BASE.replacen('{', "{\"schema_version\":1,", 1),
        BASE.replacen('{', "{\"sch\\u0065ma_version\":1,", 1),
    ] {
        assert_eq!(parse_benchmark(&input), Err(BenchmarkError::InvalidSchema));
    }
}
#[test]
fn objects_types_and_integer_tokens_are_strict() {
    for token in ["null", "[]", "1", "true", "\"SENTINEL\""] {
        assert_eq!(parse_benchmark(token), Err(BenchmarkError::InvalidSchema));
    }
    for token in ["0", "2", "1.0", "1e0", "-1", "true", "null", "\"1\""] {
        assert_eq!(
            parse_benchmark(&replace("schema_version", "1", token)),
            Err(BenchmarkError::InvalidSchema)
        );
    }
    for token in ["1.0", "1e0", "-1", "true", "null", "\"1\""] {
        assert_eq!(
            parse_benchmark(&replace("repetitions", "5", token)),
            Err(BenchmarkError::InvalidSchema)
        );
    }
    for token in ["1", "-1", "1.0", "true", "{}", "[]", "null"] {
        assert_eq!(
            parse_benchmark(&replace("duration_ns", "\"1000000000\"", token)),
            Err(BenchmarkError::InvalidSchema)
        );
    }
    assert_eq!(
        parse_benchmark(&replace(
            "latency_ns",
            "[\"1\",\"2\",\"3\",\"4\",\"100\"]",
            "[1]"
        )),
        Err(BenchmarkError::InvalidSchema)
    );
}
#[test]
fn noncanonical_numbers_bounds_and_enum_spellings_are_values_errors() {
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
        "0",
    ] {
        assert_eq!(
            parse_benchmark(&replace(
                "duration_ns",
                "\"1000000000\"",
                &format!("\"{value}\"")
            )),
            Err(BenchmarkError::InvalidValue)
        );
    }
    for token in ["0", "1001", "65536"] {
        assert_eq!(
            parse_benchmark(&replace("repetitions", "5", token)),
            Err(BenchmarkError::InvalidValue)
        );
    }
    for (key, old, new) in [
        ("os_cache", "\"cold\"", "\"hot\""),
        ("cache_mode", "\"off\"", "\"writeback\""),
        ("ssd_cache", "\"none\"", "\"hot\""),
        ("cpu_time_ns", "null", "\"01\""),
        ("working_set_bytes", "\"1048576\"", "\"0\""),
    ] {
        assert_eq!(
            parse_benchmark(&replace(key, old, new)),
            Err(BenchmarkError::InvalidValue)
        );
    }
}
#[test]
fn identity_format_and_length_are_bounded() {
    for id in [
        "".to_owned(),
        "x".repeat(65),
        "a b".into(),
        "../SENTINEL/".into(),
        "あ".into(),
        "x@y".into(),
    ] {
        assert_eq!(
            parse_benchmark(&replace(
                "workload_id",
                "\"git-status\"",
                &format!("\"{id}\"")
            )),
            Err(BenchmarkError::InvalidValue)
        );
    }
    let mut run = baseline();
    run.workload_id = "A".repeat(64);
    run.environment_id = "z_9.-".into();
    assert_eq!(parse_benchmark(&encode_benchmark(&run).unwrap()), Ok(run));
}
#[test]
fn mode_pairs_and_sample_limits_are_enforced_both_directions() {
    assert_eq!(
        parse_benchmark(&replace("ssd_cache", "\"none\"", "\"warm\"")),
        Err(BenchmarkError::InvalidValue)
    );
    assert_eq!(
        parse_benchmark(&replace("cache_mode", "\"off\"", "\"active\"")),
        Err(BenchmarkError::InvalidValue)
    );
    assert_eq!(
        parse_benchmark(&replace(
            "latency_ns",
            "[\"1\",\"2\",\"3\",\"4\",\"100\"]",
            "[]"
        )),
        Err(BenchmarkError::InvalidValue)
    );
    let samples = format!("[{}]", vec!["\"0\""; 100_000].join(","));
    let input = replace("latency_ns", "[\"1\",\"2\",\"3\",\"4\",\"100\"]", &samples);
    let mut run = parse_benchmark(&input).unwrap();
    assert_eq!(run.latency_ns.len(), 100_000);
    run.latency_ns.push(0);
    assert_eq!(encode_benchmark(&run), Err(BenchmarkError::InvalidValue));
    assert_eq!(
        parse_benchmark(&input.replace("[\"0\"", "[\"0\",\"0\"")),
        Err(BenchmarkError::InvalidValue)
    );
}
#[test]
fn encoder_revalidates_manually_constructed_public_fields() {
    let mutations: [fn(&mut BenchmarkRun); 9] = [
        |r| r.workload_id.clear(),
        |r| r.environment_id = "secret/name".into(),
        |r| r.toolchain_id = "é".into(),
        |r| r.working_set_bytes = 0,
        |r| r.duration_ns = 0,
        |r| r.latency_ns.clear(),
        |r| r.repetitions = 0,
        |r| r.repetitions = 1001,
        |r| r.ssd_cache = SsdCache::Warm,
    ];
    for mutate in mutations {
        let mut run = baseline();
        mutate(&mut run);
        assert_eq!(encode_benchmark(&run), Err(BenchmarkError::InvalidValue));
    }
}
#[test]
fn syntax_and_byte_limit_have_fixed_errors() {
    for input in [
        format!("{BASE}{{}}"),
        format!("\u{feff}{BASE}"),
        "{".into(),
        format!("{BASE}junk"),
    ] {
        assert_eq!(parse_benchmark(&input), Err(BenchmarkError::InvalidJson));
    }
    let at_limit = format!("{BASE}{}", " ".repeat(16_777_216 - BASE.len()));
    assert_eq!(parse_benchmark(&at_limit), Ok(baseline()));
    assert_eq!(
        parse_benchmark(&(at_limit + " ")),
        Err(BenchmarkError::InputTooLarge)
    );
    assert_eq!(
        parse_benchmark(&"あ".repeat(5_592_406)),
        Err(BenchmarkError::InputTooLarge)
    );
}
#[test]
fn errors_do_not_reflect_input() {
    for (error, code) in [
        (BenchmarkError::InputTooLarge, "INPUT_TOO_LARGE"),
        (BenchmarkError::InvalidJson, "INVALID_JSON"),
        (BenchmarkError::InvalidSchema, "INVALID_SCHEMA"),
        (BenchmarkError::InvalidValue, "INVALID_VALUE"),
    ] {
        assert_eq!(error.code(), code);
        assert_eq!(error.to_string(), code);
    }
    let error = parse_benchmark(&BASE.replacen('{', "{\"TOP-SECRET-SENTINEL\":1,", 1)).unwrap_err();
    assert!(!format!("{error:?} {error}").contains("SENTINEL"));
}

#[test]
fn huge_numeric_tokens_do_not_change_schema_errors_into_syntax_errors() {
    for token in ["1e400", "-1e400", "1e99999999999999999999"] {
        for (key, original) in [
            ("schema_version", "1"),
            ("repetitions", "5"),
            ("cpu_time_ns", "null"),
            ("duration_ns", "\"1000000000\""),
        ] {
            assert_eq!(
                parse_benchmark(&replace(key, original, token)),
                Err(BenchmarkError::InvalidSchema)
            );
        }
    }
}
