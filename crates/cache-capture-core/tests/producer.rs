//! Public, hand-derived producer schedules. No injected private counters/IDs.
use cache_capture_core::{
    CaptureState, CompleteError, CompletionSnapshot, Observation, Operation, Producer, Record,
    SealError, Stats, SubmitError, SubmitSnapshot,
};
use std::num::NonZeroU64;

const READ: SubmitSnapshot = SubmitSnapshot {
    version: 1,
    major: 0x03,
    minor: 0,
    flags: 0xa5,
    control_known: 0,
    range_known: 1,
    control_code: 0,
    reserved: 0,
    offset: 4_096,
    length: 2,
};
const WRITE: SubmitSnapshot = SubmitSnapshot {
    version: 1,
    major: 0x04,
    minor: 0,
    flags: 0x5a,
    control_known: 0,
    range_known: 1,
    control_code: 0,
    reserved: 0,
    offset: 8_192,
    length: 4,
};
const OK: CompletionSnapshot = CompletionSnapshot {
    version: 1,
    status: 0,
    information_known: 1,
    reserved: 0,
    information: 2,
};
const UNKNOWN: CompletionSnapshot = CompletionSnapshot {
    version: 1,
    status: 0,
    information_known: 0,
    reserved: 0,
    information: 0,
};

#[derive(Clone, Copy)]
struct ExpectedSubmission {
    major: u8,
    minor: u8,
    flags: u8,
    operation: Operation,
    range: Option<(u64, u64)>,
    control_code: Option<u32>,
}
const READ_EXPECTED: ExpectedSubmission = ExpectedSubmission {
    major: 0x03,
    minor: 0,
    flags: 0xa5,
    operation: Operation::Read,
    range: Some((4_096, 2)),
    control_code: None,
};
const WRITE_EXPECTED: ExpectedSubmission = ExpectedSubmission {
    major: 0x04,
    minor: 0,
    flags: 0x5a,
    operation: Operation::Write,
    range: Some((8_192, 4)),
    control_code: None,
};

fn producer(session: u64, target: u64) -> Producer {
    Producer::new(
        NonZeroU64::new(session).unwrap(),
        NonZeroU64::new(target).unwrap(),
    )
}

fn stats(active: usize, queued: usize, attempted: u64, dropped: u64) -> Stats {
    Stats {
        state: CaptureState::Running,
        active_contexts: active,
        queued_records: queued,
        attempted: Some(attempted),
        dropped: Some(dropped),
        tracking_lost: false,
        protocol_fault: false,
        counter_exhausted: false,
        identity_exhausted: false,
    }
}

fn assert_ids(record: &Record, expected: [u64; 4]) {
    assert_eq!(
        [
            record.seq,
            record.session_id,
            record.target_id,
            record.request_id
        ],
        expected
    );
}

fn assert_submit(record: Record, ids: [u64; 4], expected: ExpectedSubmission) {
    assert_ids(&record, ids);
    let Observation::Submit(submission) = record.observation else {
        panic!("expected submission, got {:?}", record.observation);
    };
    assert_eq!(submission.major(), expected.major);
    assert_eq!(submission.minor(), expected.minor);
    assert_eq!(submission.flags(), expected.flags);
    assert_eq!(submission.operation(), expected.operation);
    assert_eq!(
        submission
            .range()
            .map(|range| (range.offset(), range.length())),
        expected.range
    );
    assert_eq!(submission.control_code(), expected.control_code);
}

fn assert_complete(record: Record, ids: [u64; 4], status: u32, information: Option<u64>) {
    assert_ids(&record, ids);
    let Observation::Complete(completion) = record.observation else {
        panic!("expected completion, got {:?}", record.observation);
    };
    assert_eq!(completion.status, status);
    assert_eq!(completion.information, information);
}

