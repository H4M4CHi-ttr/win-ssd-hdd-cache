#[path = "../../../experiments/ocf-portability/rust_spike.rs"]
mod spike;

use spike::{Denied, Experiment, Fault, Status};

fn finish(e: &mut Experiment, id: usize, len: usize) {
    e.transfer(id, 0, len).unwrap();
    e.complete(id, Status::Ok, len).unwrap();
}

fn warm(e: &mut Experiment, line: usize, slot: usize) -> usize {
    let fill = e.fill(line).unwrap();
    finish(e, fill, 4096);
    e.bind(slot, line).unwrap();
    let cache = e.cache_write(fill, slot).unwrap();
    finish(e, cache, 4096);
    e.publish(cache).unwrap();
    cache
}

fn stale_fill_is_rejected(fault: Fault) -> bool {
    let mut e = Experiment::new(fault);
    let f = e.fill(0).unwrap();
    e.transfer(f, 0, 4096).unwrap();
    let w = e.write(0, &[9]).unwrap();
    finish(&mut e, w, 1);
    e.complete(f, Status::Ok, 4096).unwrap();
    e.bind(0, 0).unwrap();
    let c = e.cache_write(f, 0).unwrap();
    finish(&mut e, c, 4096);
    e.publish(c) == Err(Denied::Rejected)
}

fn two_writers_remain_blocking(fault: Fault) -> bool {
    let mut e = Experiment::new(fault);
    let a = e.write(0, &[1]).unwrap();
    let b = e.write(0, &[2]).unwrap();
    finish(&mut e, a, 1);
    let blocked = e.fill(0) == Err(Denied::Rejected);
    finish(&mut e, b, 1);
    blocked && e.fill(0).is_ok()
}

fn partial_overlap_invalidates_both(fault: Fault) -> bool {
    let mut e = Experiment::new(fault);
    warm(&mut e, 0, 0);
    warm(&mut e, 1, 1);
    let w = e.write(4095, &[5, 7]).unwrap();
    e.transfer(w, 1, 1).unwrap();
    e.complete(w, Status::IoError, 0).unwrap();
    let result = e.commit(w).unwrap();
    assert_eq!((result.status, result.reported), (Status::IoError, 0));
    assert_eq!(e.origin(4095, 2).unwrap(), [Some(0), Some(7)]);
    !e.visible(0).unwrap() && !e.visible(1).unwrap()
}

fn physical_overwrite_removes_old_visibility(fault: Fault) -> bool {
    let mut e = Experiment::new(fault);
    let old = e.fill(0).unwrap();
    finish(&mut e, old, 4096);
    let w = e.write(0, &[8]).unwrap();
    finish(&mut e, w, 1);
    warm(&mut e, 0, 0);
    let a = e.cache_write(old, 0).unwrap();
    finish(&mut e, a, 4096);
    !e.visible(0).unwrap() && e.publish(a) == Err(Denied::Rejected)
}

#[test]
fn correct_candidate_handles_required_d02_schedules() {
    assert!(stale_fill_is_rejected(Fault::None));
    assert!(two_writers_remain_blocking(Fault::None));
    assert!(partial_overlap_invalidates_both(Fault::None));
    assert!(physical_overwrite_removes_old_visibility(Fault::None));
}

#[test]
fn fixed_schedules_detect_four_deliberate_candidate_faults() {
    assert!(!stale_fill_is_rejected(Fault::LateToken));
    assert!(!two_writers_remain_blocking(Fault::BooleanWriter));
    assert!(!partial_overlap_invalidates_both(Fault::FirstLineOnly));
    assert!(!physical_overwrite_removes_old_visibility(
        Fault::IgnoreProvenance
    ));
}

#[test]
fn cold_fill_hit_and_final_hit_validation_use_actual_sampled_bytes() {
    let mut e = Experiment::new(Fault::None);
    warm(&mut e, 0, 0);
    let hit = e.hit(0, 2, 0).unwrap();
    finish(&mut e, hit, 2);
    let w = e.write(0, &[4, 5]).unwrap();
    finish(&mut e, w, 2);
    assert_eq!(e.commit(hit), Err(Denied::Rejected));
    let fallback = e.read(0, 2).unwrap();
    finish(&mut e, fallback, 2);
    assert_eq!(e.commit(fallback).unwrap().data, [Some(4), Some(5)]);
    let second = e.read(0, 2).unwrap();
    e.transfer(second, 0, 1).unwrap();
    e.complete(second, Status::IoError, 1).unwrap();
    let result = e.commit(second).unwrap();
    assert_eq!((result.status, result.reported), (Status::IoError, 1));
    assert_eq!(result.data, [Some(4)]);
    assert_eq!(e.commit(second), Err(Denied::State));
}

