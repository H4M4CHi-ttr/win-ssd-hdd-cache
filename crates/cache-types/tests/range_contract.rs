use cache_types::range::{ByteRange, LineSize, LineSpan, RangeError, checked_range, line_span};

#[test]
fn ranges_are_half_open() {
    assert_eq!(checked_range(3, 4, 8), Ok(ByteRange { start: 3, end: 7 }));
    assert_eq!(checked_range(0, 8, 8), Ok(ByteRange { start: 0, end: 8 }));
}

#[test]
fn empty_range_at_eof_and_empty_device_succeed() {
    assert_eq!(
        checked_range(8192, 0, 8192),
        Ok(ByteRange {
            start: 8192,
            end: 8192
        })
    );
    assert_eq!(checked_range(0, 0, 0), Ok(ByteRange { start: 0, end: 0 }));
}

#[test]
fn offset_error_precedes_length_error() {
    assert_eq!(
        checked_range(9, u64::MAX, 8),
        Err(RangeError::OffsetPastEnd)
    );
    assert_eq!(checked_range(9, 0, 8), Err(RangeError::OffsetPastEnd));
}

#[test]
fn length_past_capacity_is_rejected_without_wrapping() {
    assert_eq!(checked_range(8, 1, 8), Err(RangeError::LengthPastEnd));
    assert_eq!(
        checked_range(1, u64::MAX, u64::MAX),
        Err(RangeError::LengthPastEnd)
    );
}

#[test]
fn maximum_capacity_is_a_valid_mathematical_boundary() {
    assert_eq!(
        checked_range(u64::MAX - 1, 1, u64::MAX),
        Ok(ByteRange {
            start: u64::MAX - 1,
            end: u64::MAX
        })
    );
    assert_eq!(
        checked_range(0, u64::MAX, u64::MAX),
        Ok(ByteRange {
            start: 0,
            end: u64::MAX
        })
    );
}

#[test]
fn two_byte_crossing_covers_both_lines() {
    assert_eq!(
        line_span(4095, 2, 8192, LineSize::K4),
        Ok(LineSpan { first: 0, count: 2 })
    );
}

#[test]
fn aligned_range_does_not_include_the_next_line() {
    assert_eq!(
        line_span(4096, 4096, 8192, LineSize::K4),
        Ok(LineSpan { first: 1, count: 1 })
    );
}

#[test]
fn zero_length_covers_no_lines_even_when_unaligned() {
    assert_eq!(
        line_span(8191, 0, 8192, LineSize::K4),
        Ok(LineSpan { first: 1, count: 0 })
    );
    assert_eq!(
        line_span(8192, 0, 8192, LineSize::K4),
        Ok(LineSpan { first: 2, count: 0 })
    );
}

#[test]
fn every_line_size_has_the_documented_extent() {
    for (size, count) in [
        (LineSize::K4, 17),
        (LineSize::K8, 9),
        (LineSize::K16, 5),
        (LineSize::K32, 3),
        (LineSize::K64, 2),
    ] {
        assert_eq!(
            line_span(0, 65537, 65537, size),
            Ok(LineSpan { first: 0, count })
        );
    }
}

#[test]
fn line_span_preserves_range_errors() {
    assert_eq!(
        line_span(8193, 0, 8192, LineSize::K4),
        Err(RangeError::OffsetPastEnd)
    );
    assert_eq!(
        line_span(8192, 1, 8192, LineSize::K4),
        Err(RangeError::LengthPastEnd)
    );
}

#[test]
fn line_span_does_not_round_up_by_overflowing() {
    assert_eq!(
        line_span(u64::MAX - 1, 1, u64::MAX, LineSize::K4),
        Ok(LineSpan {
            first: 4_503_599_627_370_495,
            count: 1
        })
    );
    assert_eq!(
        line_span(0, u64::MAX, u64::MAX, LineSize::K4),
        Ok(LineSpan {
            first: 0,
            count: 4_503_599_627_370_496
        })
    );
    assert_eq!(
        line_span(u64::MAX, 0, u64::MAX, LineSize::K4),
        Ok(LineSpan {
            first: 4_503_599_627_370_495,
            count: 0
        })
    );
}
