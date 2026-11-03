mod common;
use cache_probe_core::{CaptureBuilder, CollectorFault, Command, audit_archive, decode_archive};
use common::*;
use serde_json::json;
use std::num::NonZeroU64;

#[test]
fn read_pair_report_literal() {
    assert_eq!(
        audit_archive(&read_pair()).unwrap().to_json(),
        include_str!("../../../tests/fixtures/capture-probe-v1/report-read-pair.json")
    );
    let parsed = decode_archive(&read_pair()).unwrap();
    assert_eq!(parsed.frames().len(), 3);
    assert_eq!(parsed.record_count(), 2);
}

#[test]
fn prefix_only_is_unfinished() {
    let value = json_report(&read_pair()[..64]);
    assert_eq!(value["terminal_received"], false);
    assert_eq!(value["retained_records_reconciled"], false);
    assert_eq!(
        value["reasons"],
        json!([
            "EMPTY_CAPTURE",
            "MISSING_TERMINAL",
            "UNKNOWN_COUNTERS",
            "UNVERIFIED_BOUNDARIES",
            "UNVERIFIED_PROVENANCE"
        ])
    );
}

#[test]
fn frame_boundary_without_footer() {
    let value = json_report(&read_pair()[..FOOTER]);
    assert_eq!(
        value["reasons"],
        json!([
            "MISSING_TERMINAL",
            "UNKNOWN_COUNTERS",
            "UNVERIFIED_BOUNDARIES",
            "UNVERIFIED_PROVENANCE"
        ])
    );
    assert_eq!(value["terminal_received"], false);
    assert_eq!(value["record_count"], "2");
}

#[test]
fn lost_first_drain_offline_adversarial() {
    let bytes = hex(include_str!(
        "../../../tests/fixtures/capture-probe-v1/lost-drain.hex"
    ));
    let value = json_report(&bytes);
    assert_eq!(
        value["reasons"],
        json!([
            "COLLECTOR_FAULT",
            "CALL_SEQUENCE_GAP",
            "SEQUENCE_GAP",
            "COUNT_MISMATCH",
            "UNVERIFIED_BOUNDARIES",
            "UNVERIFIED_PROVENANCE"
        ])
    );
    assert_eq!(value["record_count"], "2");
    assert_eq!(value["terminal_received"], true);
    assert_eq!(value["retained_records_reconciled"], false);
}

#[test]
fn footer_cannot_hide_derived_faults() {
    let mut bad = read_pair();
    set64(&mut bad, STOP + 48, 4);
    assert_eq!(
        json_report(&bad)["reasons"],
        json!([
            "COLLECTOR_FAULT",
            "CALL_SEQUENCE_GAP",
            "UNVERIFIED_BOUNDARIES",
            "UNVERIFIED_PROVENANCE"
        ])
    );
    set32(&mut bad, FOOTER + 24, 16);
    assert_eq!(
        json_report(&bad)["reasons"],
        json!([
            "COLLECTOR_FAULT",
            "CALL_SEQUENCE_GAP",
            "UNVERIFIED_BOUNDARIES",
            "UNVERIFIED_PROVENANCE"
        ])
    );
    set32(&mut bad, FOOTER + 28, 0);
    assert_eq!(json_report(&bad)["terminal_received"], false);
    assert!(
        json_report(&bad)["reasons"]
            .as_array()
            .unwrap()
            .contains(&json!("MISSING_TERMINAL"))
    );
    // Omission cannot heal a derived fault either.
    let missing = json_report(&bad[..FOOTER]);
    assert!(
        missing["reasons"]
            .as_array()
            .unwrap()
            .contains(&json!("COLLECTOR_FAULT"))
    );
}