#[test]
fn empty_running_producer_cannot_seal_and_stop_seal_are_idempotent() {
    let mut p = producer(11, 22);
    assert_eq!(p.stats(), stats(0, 0, 0, 0));
    assert_eq!(p.pop(), None);
    assert_eq!(p.seal(), Err(SealError::StillRunning));
    assert_eq!(p.stats(), stats(0, 0, 0, 0));
    p.stop();
    let stopped = Stats {
        state: CaptureState::Stopping,
        ..stats(0, 0, 0, 0)
    };
    assert_eq!(p.stats(), stopped);
    p.stop();
    assert_eq!(p.stats(), stopped);
    let sealed = Stats {
        state: CaptureState::Sealed,
        ..stats(0, 0, 0, 0)
    };
    assert_eq!(p.seal(), Ok(sealed));
    assert_eq!(p.seal(), Ok(sealed));
    p.stop();
    assert_eq!(p.pop(), None);
    assert_eq!(p.stats(), sealed);
}

#[test]
fn two_requests_complete_in_reverse_order_with_saved_fields_and_exact_identity() {
    let mut p = producer(11, 22);
    let first = p.submit(READ).unwrap();
    let second = p.submit(WRITE).unwrap();
    assert_eq!(
        [first.session_id(), first.target_id(), first.request_id()],
        [11, 22, 1]
    );
    assert_eq!(
        [second.session_id(), second.target_id(), second.request_id()],
        [11, 22, 2]
    );
    assert_ne!(first, second);
    assert_eq!(p.stats(), stats(2, 2, 2, 0));
    assert_eq!(
        p.complete(
            second,
            CompletionSnapshot {
                information: 7,
                ..OK
            }
        ),
        Ok(())
    );
    assert_eq!(p.stats(), stats(1, 3, 3, 0));
    assert_eq!(
        p.complete(
            first,
            CompletionSnapshot {
                information: 1,
                ..OK
            }
        ),
        Ok(())
    );
    assert_eq!(p.stats(), stats(0, 4, 4, 0));
    assert_submit(p.pop().unwrap(), [0, 11, 22, 1], READ_EXPECTED);
    assert_submit(p.pop().unwrap(), [1, 11, 22, 2], WRITE_EXPECTED);
    assert_complete(p.pop().unwrap(), [2, 11, 22, 2], 0, Some(7));
    assert_complete(p.pop().unwrap(), [3, 11, 22, 1], 0, Some(1));
    assert_eq!(p.pop(), None);
    assert_eq!(p.stats(), stats(0, 0, 4, 0));
}

#[test]
fn old_ticket_cannot_retire_a_new_context_after_slot_reuse() {
    let mut p = producer(11, 22);
    let old = p.submit(READ).unwrap();
    p.complete(old, OK).unwrap();
    let current = p.submit(WRITE).unwrap();
    assert_eq!(current.request_id(), 2);
    assert_eq!(p.complete(old, OK), Err(CompleteError::InvalidTicket));
    assert_eq!(
        p.stats(),
        Stats {
            tracking_lost: true,
            protocol_fault: true,
            ..stats(1, 3, 3, 0)
        }
    );
    p.complete(
        current,
        CompletionSnapshot {
            information: 4,
            ..OK
        },
    )
    .unwrap();
    assert_eq!(p.complete(current, OK), Err(CompleteError::InvalidTicket));
    assert_eq!(
        p.stats(),
        Stats {
            tracking_lost: true,
            protocol_fault: true,
            ..stats(0, 4, 4, 0)
        }
    );
    assert_submit(p.pop().unwrap(), [0, 11, 22, 1], READ_EXPECTED);
    assert_complete(p.pop().unwrap(), [1, 11, 22, 1], 0, Some(2));
    assert_submit(p.pop().unwrap(), [2, 11, 22, 2], WRITE_EXPECTED);
    assert_complete(p.pop().unwrap(), [3, 11, 22, 2], 0, Some(4));
    assert_eq!(p.pop(), None);
}

