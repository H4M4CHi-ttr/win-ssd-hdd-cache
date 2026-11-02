//! Deterministic host-only completions. Cancellation is not completion or DMA quiescence.
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct RequestToken(u64);

#[derive(Debug)]
pub struct Completion {
    pub token: RequestToken,
    pub payload: Arc<[u8]>,
    pub cancel_requested: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QueueError {
    Full,
    PayloadTooLarge,
    PayloadBudgetExceeded,
    UnknownToken,
    TokenExhausted,
}

pub struct CompletionQueue {
    pending: BTreeMap<RequestToken, Completion>,
    payload_bytes: usize,
    next_token: Option<u64>,
}

impl Default for CompletionQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl CompletionQueue {
    pub fn new() -> Self {
        Self {
            pending: BTreeMap::new(),
            payload_bytes: 0,
            next_token: Some(1),
        }
    }
    pub fn reserve(&mut self, payload: Arc<[u8]>) -> Result<RequestToken, QueueError> {
        if self.pending.len() >= 1024 {
            return Err(QueueError::Full);
        }
        if payload.len() > 1_048_576 {
            return Err(QueueError::PayloadTooLarge);
        }
        if payload.len() > 16_777_216 - self.payload_bytes {
            return Err(QueueError::PayloadBudgetExceeded);
        }
        let value = self.next_token.ok_or(QueueError::TokenExhausted)?;
        let token = RequestToken(value);
        self.payload_bytes += payload.len();
        self.pending.insert(
            token,
            Completion {
                token,
                payload,
                cancel_requested: false,
            },
        );
        self.next_token = value.checked_add(1);
        Ok(token)
    }
    pub fn request_cancel(&mut self, token: RequestToken) -> Result<(), QueueError> {
        let completion = self
            .pending
            .get_mut(&token)
            .ok_or(QueueError::UnknownToken)?;
        completion.cancel_requested = true;
        Ok(())
    }
    pub fn deliver(&mut self, token: RequestToken) -> Result<Completion, QueueError> {
        let completion = self
            .pending
            .remove(&token)
            .ok_or(QueueError::UnknownToken)?;
        self.payload_bytes -= completion.payload.len();
        Ok(completion)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_exhaustion_never_wraps_and_keeps_the_last_request_alive() {
        let mut queue = CompletionQueue::new();
        queue.next_token = Some(u64::MAX);
        let last = queue.reserve(Arc::from([9])).unwrap();
        assert_eq!(last, RequestToken(u64::MAX));
        assert_eq!(
            queue.reserve(Arc::from([])),
            Err(QueueError::TokenExhausted)
        );
        assert_eq!(&*queue.deliver(last).unwrap().payload, &[9]);
        assert_eq!(
            queue.reserve(Arc::from([])),
            Err(QueueError::TokenExhausted)
        );
    }

    #[test]
    fn failed_reserve_does_not_consume_the_next_token() {
        let mut queue = CompletionQueue::new();
        assert_eq!(queue.reserve(Arc::from([1])).unwrap(), RequestToken(1));
        assert_eq!(
            queue.reserve(Arc::from(vec![0; 1_048_577])),
            Err(QueueError::PayloadTooLarge)
        );
        assert_eq!(queue.reserve(Arc::from([2])).unwrap(), RequestToken(2));
    }

    #[test]
    fn size_error_has_priority_over_token_exhaustion() {
        let mut queue = CompletionQueue::new();
        queue.next_token = None;
        assert_eq!(
            queue.reserve(Arc::from(vec![0; 1_048_577])),
            Err(QueueError::PayloadTooLarge)
        );
        assert_eq!(
            queue.reserve(Arc::from([])),
            Err(QueueError::TokenExhausted)
        );
    }
}