#[test]
fn second_begin_is_history_not_parser_error() {
    let mut frames = frames();
    let mut second = frames[0].clone();
    set64(&mut second, 48, 2);
    set64(&mut frames[1], 48, 3);
    set64(&mut frames[2], 48, 4);
    frames.insert(1, second);
    let bytes = archive(&frames, true);
    assert_eq!(decode_archive(&bytes).unwrap().frames().len(), 4);
    assert_eq!(
        json_report(&bytes)["reasons"],
        json!([
            "COLLECTOR_FAULT",
            "UNVERIFIED_BOUNDARIES",
            "UNVERIFIED_PROVENANCE"
        ])
    );
}

#[test]
fn regression_resurrection_and_call_replay() {
    for (offset, value) in [(STOP + 48, 2u64), (STOP + 80, 1), (STOP + 96, 1)] {
        let mut bad = read_pair();
        set64(&mut bad, offset, value);
        assert!(
            json_report(&bad)["reasons"]
                .as_array()
                .unwrap()
                .contains(&json!("COLLECTOR_FAULT"))
        );
    }
    let mut bad = read_pair();
    set32(&mut bad, DRAIN + 72, 6);
    set32(&mut bad, DRAIN + 76, 4);
    set64(&mut bad, DRAIN + 80, 0);
    // Later STOP resurrects known bits and clears sticky exhaustion.
    assert_eq!(
        json_report(&bad)["reasons"],
        json!([
            "COLLECTOR_FAULT",
            "PRODUCER_FAULT",
            "UNKNOWN_COUNTERS",
            "UNVERIFIED_BOUNDARIES",
            "UNVERIFIED_PROVENANCE"
        ])
    );
    let mut bad = read_pair();
    set32(&mut bad, STOP + 72, 6);
    set64(&mut bad, STOP + 80, 0); // dropping known without exhaustion is a history fault
    assert_eq!(
        json_report(&bad)["reasons"],
        json!([
            "COLLECTOR_FAULT",
            "UNKNOWN_COUNTERS",
            "UNVERIFIED_BOUNDARIES",
            "UNVERIFIED_PROVENANCE"
        ])
    );
}

#[test]
fn counter_identity_and_popped_delta() {
    let mut bad = read_pair();
    set64(&mut bad, DRAIN + 96, 1);
    assert_eq!(
        json_report(&bad)["reasons"],
        json!([
            "COLLECTOR_FAULT",
            "COUNT_MISMATCH",
            "UNVERIFIED_BOUNDARIES",
            "UNVERIFIED_PROVENANCE"
        ])
    );
    let mut bad = read_pair();
    set32(&mut bad, FOOTER + 20, 1);
    assert_eq!(
        json_report(&bad)["reasons"],
        json!([
            "MISSING_TERMINAL",
            "COLLECTOR_FAULT",
            "UNKNOWN_COUNTERS",
            "COUNT_MISMATCH",
            "UNVERIFIED_BOUNDARIES",
            "UNVERIFIED_PROVENANCE"
        ])
    );
    assert_eq!(json_report(&bad)["terminal_received"], false);
}

#[test]
fn duplicate_and_orphan_correlation() {
    let mut f = frames();
    let record = f[1][240..336].to_vec();
    f[1].extend(record);
    set32(&mut f[1], 16, 432);
    set32(&mut f[1], 24, 3);
    set64(&mut f[1], 336, 2);
    for frame in &mut f[1..] {
        set64(frame, 80, 3);
        set64(frame, 96, 3);
    }
    let b = archive(&f, true);
    assert_eq!(decode_archive(&b).unwrap().record_count(), 3);
    assert_eq!(
        json_report(&b)["reasons"],
        json!([
            "COLLECTOR_FAULT",
            "DUPLICATE_REQUEST",
            "UNVERIFIED_BOUNDARIES",
            "UNVERIFIED_PROVENANCE"
        ])
    );

    let mut orphan = read_pair();
    set64(&mut orphan, COMPLETE + 8, 2);
    assert_eq!(
        json_report(&orphan)["reasons"],
        json!([
            "UNPAIRED_REQUEST",
            "UNVERIFIED_BOUNDARIES",
            "UNVERIFIED_PROVENANCE"
        ])
    );
    assert_eq!(json_report(&orphan)["retained_records_reconciled"], false);
}

