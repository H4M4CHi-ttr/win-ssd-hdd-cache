mod common;

use cache_probe_core::{Command, Request, decode_response};
use common::*;
use std::num::NonZeroU64;

fn request(command: Command, call: u64, maximum: u32) -> Request {
    Request::new(
        command,
        NonZeroU64::new(7).unwrap(),
        NonZeroU64::new(call).unwrap(),
        maximum,
    )
    .unwrap()
}

#[test]
fn wire_lengths_and_ioctl_values() {
    // A wrong access/method/function bit or layout byte must fail the consumer boundary.
    for (command, call, maximum, ioctl, output) in [
        (Command::Begin, 1, 0, 0x8337_E000, 144),
        (Command::Status, 2, 0, 0x8337_E004, 144),
        (Command::Drain, 2, 16, 0x8337_E008, 1680),
        (Command::Stop, 3, 0, 0x8337_E00C, 144),
    ] {
        let req = request(command, call, maximum);
        assert_eq!(req.ioctl(), ioctl);
        assert_eq!(command.ioctl(), ioctl);
        assert_eq!(req.output_capacity(), output);
    }
    let expected = hex(
        "5743485030345251 01000000 30000000 0700000000000000 0100000000000000 0200000000000000 10000000 00000000",
    );
    assert_eq!(
        request(Command::Drain, 2, 16).to_bytes().as_slice(),
        expected
    );
    for (command, call, maximum) in [
        (Command::Begin, 2, 0),
        (Command::Drain, 2, 0),
        (Command::Drain, 2, 17),
        (Command::Stop, 3, 1),
    ] {
        assert_eq!(
            Request::new(
                command,
                NonZeroU64::new(7).unwrap(),
                NonZeroU64::new(call).unwrap(),
                maximum
            )
            .unwrap_err()
            .code(),
            "INVALID_PROBE_ARCHIVE"
        );
    }
}

#[test]
fn wire_known_read_pair() {
    let bytes = read_pair();
    assert_eq!(bytes.len(), 752);
    let frame = decode_response(request(Command::Drain, 2, 16), &bytes[DRAIN..STOP]).unwrap();
    assert_eq!(frame.records().len(), 2);
    assert_eq!(frame.records()[0].seq(), 0);
    assert_eq!(frame.records()[0].request_id(), 1);
    let submit = frame.records()[0].submission().unwrap();
    assert_eq!((submit.major(), submit.minor(), submit.flags()), (3, 0, 0));
    assert_eq!(
        (
            submit.range().unwrap().offset(),
            submit.range().unwrap().length()
        ),
        (4096, 512)
    );
    assert_eq!(frame.records()[1].seq(), 1);
    assert_eq!(frame.records()[1].status(), Some(0));
    assert_eq!(frame.records()[1].information(), Some(512));
}

#[test]
fn wire_binding_and_reserved_rejected() {
    let bytes = read_pair();
    let good = &bytes[DRAIN..STOP];
    for offset in [
        8,
        12,
        16,
        20,
        28,
        32,
        40,
        48,
        112,
        116,
        120,
        143,
        144 + 20,
        144 + 80,
        240 + 80,
    ] {
        let mut bad = good.to_vec();
        bad[offset] ^= 0x80;
        assert_eq!(
            decode_response(request(Command::Drain, 2, 16), &bad)
                .unwrap_err()
                .code(),
            "INVALID_PROBE_ARCHIVE",
            "offset {offset}"
        );
    }
    for (offset, value) in [
        (56, 0),
        (56, 4),
        (60, 7),
        (64, 65),
        (68, 257),
        (72, 8),
        (76, 512),
        (144 + 24, 256),
        (144 + 36, 2),
        (240 + 68, 2),
    ] {
        let mut bad = good.to_vec();
        set32(&mut bad, offset, value);
        assert!(
            decode_response(request(Command::Drain, 2, 16), &bad).is_err(),
            "offset {offset}"
        );
    }
    let mut bad = good.to_vec();
    set32(&mut bad, 72, 6); // unknown attempted/drop cannot contain 2
    assert!(decode_response(request(Command::Drain, 2, 16), &bad).is_err());
}

#[test]
fn wire_actual_returned_bytes_not_capacity() {
    let bytes = read_pair();
    let good = bytes[DRAIN..STOP].to_vec();
    assert!(decode_response(request(Command::Drain, 2, 16), &good).is_ok());
    let mut padded = good.clone();
    padded.resize(1680, 0);
    assert!(decode_response(request(Command::Drain, 2, 16), &padded).is_err());
    assert!(decode_response(request(Command::Drain, 2, 1), &good).is_err());
    for end in [0, 8, 143, 144, 335] {
        assert!(decode_response(request(Command::Drain, 2, 16), &good[..end]).is_err());
    }
}

#[test]
fn wire_unknown_information_distinct_from_zero() {
    let bytes = flush_pair();
    let frame = decode_response(request(Command::Drain, 2, 16), &bytes[DRAIN..STOP]).unwrap();
    assert_eq!(frame.records()[1].status(), Some(0));
    assert_eq!(frame.records()[1].information(), None);
    let mut known_zero = read_pair()[DRAIN..STOP].to_vec();
    set64(&mut known_zero, 240 + 72, 0);
    assert_eq!(
        decode_response(request(Command::Drain, 2, 16), &known_zero)
            .unwrap()
            .records()[1]
            .information(),
        Some(0)
    );
    set32(&mut known_zero, 240 + 64, 0xc000_0001);
    assert!(decode_response(request(Command::Drain, 2, 16), &known_zero).is_err());
    let mut unknown_nonzero = bytes[DRAIN..STOP].to_vec();
    set64(&mut unknown_nonzero, 240 + 72, 1);
    assert!(decode_response(request(Command::Drain, 2, 16), &unknown_nonzero).is_err());
}

#[test]
fn wire_scalar_union_and_bounds_are_not_reinterpreted() {
    let source = read_pair();
    for (major, minor, control_known, range_known, code, offset, length) in [
        (15, 0, 1, 0, 7, 0, 0), // internal control is never public control
        (3, 1, 0, 1, 0, 0, 1),
        (9, 0, 0, 1, 0, 0, 1),
        (3, 0, 0, 1, 0, 1u64 << 63, 1),
        (3, 0, 0, 1, 0, 0, 1u64 << 32),
        (3, 0, 0, 0, 0, 1, 0),
        (14, 0, 0, 0, 1, 0, 0),
    ] {
        let mut b = source[DRAIN..STOP].to_vec();
        for (o, v) in [
            (24, major),
            (28, minor),
            (36, control_known),
            (40, range_known),
            (44, code),
        ] {
            set32(&mut b, 144 + o, v);
        }
        set64(&mut b, 144 + 48, offset);
        set64(&mut b, 144 + 56, length);
        assert!(decode_response(request(Command::Drain, 2, 16), &b).is_err());
    }
}