#[test]
fn full_ring_drops_newest_without_revoking_contexts_or_claiming_tracking_loss() {
    let mut p = producer(11, 22);
    for id in 1..=130 {
        let ticket = p.submit(READ).unwrap();
        assert_eq!(ticket.request_id(), id);
        p.complete(ticket, OK).unwrap();
    }
    assert_eq!(p.stats(), stats(0, 256, 260, 4));
    for id in 1..=128 {
        assert_submit(p.pop().unwrap(), [2 * (id - 1), 11, 22, id], READ_EXPECTED);
        assert_complete(p.pop().unwrap(), [2 * id - 1, 11, 22, id], 0, Some(2));
    }
    assert_eq!(p.pop(), None);
    assert_eq!(p.stats(), stats(0, 0, 260, 4));
    p.stop();
    assert_eq!(
        p.seal(),
        Ok(Stats {
            state: CaptureState::Sealed,
            ..stats(0, 0, 260, 4)
        })
    );
}

#[test]
fn context_exhaustion_has_an_ordinal_gap_and_sticky_tracking_loss_after_drain() {
    let mut p = producer(11, 22);
    let tickets: Vec<_> = (1..=64)
        .map(|id| {
            let ticket = p.submit(READ).unwrap();
            assert_eq!(ticket.request_id(), id);
            ticket
        })
        .collect();
    assert_eq!(p.stats(), stats(64, 64, 64, 0));
    assert_eq!(p.submit(WRITE), Err(SubmitError::NoContext));
    assert_eq!(
        p.stats(),
        Stats {
            tracking_lost: true,
            ..stats(64, 64, 65, 1)
        }
    );
    for ticket in tickets {
        p.complete(ticket, OK).unwrap();
    }
    assert_eq!(
        p.stats(),
        Stats {
            tracking_lost: true,
            ..stats(0, 128, 129, 1)
        }
    );
    for id in 1..=64 {
        assert_submit(p.pop().unwrap(), [id - 1, 11, 22, id], READ_EXPECTED);
    }
    for id in 1..=64 {
        assert_complete(p.pop().unwrap(), [64 + id, 11, 22, id], 0, Some(2));
    }
    assert_eq!(p.pop(), None);
    let next = p.submit(WRITE).unwrap();
    assert_eq!(next.request_id(), 65);
    p.complete(
        next,
        CompletionSnapshot {
            information: 4,
            ..OK
        },
    )
    .unwrap();
    assert_submit(p.pop().unwrap(), [129, 11, 22, 65], WRITE_EXPECTED);
    assert_complete(p.pop().unwrap(), [130, 11, 22, 65], 0, Some(4));
    assert_eq!(
        p.stats(),
        Stats {
            tracking_lost: true,
            ..stats(0, 0, 131, 1)
        }
    );
    p.stop();
    assert_eq!(
        p.seal(),
        Ok(Stats {
            state: CaptureState::Sealed,
            tracking_lost: true,
            ..stats(0, 0, 131, 1)
        })
    );
}

#[test]
fn popping_every_event_does_not_release_any_request_context() {
    let mut p = producer(11, 22);
    let tickets: Vec<_> = (0..64).map(|_| p.submit(READ).unwrap()).collect();
    for id in 1..=64 {
        assert_submit(p.pop().unwrap(), [id - 1, 11, 22, id], READ_EXPECTED);
    }
    assert_eq!(p.stats(), stats(64, 0, 64, 0));
    assert_eq!(p.submit(WRITE), Err(SubmitError::NoContext));
    p.complete(tickets[0], OK).unwrap();
    assert_eq!(
        p.stats(),
        Stats {
            tracking_lost: true,
            ..stats(63, 1, 66, 1)
        }
    );
    let replacement = p.submit(WRITE).unwrap();
    assert_eq!(replacement.request_id(), 65);
    assert_eq!(
        p.stats(),
        Stats {
            tracking_lost: true,
            ..stats(64, 2, 67, 1)
        }
    );
    assert_complete(p.pop().unwrap(), [65, 11, 22, 1], 0, Some(2));
    assert_submit(p.pop().unwrap(), [66, 11, 22, 65], WRITE_EXPECTED);
    p.stop();
    assert_eq!(p.seal(), Err(SealError::Outstanding));
    assert_eq!(
        p.stats(),
        Stats {
            state: CaptureState::Stopping,
            tracking_lost: true,
            ..stats(64, 0, 67, 1)
        }
    );
}