#[test]
fn known_drop_gap_is_loss_not_invented_collector_fault() {
    let mut b = read_pair();
    set64(&mut b, COMPLETE, 2);
    for at in [DRAIN, STOP] {
        set64(&mut b, at + 80, 3);
        set64(&mut b, at + 88, 1);
    }
    assert_eq!(
        json_report(&b)["reasons"],
        json!([
            "DROPPED_EVENTS",
            "SEQUENCE_GAP",
            "UNVERIFIED_BOUNDARIES",
            "UNVERIFIED_PROVENANCE"
        ])
    );
}

#[test]
fn classification_state_and_first_baseline_are_history_faults() {
    let mut b = flush_pair();
    set32(&mut b, COMPLETE + 68, 1);
    assert_eq!(
        json_report(&b)["reasons"],
        json!([
            "COLLECTOR_FAULT",
            "UNVERIFIED_BOUNDARIES",
            "UNVERIFIED_PROVENANCE"
        ])
    );
    for (at, value) in [
        (BEGIN + 60, 0),
        (BEGIN + 64, 1),
        (STOP + 56, 1),
        (STOP + 64, 1),
    ] {
        let mut b = read_pair();
        set32(&mut b, at, value);
        assert!(
            json_report(&b)["reasons"]
                .as_array()
                .unwrap()
                .contains(&json!("COLLECTOR_FAULT"))
        );
    }
    let mut b = read_pair();
    set32(&mut b, STOP + 28, 2); // No acknowledged STOP.
    assert_eq!(json_report(&b)["terminal_received"], false);
}

#[test]
fn native_and_projection_limits() {
    assert_eq!(
        audit_archive(&vec![0; 1_048_576]).unwrap_err().code(),
        "INVALID_PROBE_ARCHIVE"
    );
    assert_eq!(
        audit_archive(&vec![0; 1_048_577]).unwrap_err().code(),
        "PROBE_TOO_LARGE"
    );
    let mut f = vec![frames()[0].clone(); 4096];
    assert_eq!(
        decode_archive(&archive(&f, false)).unwrap().frames().len(),
        4096
    );
    f.push(frames()[0].clone());
    assert_eq!(
        decode_archive(&archive(&f, false)).unwrap_err().code(),
        "PROBE_TOO_LARGE"
    );
    let mut sixteen = frames()[1].clone();
    let pair = sixteen[144..].to_vec();
    for _ in 1..8 {
        sixteen.extend(&pair);
    }
    set32(&mut sixteen, 16, 1680);
    set32(&mut sixteen, 24, 16);
    let mut f = vec![sixteen; 256];
    assert_eq!(
        decode_archive(&archive(&f, false)).unwrap().record_count(),
        4096
    );
    f.push(frames()[1].clone());
    assert_eq!(
        decode_archive(&archive(&f, false)).unwrap_err().code(),
        "PROBE_TOO_LARGE"
    );
}

#[test]
fn malformed_precedence() {
    for offset in [
        0,
        8,
        12,
        16,
        20,
        24,
        28,
        32,
        40,
        48,
        52,
        56,
        60,
        DRAIN + 40,
        FOOTER + 8,
        FOOTER + 12,
        FOOTER + 32,
        FOOTER + 40,
        FOOTER + 52,
        FOOTER + 56,
    ] {
        let mut b = read_pair();
        b[offset] ^= 0x80;
        assert_eq!(
            decode_archive(&b).unwrap_err().code(),
            "INVALID_PROBE_ARCHIVE",
            "offset {offset}"
        );
    }
    for length in [0, 1, 63, 65, 207, 209, 543, 545, 689, 751] {
        assert_eq!(
            decode_archive(&read_pair()[..length]).unwrap_err().code(),
            "INVALID_PROBE_ARCHIVE"
        );
    }
    let mut extra = archive(&vec![frames()[0].clone(); 4096], false);
    extra.extend_from_slice(b"WCHP04RS");
    assert_eq!(
        decode_archive(&extra).unwrap_err().code(),
        "INVALID_PROBE_ARCHIVE"
    );
    let mut b = read_pair();
    b.push(0);
    assert!(decode_archive(&b).is_err());
}