#[test]
fn short_unknown_or_failed_fill_never_produces_cache_write() {
    let mut e = Experiment::new(Fault::None);
    e.bind(0, 0).unwrap();
    let short = e.fill(0).unwrap();
    e.transfer(short, 0, 1).unwrap();
    e.complete(short, Status::Ok, 1).unwrap();
    assert_eq!(e.cache_write(short, 0), Err(Denied::Rejected));
    let d = e.discard(0, 1).unwrap();
    finish(&mut e, d, 1);
    let unknown = e.fill(0).unwrap();
    finish(&mut e, unknown, 4096);
    assert_eq!(e.cache_write(unknown, 0), Err(Denied::Rejected));
    let failed = e.fill(1).unwrap();
    e.transfer(failed, 0, 4096).unwrap();
    e.complete(failed, Status::IoError, 4096).unwrap();
    e.bind(1, 1).unwrap();
    assert_eq!(e.cache_write(failed, 1), Err(Denied::Rejected));
}

#[test]
fn cancelled_and_invalidated_physical_io_keeps_lease_until_actual_completion() {
    let mut e = Experiment::new(Fault::None);
    let fill = e.fill(0).unwrap();
    finish(&mut e, fill, 4096);
    e.bind(0, 0).unwrap();
    let c = e.cache_write(fill, 0).unwrap();
    e.cancel(c).unwrap();
    e.invalidate_epoch();
    assert_eq!(e.bind(0, 1), Err(Denied::Pending));
    finish(&mut e, c, 4096);
    e.bind(0, 1).unwrap();
    assert_eq!(e.publish(c), Err(Denied::Rejected));
    assert_eq!(e.complete(c, Status::Ok, 4096), Err(Denied::State));
    assert_eq!(e.transfer(c, 0, 1), Err(Denied::State));
}

fn superseded_publish_is_rejected(fault: Fault) -> bool {
    let mut e = Experiment::new(fault);
    let a = e.fill(0).unwrap();
    finish(&mut e, a, 4096);
    let w = e.write(0, &[8]).unwrap();
    finish(&mut e, w, 1);
    let b = e.fill(0).unwrap();
    finish(&mut e, b, 4096);
    e.bind(0, 0).unwrap();
    let cb = e.cache_write(b, 0).unwrap();
    finish(&mut e, cb, 4096);
    let ca = e.cache_write(a, 0).unwrap();
    finish(&mut e, ca, 4096);
    let superseded_rejected = e.publish(cb) == Err(Denied::Rejected);
    assert_eq!(e.publish(ca), Err(Denied::Rejected));
    superseded_rejected
}

#[test]
fn superseded_successful_child_cannot_publish_wrong_physical_slot() {
    assert!(superseded_publish_is_rejected(Fault::None));
}

#[test]
fn provenance_mutant_is_detected_at_publish_not_only_visibility() {
    assert!(!superseded_publish_is_rejected(Fault::IgnoreProvenance));
}

#[test]
fn shared_read_leases_survive_one_completion_cancellation_and_epoch_change() {
    let mut e = Experiment::new(Fault::None);
    let f = e.fill(0).unwrap();
    finish(&mut e, f, 4096);
    e.bind(0, 0).unwrap();
    let c = e.cache_write(f, 0).unwrap();
    finish(&mut e, c, 4096);
    e.publish(c).unwrap();
    let first = e.hit(0, 1, 0).unwrap();
    let second = e.hit(0, 1, 0).unwrap();
    finish(&mut e, first, 1);
    assert_eq!(e.bind(0, 1), Err(Denied::Pending));
    assert_eq!(e.cache_write(f, 0), Err(Denied::Pending));
    e.cancel(second).unwrap();
    assert_eq!(e.bind(0, 1), Err(Denied::Pending));
    assert_eq!(e.cache_write(f, 0), Err(Denied::Pending));
    e.invalidate_epoch();
    assert_eq!(e.bind(0, 1), Err(Denied::Pending));
    finish(&mut e, second, 1);
    e.bind(0, 1).unwrap();
    assert_eq!(e.commit(first), Err(Denied::Rejected));
    assert_eq!(e.commit(second), Err(Denied::Rejected));
}