#[test]
fn fifo_wrap_reuses_only_drained_slots_and_keeps_original_attempted_ordinals() {
    let mut p = producer(11, 22);
    for _ in 0..128 {
        let ticket = p.submit(READ).unwrap();
        p.complete(ticket, OK).unwrap();
    }
    assert_submit(p.pop().unwrap(), [0, 11, 22, 1], READ_EXPECTED);
    assert_complete(p.pop().unwrap(), [1, 11, 22, 1], 0, Some(2));
    assert_submit(p.pop().unwrap(), [2, 11, 22, 2], READ_EXPECTED);
    for id in [129, 130] {
        let ticket = p.submit(READ).unwrap();
        assert_eq!(ticket.request_id(), id);
        p.complete(ticket, OK).unwrap();
    }
    assert_eq!(p.stats(), stats(0, 256, 260, 1));
    assert_complete(p.pop().unwrap(), [3, 11, 22, 2], 0, Some(2));
    for id in 3..=128 {
        assert_submit(p.pop().unwrap(), [2 * (id - 1), 11, 22, id], READ_EXPECTED);
        assert_complete(p.pop().unwrap(), [2 * id - 1, 11, 22, id], 0, Some(2));
    }
    assert_submit(p.pop().unwrap(), [256, 11, 22, 129], READ_EXPECTED);
    assert_complete(p.pop().unwrap(), [257, 11, 22, 129], 0, Some(2));
    assert_submit(p.pop().unwrap(), [258, 11, 22, 130], READ_EXPECTED);
    assert_eq!(p.pop(), None);
    let next = p.submit(READ).unwrap();
    assert_eq!(next.request_id(), 131);
    p.complete(next, OK).unwrap();
    assert_submit(p.pop().unwrap(), [260, 11, 22, 131], READ_EXPECTED);
    assert_complete(p.pop().unwrap(), [261, 11, 22, 131], 0, Some(2));
    assert_eq!(p.stats(), stats(0, 0, 262, 1));
}

#[test]
fn stop_retains_outstanding_then_sealed_state_is_immutable_except_event_drain() {
    let mut p = producer(11, 22);
    let ticket = p.submit(READ).unwrap();
    p.stop();
    let stopped = Stats {
        state: CaptureState::Stopping,
        ..stats(1, 1, 1, 0)
    };
    assert_eq!(p.stats(), stopped);
    p.stop();
    assert_eq!(p.submit(WRITE), Err(SubmitError::Closed));
    assert_eq!(
        p.submit(SubmitSnapshot { version: 0, ..READ }),
        Err(SubmitError::Closed)
    );
    assert_eq!(p.seal(), Err(SealError::Outstanding));
    assert_eq!(p.stats(), stopped);
    assert_submit(p.pop().unwrap(), [0, 11, 22, 1], READ_EXPECTED);
    assert_eq!(p.seal(), Err(SealError::Outstanding));
    assert_eq!(
        p.complete(
            ticket,
            CompletionSnapshot {
                status: 0xc0000001,
                ..UNKNOWN
            }
        ),
        Ok(())
    );
    let sealed = Stats {
        state: CaptureState::Sealed,
        ..stats(0, 1, 2, 0)
    };
    assert_eq!(p.seal(), Ok(sealed));
    assert_eq!(
        p.complete(ticket, CompletionSnapshot { version: 0, ..OK }),
        Err(CompleteError::Closed)
    );
    assert_eq!(
        p.submit(SubmitSnapshot { version: 0, ..READ }),
        Err(SubmitError::Closed)
    );
    p.stop();
    assert_eq!(p.seal(), Ok(sealed));
    assert_eq!(p.stats(), sealed);
    assert_complete(p.pop().unwrap(), [1, 11, 22, 1], 0xc0000001, None);
    assert_eq!(p.pop(), None);
    let drained = Stats {
        state: CaptureState::Sealed,
        ..stats(0, 0, 2, 0)
    };
    assert_eq!(p.seal(), Ok(drained));
    assert_eq!(p.stats(), drained);
}

