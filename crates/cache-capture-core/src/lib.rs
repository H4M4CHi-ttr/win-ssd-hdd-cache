//! Bounded observation metadata only; no Windows callback or quiescence proof.
#![no_std]
#![forbid(unsafe_code)]

mod producer;
mod snapshot;

pub use producer::{
    CaptureState, CompleteError, Observation, Producer, Record, SealError, Stats, SubmitError,
    Ticket,
};

pub use snapshot::{
    Completion, CompletionSnapshot, Operation, RangeCandidate, SnapshotError, Submission,
    SubmitSnapshot, decode_completion, decode_submit,
};
