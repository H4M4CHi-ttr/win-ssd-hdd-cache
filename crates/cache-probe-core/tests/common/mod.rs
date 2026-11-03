#![allow(dead_code)]

pub fn hex(text: &str) -> Vec<u8> {
    let digits: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(digits.len().is_multiple_of(2));
    digits
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

pub fn read_pair() -> Vec<u8> {
    hex(include_str!(
        "../../../../tests/fixtures/capture-probe-v1/read-pair.hex"
    ))
}

pub fn flush_pair() -> Vec<u8> {
    hex(include_str!(
        "../../../../tests/fixtures/capture-probe-v1/flush-pair.hex"
    ))
}

pub fn set32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

pub fn set64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

// Fixed positions are hand-derived from prefix64 + headers144 + two records96.
pub const BEGIN: usize = 64;
pub const DRAIN: usize = 208;
pub const SUBMIT: usize = 352;
pub const COMPLETE: usize = 448;
pub const STOP: usize = 544;
pub const FOOTER: usize = 688;

pub fn frames() -> Vec<Vec<u8>> {
    let b = read_pair();
    vec![
        b[BEGIN..DRAIN].to_vec(),
        b[DRAIN..STOP].to_vec(),
        b[STOP..FOOTER].to_vec(),
    ]
}

pub fn archive(frames: &[Vec<u8>], footer: bool) -> Vec<u8> {
    let literal = read_pair();
    let mut result = literal[..64].to_vec();
    let mut records = 0;
    for frame in frames {
        records += (frame.len() - 144) / 96;
        result.extend(frame);
    }
    if footer {
        let mut end = literal[FOOTER..].to_vec();
        set32(&mut end, 16, frames.len() as u32);
        set32(&mut end, 20, records as u32);
        set32(&mut end, 28, frames.len().saturating_sub(1) as u32);
        result.extend(end);
    }
    result
}

pub fn json_report(bytes: &[u8]) -> serde_json::Value {
    serde_json::from_str(&cache_probe_core::audit_archive(bytes).unwrap().to_json()).unwrap()
}