#[test]
fn invalid_submission_latches_fault_without_consuming_ordinal_or_identity() {
    let mut p = producer(11, 22);
    assert_eq!(
        p.submit(SubmitSnapshot {
            reserved: 1,
            ..READ
        }),
        Err(SubmitError::InvalidSnapshot)
    );
    assert_eq!(
        p.stats(),
        Stats {
            tracking_lost: true,
            protocol_fault: true,
            ..stats(0, 0, 0, 0)
        }
    );
    assert_eq!(p.pop(), None);
    let ticket = p.submit(READ).unwrap();
    assert_eq!(ticket.request_id(), 1);
    p.complete(ticket, OK).unwrap();
    assert_submit(p.pop().unwrap(), [0, 11, 22, 1], READ_EXPECTED);
    assert_complete(p.pop().unwrap(), [1, 11, 22, 1], 0, Some(2));
    assert_eq!(
        p.stats(),
        Stats {
            tracking_lost: true,
            protocol_fault: true,
            ..stats(0, 0, 2, 0)
        }
    );
}

#[test]
fn running_snapshot_validation_precedes_context_capacity_check() {
    let mut p = producer(11, 22);
    for _ in 0..64 {
        p.submit(READ).unwrap();
    }
    assert_eq!(
        p.submit(SubmitSnapshot { version: 0, ..READ }),
        Err(SubmitError::InvalidSnapshot)
    );
    assert_eq!(
        p.stats(),
        Stats {
            tracking_lost: true,
            protocol_fault: true,
            ..stats(64, 64, 64, 0)
        }
    );
    assert_eq!(p.submit(READ), Err(SubmitError::NoContext));
    assert_eq!(
        p.stats(),
        Stats {
            tracking_lost: true,
            protocol_fault: true,
            ..stats(64, 64, 65, 1)
        }
    );
    for id in 1..=64 {
        assert_submit(p.pop().unwrap(), [id - 1, 11, 22, id], READ_EXPECTED);
    }
    assert_eq!(p.pop(), None);
}

#[test]
fn invalid_completion_retires_once_and_duplicate_cannot_repair_the_missing_record() {
    let mut p = producer(11, 22);
    let ticket = p.submit(READ).unwrap();
    assert_eq!(
        p.complete(
            ticket,
            CompletionSnapshot {
                information_known: 2,
                ..OK
            }
        ),
        Err(CompleteError::InvalidSnapshot)
    );
    assert_eq!(
        p.stats(),
        Stats {
            protocol_fault: true,
            ..stats(0, 1, 2, 1)
        }
    );
    assert_eq!(p.complete(ticket, OK), Err(CompleteError::InvalidTicket));
    assert_eq!(
        p.stats(),
        Stats {
            protocol_fault: true,
            tracking_lost: true,
            ..stats(0, 1, 2, 1)
        }
    );
    let next = p.submit(WRITE).unwrap();
    assert_eq!(next.request_id(), 2);
    p.complete(
        next,
        CompletionSnapshot {
            information: 4,
            ..OK
        },
    )
    .unwrap();
    assert_submit(p.pop().unwrap(), [0, 11, 22, 1], READ_EXPECTED);
    assert_submit(p.pop().unwrap(), [2, 11, 22, 2], WRITE_EXPECTED);
    assert_complete(p.pop().unwrap(), [3, 11, 22, 2], 0, Some(4));
    assert_eq!(p.pop(), None);
    assert_eq!(
        p.stats(),
        Stats {
            protocol_fault: true,
            tracking_lost: true,
            ..stats(0, 0, 4, 1)
        }
    );
}

