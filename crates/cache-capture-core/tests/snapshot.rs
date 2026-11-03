use cache_capture_core::{
    CompletionSnapshot, Operation, SnapshotError, SubmitSnapshot, decode_completion, decode_submit,
};
use core::mem::{align_of, offset_of, size_of};

fn ordinary() -> SubmitSnapshot {
    SubmitSnapshot {
        version: 1,
        major: 3,
        minor: 0,
        flags: 0xa5,
        control_known: 0,
        range_known: 1,
        control_code: 0,
        reserved: 0,
        offset: 4096,
        length: 8192,
    }
}

fn completion() -> CompletionSnapshot {
    CompletionSnapshot {
        version: 1,
        status: 0,
        information_known: 1,
        reserved: 0,
        information: 8192,
    }
}

#[test]
fn scalar_layout_is_exact_without_serializing_structure_bytes() {
    assert_eq!(
        (size_of::<SubmitSnapshot>(), align_of::<SubmitSnapshot>()),
        (48, 8)
    );
    assert_eq!(
        [
            offset_of!(SubmitSnapshot, version),
            offset_of!(SubmitSnapshot, major),
            offset_of!(SubmitSnapshot, minor),
            offset_of!(SubmitSnapshot, flags),
            offset_of!(SubmitSnapshot, control_known),
            offset_of!(SubmitSnapshot, range_known),
            offset_of!(SubmitSnapshot, control_code),
            offset_of!(SubmitSnapshot, reserved),
            offset_of!(SubmitSnapshot, offset),
            offset_of!(SubmitSnapshot, length)
        ],
        [0, 4, 8, 12, 16, 20, 24, 28, 32, 40]
    );
    assert_eq!(
        (
            size_of::<CompletionSnapshot>(),
            align_of::<CompletionSnapshot>()
        ),
        (24, 8)
    );
    assert_eq!(
        [
            offset_of!(CompletionSnapshot, version),
            offset_of!(CompletionSnapshot, status),
            offset_of!(CompletionSnapshot, information_known),
            offset_of!(CompletionSnapshot, reserved),
            offset_of!(CompletionSnapshot, information)
        ],
        [0, 4, 8, 12, 16]
    );
}

#[test]
fn ordinary_classification_preserves_zero_and_unsigned_end_candidates() {
    let read = decode_submit(ordinary()).unwrap();
    assert_eq!(
        (read.major(), read.minor(), read.flags(), read.operation()),
        (3, 0, 0xa5, Operation::Read)
    );
    assert_eq!(read.control_code(), None);
    let range = read.range().unwrap();
    assert_eq!((range.offset(), range.length()), (4096, 8192));
    let zero = decode_submit(SubmitSnapshot {
        major: 4,
        offset: 0,
        length: 0,
        ..ordinary()
    })
    .unwrap();
    assert_eq!(zero.operation(), Operation::Write);
    assert_eq!(
        (
            zero.range().unwrap().offset(),
            zero.range().unwrap().length()
        ),
        (0, 0)
    );
    let end = decode_submit(SubmitSnapshot {
        offset: 0x7fff_ffff_ffff_ffff,
        length: 0xffff_ffff,
        ..ordinary()
    })
    .unwrap()
    .range()
    .unwrap();
    assert_eq!(
        end.offset().checked_add(end.length()),
        Some(0x8000_0000_ffff_fffe)
    );
}

#[test]
fn unknown_range_is_not_zero_and_nonordinary_unions_are_absent() {
    let unknown = SubmitSnapshot {
        range_known: 0,
        offset: 0,
        length: 0,
        ..ordinary()
    };
    let read = decode_submit(unknown).unwrap();
    assert_eq!((read.operation(), read.range()), (Operation::Read, None));
    for (major, minor, operation) in [
        (3, 1, Operation::Other),
        (4, 1, Operation::Other),
        (9, 0, Operation::Flush),
        (9, 1, Operation::Other),
        (0xf, 0, Operation::Other),
        (0x1b, 7, Operation::Other),
        (0xff, 0xff, Operation::Other),
    ] {
        let value = decode_submit(SubmitSnapshot {
            major,
            minor,
            ..unknown
        })
        .unwrap();
        assert_eq!(
            (
                value.operation(),
                value.range(),
                value.control_code(),
                value.flags()
            ),
            (operation, None, None, 0xa5)
        );
    }
    let control = decode_submit(SubmitSnapshot {
        major: 0xe,
        minor: 0,
        control_known: 1,
        control_code: 0,
        ..unknown
    })
    .unwrap();
    assert_eq!(
        (control.operation(), control.control_code()),
        (Operation::Other, Some(0))
    );
}

