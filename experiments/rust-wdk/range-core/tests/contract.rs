use wdk_probe_core::{RangeReply, checked_range};

#[test]
fn explicit_c_layout_has_no_implicit_gap() {
    assert_eq!(size_of::<RangeReply>(), 24);
    assert_eq!(align_of::<RangeReply>(), 8);
    assert_eq!(core::mem::offset_of!(RangeReply, status), 0);
    assert_eq!(core::mem::offset_of!(RangeReply, reserved), 4);
    assert_eq!(core::mem::offset_of!(RangeReply, start), 8);
    assert_eq!(core::mem::offset_of!(RangeReply, end), 16);
}

#[test]
fn literal_boundaries_and_error_priority() {
    let cases = [
        ((0, 0, 0), (0, 0, 0)),
        ((4, 2, 6), (0, 4, 6)),
        ((6, 0, 6), (0, 6, 6)),
        ((7, 9, 6), (1, 0, 0)),
        ((4, 3, 6), (2, 0, 0)),
        ((u64::MAX, 0, u64::MAX), (0, u64::MAX, u64::MAX)),
        ((u64::MAX, 1, u64::MAX), (2, 0, 0)),
        ((1, u64::MAX, u64::MAX), (2, 0, 0)),
    ];
    for ((offset, length, capacity), (status, start, end)) in cases {
        assert_eq!(
            checked_range(offset, length, capacity),
            RangeReply {
                status,
                reserved: 0,
                start,
                end
            }
        );
    }
}

#[test]
fn bounded_exhaustive_ranges_match_widened_arithmetic() {
    for capacity in 0..32_u64 {
        for offset in 0..40_u64 {
            for length in 0..40_u64 {
                let reply = checked_range(offset, length, capacity);
                let expected = if offset > capacity {
                    1
                } else if u128::from(offset) + u128::from(length) > u128::from(capacity) {
                    2
                } else {
                    0
                };
                assert_eq!(reply.status, expected);
                assert_eq!(reply.reserved, 0);
                if expected == 0 {
                    assert_eq!((reply.start, reply.end), (offset, offset + length));
                } else {
                    assert_eq!((reply.start, reply.end), (0, 0));
                }
            }
        }
    }
}