#[test]
fn reported_prefix_and_fragments_do_not_invent_transfer_or_ack() {
    let mut e = Experiment::new(Fault::None);
    let w = e.write(0, &[1, 2]).unwrap();
    assert_eq!(e.commit(w), Err(Denied::Pending));
    assert_eq!(e.complete(w, Status::Ok, 2), Err(Denied::State));
    e.transfer(w, 1, 1).unwrap();
    assert_eq!(e.transfer(w, 1, 1), Err(Denied::State));
    e.complete(w, Status::IoError, 0).unwrap();
    assert_eq!(e.commit(w).unwrap().reported, 0);
    assert_eq!(e.origin(0, 2).unwrap(), [Some(0), Some(2)]);
    assert_eq!(e.write(u64::MAX, &[1]), Err(Denied::Bounds));
    assert!(e.read(8192, 0).is_ok());
}

#[test]
fn exhausted_limits_never_drop_existing_io_and_measurements_are_explicit() {
    let mut e = Experiment::new(Fault::None);
    let pending = e.write(0, &[1]).unwrap();
    while e.read(0, 8192).is_ok() {}
    let before = e.memory();
    assert!(before.reserved_cells <= 65536);
    assert!(before.record_count <= 64);
    assert_eq!(e.read(0, 8192), Err(Denied::Capacity));
    assert_eq!(e.memory(), before);
    finish(&mut e, pending, 1);
    assert_eq!(e.commit(pending).unwrap().status, Status::Ok);
    println!("D03_HOST_MEMORY {before:?}");
}

#[test]
fn small_generation_and_incarnation_limit_retires_instead_of_wrapping() {
    let mut e = Experiment::with_limit(Fault::None, 2);
    warm(&mut e, 0, 0);
    assert_eq!(e.bind(0, 0), Err(Denied::Rejected));
    let a = e.write(0, &[1]).unwrap();
    finish(&mut e, a, 1);
    let b = e.write(0, &[2]).unwrap();
    finish(&mut e, b, 1);
    assert_eq!(e.fill(0), Err(Denied::Rejected));
    assert!(!e.visible(0).unwrap());
}

#[test]
fn cache_success_count_without_physical_coverage_cannot_publish() {
    let mut e = Experiment::new(Fault::None);
    let f = e.fill(0).unwrap();
    finish(&mut e, f, 4096);
    e.bind(0, 0).unwrap();
    let c = e.cache_write(f, 0).unwrap();
    e.transfer(c, 0, 1).unwrap();
    e.complete(c, Status::Ok, 4096).unwrap();
    assert_eq!(e.publish(c), Err(Denied::Rejected));
    assert!(!e.visible(0).unwrap());
    assert_eq!(e.origin(0, 2).unwrap(), [Some(0), Some(0)]);
}

#[test]
fn returned_hit_snapshot_survives_later_origin_and_slot_writes() {
    let mut e = Experiment::new(Fault::None);
    warm(&mut e, 0, 0);
    let h = e.hit(0, 1, 0).unwrap();
    finish(&mut e, h, 1);
    assert_eq!(e.commit(h).unwrap().data, [Some(0)]);
    let w = e.write(0, &[9]).unwrap();
    finish(&mut e, w, 1);
    warm(&mut e, 0, 0);
    assert_eq!(e.committed(h).unwrap().unwrap().data, [Some(0)]);
}

#[test]
fn zero_length_requests_still_obey_record_cap() {
    let mut e = Experiment::new(Fault::None);
    for _ in 0..64 {
        e.read(8192, 0).unwrap();
    }
    assert_eq!(e.read(8192, 0), Err(Denied::Capacity));
    assert_eq!(e.memory().reserved_cells, 0);
    assert_eq!(e.memory().record_count, 64);
}