#[test]
fn invalid_completion_while_ring_full_counts_one_drop_and_still_retires_context() {
    let mut p = producer(11, 22);
    for _ in 0..128 {
        let ticket = p.submit(READ).unwrap();
        p.complete(ticket, OK).unwrap();
    }
    let ticket = p.submit(READ).unwrap();
    assert_eq!(ticket.request_id(), 129);
    assert_eq!(p.stats(), stats(1, 256, 257, 1));
    assert_eq!(
        p.complete(ticket, CompletionSnapshot { reserved: 1, ..OK }),
        Err(CompleteError::InvalidSnapshot)
    );
    assert_eq!(
        p.stats(),
        Stats {
            protocol_fault: true,
            ..stats(0, 256, 258, 2)
        }
    );
    for id in 1..=128 {
        assert_submit(p.pop().unwrap(), [2 * (id - 1), 11, 22, id], READ_EXPECTED);
        assert_complete(p.pop().unwrap(), [2 * id - 1, 11, 22, id], 0, Some(2));
    }
    assert_eq!(p.pop(), None);
    assert_eq!(
        p.stats(),
        Stats {
            protocol_fault: true,
            ..stats(0, 0, 258, 2)
        }
    );
}

#[test]
fn foreign_ticket_is_rejected_before_metadata_without_mutating_either_context() {
    let mut p = producer(11, 22);
    let mut foreign = producer(12, 23);
    let own = p.submit(READ).unwrap();
    let wrong = foreign.submit(WRITE).unwrap();
    assert_eq!(
        [wrong.session_id(), wrong.target_id(), wrong.request_id()],
        [12, 23, 1]
    );
    assert_eq!(
        p.complete(wrong, CompletionSnapshot { version: 0, ..OK }),
        Err(CompleteError::InvalidTicket)
    );
    assert_eq!(
        p.stats(),
        Stats {
            tracking_lost: true,
            protocol_fault: true,
            ..stats(1, 1, 1, 0)
        }
    );
    assert_eq!(foreign.stats(), stats(1, 1, 1, 0));
    p.complete(own, OK).unwrap();
    foreign
        .complete(
            wrong,
            CompletionSnapshot {
                information: 4,
                ..OK
            },
        )
        .unwrap();
    assert_submit(p.pop().unwrap(), [0, 11, 22, 1], READ_EXPECTED);
    assert_complete(p.pop().unwrap(), [1, 11, 22, 1], 0, Some(2));
    assert_submit(foreign.pop().unwrap(), [0, 12, 23, 1], WRITE_EXPECTED);
    assert_complete(foreign.pop().unwrap(), [1, 12, 23, 1], 0, Some(4));
    assert_eq!(
        p.stats(),
        Stats {
            tracking_lost: true,
            protocol_fault: true,
            ..stats(0, 0, 2, 0)
        }
    );
    assert_eq!(foreign.stats(), stats(0, 0, 2, 0));
}

#[test]
fn completion_information_validation_uses_each_requests_saved_classification() {
    let mut p = producer(11, 22);
    let ordinary = p.submit(READ).unwrap();
    let pnp = p
        .submit(SubmitSnapshot {
            major: 0x1b,
            minor: 7,
            flags: 0x33,
            range_known: 0,
            offset: 0,
            length: 0,
            ..READ
        })
        .unwrap();
    let pointer_like = CompletionSnapshot {
        information: 0xffff8123456789ab,
        ..OK
    };
    assert_eq!(
        p.complete(pnp, pointer_like),
        Err(CompleteError::InvalidSnapshot)
    );
    assert_eq!(
        p.stats(),
        Stats {
            protocol_fault: true,
            ..stats(1, 2, 3, 1)
        }
    );
    assert_eq!(p.complete(ordinary, pointer_like), Ok(()));
    assert_eq!(
        p.stats(),
        Stats {
            protocol_fault: true,
            ..stats(0, 3, 4, 1)
        }
    );
    assert_submit(p.pop().unwrap(), [0, 11, 22, 1], READ_EXPECTED);
    assert_submit(
        p.pop().unwrap(),
        [1, 11, 22, 2],
        ExpectedSubmission {
            major: 0x1b,
            minor: 7,
            flags: 0x33,
            operation: Operation::Other,
            range: None,
            control_code: None,
        },
    );
    assert_complete(
        p.pop().unwrap(),
        [3, 11, 22, 1],
        0,
        Some(0xffff8123456789ab),
    );
    assert_eq!(p.pop(), None);
}