#[test]
fn malformed_submit_fields_are_rejected_not_healed() {
    let raw = ordinary();
    let bad = [
        SubmitSnapshot { version: 0, ..raw },
        SubmitSnapshot { version: 2, ..raw },
        SubmitSnapshot { major: 256, ..raw },
        SubmitSnapshot { minor: 256, ..raw },
        SubmitSnapshot { flags: 256, ..raw },
        SubmitSnapshot { reserved: 1, ..raw },
        SubmitSnapshot {
            control_known: 2,
            ..raw
        },
        SubmitSnapshot {
            range_known: 2,
            ..raw
        },
        SubmitSnapshot {
            control_code: 1,
            ..raw
        },
        SubmitSnapshot {
            control_known: 1,
            ..raw
        },
        SubmitSnapshot {
            range_known: 0,
            ..raw
        },
        SubmitSnapshot { major: 9, ..raw },
        SubmitSnapshot { minor: 1, ..raw },
        SubmitSnapshot {
            offset: 0x8000_0000_0000_0000,
            ..raw
        },
        SubmitSnapshot {
            offset: u64::MAX,
            ..raw
        },
        SubmitSnapshot {
            length: 0x1_0000_0000,
            ..raw
        },
        SubmitSnapshot {
            major: 0xf,
            range_known: 0,
            offset: 0,
            length: 0,
            control_known: 1,
            ..raw
        },
        SubmitSnapshot {
            major: 0xe,
            minor: 1,
            range_known: 0,
            offset: 0,
            length: 0,
            control_known: 1,
            ..raw
        },
    ];
    for input in bad {
        assert_eq!(decode_submit(input), Err(SnapshotError::InvalidSnapshot));
    }
}

#[test]
fn exact_success_information_is_independent_of_range_and_not_clipped() {
    let read = decode_submit(ordinary()).unwrap();
    for information in [0, 8192, 0x1_0000_0001, 0xffff_8123_4567_89ab, u64::MAX] {
        let actual = decode_completion(
            CompletionSnapshot {
                information,
                ..completion()
            },
            &read,
        )
        .unwrap();
        assert_eq!((actual.status, actual.information), (0, Some(information)));
    }
    let unknown = decode_submit(SubmitSnapshot {
        range_known: 0,
        offset: 0,
        length: 0,
        ..ordinary()
    })
    .unwrap();
    assert_eq!(
        decode_completion(
            CompletionSnapshot {
                information: 0,
                ..completion()
            },
            &unknown
        )
        .unwrap()
        .information,
        Some(0)
    );
    assert_eq!(
        decode_completion(
            CompletionSnapshot {
                information_known: 0,
                information: 0,
                ..completion()
            },
            &read
        )
        .unwrap()
        .information,
        None
    );
}

#[test]
fn failed_pending_other_and_pnp_information_never_become_known() {
    let read = decode_submit(ordinary()).unwrap();
    for status in [0x103, 0x8000_0005, 0xc000_0185, u32::MAX] {
        let raw = CompletionSnapshot {
            status,
            information_known: 0,
            information: 0,
            ..completion()
        };
        assert_eq!(
            (
                decode_completion(raw, &read).unwrap().status,
                decode_completion(raw, &read).unwrap().information
            ),
            (status, None)
        );
        assert_eq!(
            decode_completion(
                CompletionSnapshot {
                    information_known: 1,
                    ..raw
                },
                &read
            ),
            Err(SnapshotError::InvalidSnapshot)
        );
    }
    for (major, minor) in [(3, 1), (4, 1), (9, 0), (0xe, 0), (0xf, 0), (0x1b, 7)] {
        let saved = decode_submit(SubmitSnapshot {
            major,
            minor,
            range_known: 0,
            offset: 0,
            length: 0,
            ..ordinary()
        })
        .unwrap();
        assert_eq!(
            decode_completion(
                CompletionSnapshot {
                    information_known: 0,
                    information: 0,
                    ..completion()
                },
                &saved
            )
            .unwrap()
            .information,
            None
        );
        assert_eq!(
            decode_completion(
                CompletionSnapshot {
                    information: 0xffff_8123_4567_89ab,
                    ..completion()
                },
                &saved
            ),
            Err(SnapshotError::InvalidSnapshot)
        );
    }
}

#[test]
fn malformed_completion_shapes_are_rejected_not_normalized() {
    let read = decode_submit(ordinary()).unwrap();
    for bad in [
        CompletionSnapshot {
            version: 0,
            ..completion()
        },
        CompletionSnapshot {
            version: 2,
            ..completion()
        },
        CompletionSnapshot {
            reserved: 1,
            ..completion()
        },
        CompletionSnapshot {
            information_known: 2,
            ..completion()
        },
        CompletionSnapshot {
            information_known: 0,
            ..completion()
        },
    ] {
        assert_eq!(
            decode_completion(bad, &read),
            Err(SnapshotError::InvalidSnapshot)
        );
    }
}
