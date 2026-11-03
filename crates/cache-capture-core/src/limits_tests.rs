//! Private limit injection only; these are not physically observed counters.
use super::*;
use crate::{CompletionSnapshot, SubmitSnapshot};
use core::num::NonZeroU64;

const READ: SubmitSnapshot = SubmitSnapshot {
    version: 1,
    major: 3,
    minor: 0,
    flags: 0,
    control_known: 0,
    range_known: 1,
    control_code: 0,
    reserved: 0,
    offset: 0,
    length: 1,
};
const OK: CompletionSnapshot = CompletionSnapshot {
    version: 1,
    status: 0,
    information_known: 1,
    reserved: 0,
    information: 1,
};

fn producer() -> Producer {
    Producer::new(NonZeroU64::new(1).unwrap(), NonZeroU64::new(2).unwrap())
}

#[test]
fn final_request_identity_is_allocated_once_and_diagnostic_is_immediate() {
    let mut p = producer();
    p.next_request_id = Some(u64::MAX);
    let final_ticket = p.submit(READ).unwrap();
    assert_eq!(final_ticket.request_id(), u64::MAX);
    assert!(p.stats().identity_exhausted);
    assert_eq!(p.submit(READ), Err(SubmitError::IdentityExhausted));
    assert_eq!(
        (
            p.stats().attempted,
            p.stats().dropped,
            p.stats().active_contexts
        ),
        (Some(2), Some(1), 1)
    );
    p.complete(final_ticket, OK).unwrap();
    assert_eq!(
        (
            p.stats().attempted,
            p.stats().dropped,
            p.stats().active_contexts
        ),
        (Some(3), Some(1), 0)
    );
    assert!(p.stats().tracking_lost);
    assert_eq!((p.pop().unwrap().seq, p.pop().unwrap().seq), (0, 2));
    assert_eq!(p.pop(), None);
}

#[test]
fn context_exhaustion_precedes_identity_exhaustion_but_not_validation() {
    let mut p = producer();
    let first = p.submit(READ).unwrap();
    for _ in 1..64 {
        p.submit(READ).unwrap();
    }
    p.next_request_id = None;
    p.identity_exhausted = true;
    assert_eq!(
        p.submit(SubmitSnapshot { version: 0, ..READ }),
        Err(SubmitError::InvalidSnapshot)
    );
    assert_eq!(p.stats().attempted, Some(64));
    assert_eq!(p.submit(READ), Err(SubmitError::NoContext));
    p.complete(first, OK).unwrap();
    assert_eq!(p.submit(READ), Err(SubmitError::IdentityExhausted));
    assert_eq!(
        (
            p.stats().attempted,
            p.stats().dropped,
            p.stats().active_contexts
        ),
        (Some(67), Some(2), 63)
    );
}

#[test]
fn attempted_overflow_discards_new_events_but_still_retires_and_drains() {
    let mut p = producer();
    let ticket = p.submit(READ).unwrap();
    p.attempted = Some(u64::MAX);
    p.complete(ticket, OK).unwrap();
    assert_eq!((p.stats().attempted, p.stats().dropped), (None, None));
    assert!(p.stats().counter_exhausted);
    assert!(!p.stats().tracking_lost);
    assert_eq!(
        (p.stats().active_contexts, p.stats().queued_records),
        (0, 1)
    );
    assert_eq!(p.pop().unwrap().seq, 0);
    let later = p.submit(READ).unwrap();
    assert_eq!(later.request_id(), 2);
    p.complete(later, OK).unwrap();
    assert_eq!(p.pop(), None);
    p.stop();
    let final_stats = p.seal().unwrap();
    assert_eq!((final_stats.attempted, final_stats.dropped), (None, None));
    assert_eq!(
        (final_stats.active_contexts, final_stats.queued_records),
        (0, 0)
    );
    assert!(final_stats.counter_exhausted);
}

#[test]
fn last_representable_attempt_is_retained_without_wrapping_next_ordinal() {
    let mut p = producer();
    p.attempted = Some(u64::MAX - 1);
    let ticket = p.submit(READ).unwrap();
    assert_eq!(p.stats().attempted, Some(u64::MAX));
    assert!(!p.stats().counter_exhausted);
    p.complete(ticket, OK).unwrap();
    assert_eq!(p.pop().unwrap().seq, u64::MAX - 1);
    assert_eq!(p.pop(), None);
    assert_eq!((p.stats().attempted, p.stats().dropped), (None, None));
    assert_eq!(p.stats().active_contexts, 0);
}

#[test]
fn dropped_overflow_makes_both_counters_unknown_and_preserves_ring() {
    let mut p = producer();
    for _ in 0..128 {
        let t = p.submit(READ).unwrap();
        p.complete(t, OK).unwrap();
    }
    p.dropped = Some(u64::MAX);
    let t = p.submit(READ).unwrap();
    assert_eq!(t.request_id(), 129);
    assert_eq!((p.stats().attempted, p.stats().dropped), (None, None));
    assert_eq!(
        (p.stats().active_contexts, p.stats().queued_records),
        (1, 256)
    );
    p.complete(t, OK).unwrap();
    assert_eq!(p.stats().active_contexts, 0);
    for seq in 0..256 {
        assert_eq!(p.pop().unwrap().seq, seq);
    }
    assert_eq!(p.pop(), None);
    assert!(p.stats().counter_exhausted);
    assert!(!p.stats().tracking_lost);
}

#[test]
fn invalid_completion_during_counter_exhaustion_cannot_pin_a_context() {
    let mut p = producer();
    let t = p.submit(READ).unwrap();
    p.attempted = Some(u64::MAX);
    assert_eq!(
        p.complete(t, CompletionSnapshot { reserved: 1, ..OK }),
        Err(CompleteError::InvalidSnapshot)
    );
    assert_eq!(p.stats().active_contexts, 0);
    assert_eq!((p.stats().attempted, p.stats().dropped), (None, None));
    assert!(p.stats().protocol_fault);
    assert!(!p.stats().tracking_lost);
    p.stop();
    assert_eq!(p.seal().unwrap().state, CaptureState::Sealed);
}

#[test]
fn lowest_free_slot_is_reused_but_its_request_identity_is_not() {
    let mut p = producer();
    let first = p.submit(READ).unwrap();
    let second = p.submit(READ).unwrap();
    assert_eq!((first.slot, second.slot), (0, 1));
    p.complete(first, OK).unwrap();
    let third = p.submit(READ).unwrap();
    assert_eq!((third.slot, third.request_id()), (0, 3));
    assert_eq!(p.complete(first, OK), Err(CompleteError::InvalidTicket));
    assert_eq!(p.stats().active_contexts, 2);
}

#[test]
fn impossible_ticket_slot_is_rejected_before_indexing_or_retirement() {
    let mut p = producer();
    let valid = p.submit(READ).unwrap();
    let invalid = Ticket {
        slot: usize::MAX,
        ..valid
    };
    assert_eq!(p.complete(invalid, OK), Err(CompleteError::InvalidTicket));
    assert_eq!(
        (p.stats().attempted, p.stats().active_contexts),
        (Some(1), 1)
    );
    p.complete(valid, OK).unwrap();
    assert_eq!(p.stats().active_contexts, 0);
}