#[test]
fn completion_status_information_unknown_zero_short_and_large_are_not_healed() {
    for (status, information_known, information, expected) in [
        (0, 0, 0, None),
        (0, 1, 0, Some(0)),
        (0, 1, 1, Some(1)),
        (0, 1, u64::MAX, Some(u64::MAX)),
        (0xc0000001, 0, 0, None),
        (0x80000005, 0, 0, None),
        (0x103, 0, 0, None),
    ] {
        let mut p = producer(11, 22);
        let ticket = p.submit(READ).unwrap();
        p.complete(
            ticket,
            CompletionSnapshot {
                status,
                information_known,
                information,
                ..UNKNOWN
            },
        )
        .unwrap();
        assert_eq!(p.stats(), stats(0, 2, 2, 0));
        assert_submit(p.pop().unwrap(), [0, 11, 22, 1], READ_EXPECTED);
        assert_complete(p.pop().unwrap(), [1, 11, 22, 1], status, expected);
        assert_eq!(p.pop(), None);
    }
}

#[test]
fn saved_unknown_zero_range_flush_and_control_fields_survive_notification_queue() {
    for (source, expected, completion, expected_information) in [
        (
            SubmitSnapshot {
                range_known: 0,
                offset: 0,
                length: 0,
                ..READ
            },
            ExpectedSubmission {
                range: None,
                ..READ_EXPECTED
            },
            OK,
            Some(2),
        ),
        (
            SubmitSnapshot { length: 0, ..READ },
            ExpectedSubmission {
                range: Some((4_096, 0)),
                ..READ_EXPECTED
            },
            OK,
            Some(2),
        ),
        (
            SubmitSnapshot {
                major: 0x09,
                minor: 0,
                range_known: 0,
                offset: 0,
                length: 0,
                ..READ
            },
            ExpectedSubmission {
                major: 0x09,
                operation: Operation::Flush,
                range: None,
                ..READ_EXPECTED
            },
            UNKNOWN,
            None,
        ),
        (
            SubmitSnapshot {
                major: 0x0e,
                minor: 0,
                control_known: 1,
                control_code: 0xdeadbeef,
                range_known: 0,
                offset: 0,
                length: 0,
                ..READ
            },
            ExpectedSubmission {
                major: 0x0e,
                operation: Operation::Other,
                range: None,
                control_code: Some(0xdeadbeef),
                ..READ_EXPECTED
            },
            UNKNOWN,
            None,
        ),
        (
            SubmitSnapshot {
                major: 0x0f,
                minor: 0,
                range_known: 0,
                offset: 0,
                length: 0,
                ..READ
            },
            ExpectedSubmission {
                major: 0x0f,
                operation: Operation::Other,
                range: None,
                ..READ_EXPECTED
            },
            UNKNOWN,
            None,
        ),
        (
            SubmitSnapshot {
                minor: 1,
                range_known: 0,
                offset: 0,
                length: 0,
                ..READ
            },
            ExpectedSubmission {
                minor: 1,
                operation: Operation::Other,
                range: None,
                ..READ_EXPECTED
            },
            UNKNOWN,
            None,
        ),
    ] {
        let mut p = producer(11, 22);
        let ticket = p.submit(source).unwrap();
        p.complete(ticket, completion).unwrap();
        assert_eq!(p.stats(), stats(0, 2, 2, 0));
        assert_submit(p.pop().unwrap(), [0, 11, 22, 1], expected);
        assert_complete(p.pop().unwrap(), [1, 11, 22, 1], 0, expected_information);
        assert_eq!(p.pop(), None);
    }
}

#[cfg(target_pointer_width = "64")]
#[test]
fn producer_stays_within_reviewed_host_size_budget() {
    assert!(std::mem::size_of::<Producer>() <= 40_960);
}