#[test]
fn builder_failed_drain_allows_only_stop() {
    let mut builder = CaptureBuilder::new(NonZeroU64::new(7).unwrap());
    let begin = builder.next_request(Command::Begin, 0).unwrap();
    assert_eq!(begin.call_id(), 1);
    assert!(builder.next_request(Command::Drain, 16).is_err()); // outstanding BEGIN
    builder.receive(begin, &read_pair()[BEGIN..DRAIN]).unwrap();
    let drain = builder.next_request(Command::Drain, 16).unwrap();
    builder.fail(drain, CollectorFault::TransportError).unwrap();
    assert!(!builder.normal_draining_allowed());
    assert!(builder.next_request(Command::Drain, 16).is_err());
    let stop = builder.next_request(Command::Stop, 0).unwrap();
    assert_eq!(stop.call_id(), 3);
    builder.fail(stop, CollectorFault::TransportError).unwrap();
    assert!(builder.next_request(Command::Stop, 0).is_err());
    let result = json_report(&builder.finish().unwrap());
    assert_eq!(result["frame_count"], "1");
    assert_eq!(
        result["reasons"],
        json!([
            "EMPTY_CAPTURE",
            "MISSING_TERMINAL",
            "COLLECTOR_FAULT",
            "UNKNOWN_COUNTERS",
            "UNVERIFIED_BOUNDARIES",
            "UNVERIFIED_PROVENANCE"
        ])
    );
}

#[test]
fn builder_round_trip_preserves_exact_native_bytes() {
    let literal = read_pair();
    let mut builder = CaptureBuilder::new(NonZeroU64::new(7).unwrap());
    for (command, max, start, end) in [
        (Command::Begin, 0, BEGIN, DRAIN),
        (Command::Drain, 16, DRAIN, STOP),
        (Command::Stop, 0, STOP, FOOTER),
    ] {
        let request = builder.next_request(command, max).unwrap();
        builder.receive(request, &literal[start..end]).unwrap();
    }
    assert!(builder.terminal_received());
    assert_eq!(builder.finish().unwrap(), literal);
}

#[test]
fn false_terminal_footer_cannot_hide_observed_terminal() {
    let mut b = read_pair();
    set32(&mut b, FOOTER + 48, 0);
    set32(&mut b, FOOTER + 28, u32::MAX);
    assert_eq!(
        json_report(&b)["reasons"],
        json!([
            "MISSING_TERMINAL",
            "COLLECTOR_FAULT",
            "UNKNOWN_COUNTERS",
            "UNVERIFIED_BOUNDARIES",
            "UNVERIFIED_PROVENANCE"
        ])
    );
}

#[test]
fn failed_normal_stop_permits_one_fresh_cleanup_stop() {
    let mut builder = CaptureBuilder::new(NonZeroU64::new(7).unwrap());
    let begin = builder.next_request(Command::Begin, 0).unwrap();
    builder.receive(begin, &read_pair()[BEGIN..DRAIN]).unwrap();
    let stop = builder.next_request(Command::Stop, 0).unwrap();
    builder.fail(stop, CollectorFault::TransportError).unwrap();
    let cleanup = builder.next_request(Command::Stop, 0).unwrap();
    assert_eq!(cleanup.call_id(), 3);
    builder
        .fail(cleanup, CollectorFault::TransportError)
        .unwrap();
    assert!(builder.next_request(Command::Stop, 0).is_err());
}
