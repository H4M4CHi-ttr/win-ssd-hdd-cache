use cache_model::origin::{MemoryOrigin, OriginError};
use cache_types::range::RangeError;

#[test]
fn initial_bytes_are_known_and_read_results_are_independent() {
    let origin = MemoryOrigin::new(vec![1, 2, 3, 4]).unwrap();
    let mut copy = origin.read(1, 2).unwrap();
    assert_eq!(copy, vec![2, 3]);
    copy.fill(9);
    assert_eq!(origin.read(0, 4), Ok(vec![1, 2, 3, 4]));
}

#[test]
fn cross_line_write_only_changes_its_two_bytes() {
    let mut origin = MemoryOrigin::new(vec![7; 8192]).unwrap();
    origin.write(4095, &[11, 12]).unwrap();
    assert_eq!(origin.read(4094, 4), Ok(vec![7, 11, 12, 7]));
    assert_eq!(origin.read(0, 2), Ok(vec![7, 7]));
    assert_eq!(origin.read(8190, 2), Ok(vec![7, 7]));
}

#[test]
fn invalid_mutations_preserve_bytes_and_unknown_map() {
    let mut origin = MemoryOrigin::new(vec![1, 2, 3, 4]).unwrap();
    origin.discard(1, 1).unwrap();
    assert_eq!(
        origin.write(3, &[8, 9]),
        Err(OriginError::Range(RangeError::LengthPastEnd))
    );
    assert_eq!(
        origin.discard(3, 2),
        Err(OriginError::Range(RangeError::LengthPastEnd))
    );
    assert_eq!(
        origin.write(5, &[]),
        Err(OriginError::Range(RangeError::OffsetPastEnd))
    );
    assert_eq!(origin.read(0, 1), Ok(vec![1]));
    assert_eq!(origin.read(1, 1), Err(OriginError::UnknownRange));
    assert_eq!(origin.read(2, 2), Ok(vec![3, 4]));
}

#[test]
fn discard_is_unknown_and_partial_rewrite_only_restores_written_bytes() {
    let mut origin = MemoryOrigin::new(vec![5, 6, 7, 8]).unwrap();
    origin.discard(1, 2).unwrap();
    assert_eq!(origin.read(0, 4), Err(OriginError::UnknownRange));
    origin.write(1, &[9]).unwrap();
    assert_eq!(origin.read(0, 2), Ok(vec![5, 9]));
    assert_eq!(origin.read(2, 1), Err(OriginError::UnknownRange));
    origin.write(2, &[0]).unwrap();
    assert_eq!(origin.read(0, 4), Ok(vec![5, 9, 0, 8]));
}

#[test]
fn zero_length_is_valid_at_eof_and_on_unknown_bytes() {
    let mut origin = MemoryOrigin::new(vec![5, 6]).unwrap();
    origin.discard(0, 2).unwrap();
    assert_eq!(origin.read(0, 0), Ok(vec![]));
    assert_eq!(origin.read(2, 0), Ok(vec![]));
    assert_eq!(origin.write(2, &[]), Ok(()));
    assert_eq!(origin.discard(2, 0), Ok(()));
    assert_eq!(origin.read(0, 1), Err(OriginError::UnknownRange));
    assert_eq!(MemoryOrigin::new(vec![]).unwrap().read(0, 0), Ok(vec![]));
}

#[test]
fn overflowing_ranges_are_rejected_without_mutation() {
    let mut origin = MemoryOrigin::new(vec![1]).unwrap();
    assert_eq!(
        origin.discard(1, u64::MAX),
        Err(OriginError::Range(RangeError::LengthPastEnd))
    );
    assert_eq!(
        origin.read(u64::MAX, 1),
        Err(OriginError::Range(RangeError::OffsetPastEnd))
    );
    assert_eq!(origin.read(0, 1), Ok(vec![1]));
}

#[test]
fn capacity_limit_accepts_exactly_one_mib() {
    let origin = MemoryOrigin::new(vec![3; 1_048_576]).unwrap();
    assert_eq!(origin.read(1_048_575, 1), Ok(vec![3]));
    assert_eq!(
        MemoryOrigin::new(vec![0; 1_048_577]).unwrap_err(),
        OriginError::CapacityTooLarge
    );
}
