use cache_model::completion::{CompletionQueue, QueueError};
use std::sync::Arc;

#[test]
fn cancellation_retains_payload_until_explicit_completion() {
    let payload: Arc<[u8]> = Arc::from([1, 2, 3]);
    let weak = Arc::downgrade(&payload);
    let mut queue = CompletionQueue::new();
    let token = queue.reserve(payload).unwrap();
    queue.request_cancel(token).unwrap();
    queue.request_cancel(token).unwrap();
    assert!(weak.upgrade().is_some());
    let completion = queue.deliver(token).unwrap();
    assert_eq!(completion.token, token);
    assert!(completion.cancel_requested);
    assert_eq!(&*completion.payload, &[1, 2, 3]);
    drop(queue);
    assert_eq!(&*completion.payload, &[1, 2, 3]);
    drop(completion);
    assert!(weak.upgrade().is_none());
}

#[test]
fn completion_order_is_chosen_by_the_caller() {
    let mut queue = CompletionQueue::new();
    let a = queue.reserve(Arc::from([7])).unwrap();
    let b = queue.reserve(Arc::from([8])).unwrap();
    assert!(a < b);
    let second = queue.deliver(b).unwrap();
    assert_eq!(&*second.payload, &[8]);
    assert!(!second.cancel_requested);
    assert_eq!(&*queue.deliver(a).unwrap().payload, &[7]);
}

#[test]
fn completed_and_unknown_tokens_are_rejected_without_affecting_pending_work() {
    let mut queue = CompletionQueue::new();
    let a = queue.reserve(Arc::from([1])).unwrap();
    queue.deliver(a).unwrap();
    let b = queue.reserve(Arc::from([2])).unwrap();
    assert!(b > a);
    assert_eq!(queue.deliver(a).unwrap_err(), QueueError::UnknownToken);
    assert_eq!(queue.request_cancel(a), Err(QueueError::UnknownToken));
    let mut empty = CompletionQueue::new();
    assert_eq!(empty.deliver(b).unwrap_err(), QueueError::UnknownToken);
    assert_eq!(&*queue.deliver(b).unwrap().payload, &[2]);
}

#[test]
fn pending_limit_has_priority_and_rejection_preserves_the_queue() {
    let mut queue = CompletionQueue::new();
    let mut tokens = Vec::new();
    for _ in 0..1024 {
        tokens.push(queue.reserve(Arc::from([])).unwrap());
    }
    assert_eq!(
        queue.reserve(Arc::from(vec![0; 1_048_577])),
        Err(QueueError::Full)
    );
    queue.request_cancel(tokens[0]).unwrap();
    assert_eq!(queue.reserve(Arc::from([])), Err(QueueError::Full));
    queue.deliver(tokens[0]).unwrap();
    let later = queue.reserve(Arc::from([])).unwrap();
    assert!(later > *tokens.last().unwrap());
    for token in tokens.into_iter().skip(1) {
        queue.deliver(token).unwrap();
    }
    queue.deliver(later).unwrap();
}

#[test]
fn payload_limit_precedes_total_budget_and_rejection_does_not_consume_tokens() {
    let payload: Arc<[u8]> = Arc::from(vec![0x5a; 1_048_576]);
    let mut queue = CompletionQueue::new();
    let mut last = None;
    for _ in 0..16 {
        last = Some(queue.reserve(payload.clone()).unwrap());
    }
    assert_eq!(
        queue.reserve(Arc::from(vec![0; 1_048_577])),
        Err(QueueError::PayloadTooLarge)
    );
    assert_eq!(
        queue.reserve(Arc::from([1])),
        Err(QueueError::PayloadBudgetExceeded)
    );
    let last = last.unwrap();
    queue.request_cancel(last).unwrap();
    assert_eq!(
        queue.reserve(Arc::from([1])),
        Err(QueueError::PayloadBudgetExceeded)
    );
    assert_eq!(&*queue.deliver(last).unwrap().payload, &*payload);
    assert!(queue.reserve(payload).unwrap() > last);
}

#[test]
fn a_failed_reservation_leaves_existing_payload_unchanged() {
    let mut queue = CompletionQueue::new();
    let token = queue.reserve(Arc::from([11, 12])).unwrap();
    assert_eq!(
        queue.reserve(Arc::from(vec![0; 1_048_577])),
        Err(QueueError::PayloadTooLarge)
    );
    assert_eq!(&*queue.deliver(token).unwrap().payload, &[11, 12]);
}
