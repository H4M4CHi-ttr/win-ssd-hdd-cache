#[path = "../../../experiments/coherence/model.rs"]
mod model;

use model::{Case, Fault, Outcome, explore};

#[test]
fn every_correct_bounded_case_is_exhausted_without_a_violation() {
    for case in Case::ALL {
        let report = explore(case, Fault::None, 100_000);
        assert_eq!(
            report.outcome,
            Outcome::SafeWithinBound,
            "{case:?}: {report:?}"
        );
        assert!(report.states > 1 && report.transitions > 0 && report.terminals > 0);
        println!(
            "{case:?}: states={} transitions={} terminals={}",
            report.states, report.transitions, report.terminals
        );
    }
}

#[test]
fn wrong_models_produce_valid_safety_counterexamples() {
    let pairs = [
        (Case::StaleFill, Fault::IgnoreGeneration),
        (Case::StaleFill, Fault::CaptureTokenAtSubmit),
        (Case::TwoWriters, Fault::BooleanWriters),
        (Case::PartialWrite, Fault::IgnoreGeneration),
        (Case::ShortFill, Fault::PublishShortFill),
        (Case::UnknownFill, Fault::PublishUnknownFill),
        (Case::HitRace, Fault::SkipHitRevalidation),
        (Case::CounterExhaustion, Fault::WrapGeneration),
        (Case::LateSlotWrite, Fault::ReleaseOnEpoch),
        (Case::LateSlotRead, Fault::ReleaseOnEpoch),
        (Case::CancelledIo, Fault::ReleaseOnCancel),
        (Case::LateCallerCompletion, Fault::CompleteParentTwice),
    ];
    for (case, fault) in pairs {
        let report = explore(case, fault, 100_000);
        assert_eq!(
            report.outcome,
            Outcome::Violation,
            "{case:?}/{fault:?}: {report:?}"
        );
        assert!(!report.witness.is_empty());
        assert!(report.violation.is_some());
        println!(
            "{case:?}/{fault:?}: {:?}: {:?}",
            report.violation, report.witness
        );
    }
}

#[test]
fn exhaustion_is_inconclusive_never_a_success() {
    let report = explore(Case::TwoWriters, Fault::None, 1);
    assert_eq!(report.outcome, Outcome::Inconclusive);
    assert_eq!(report.terminals, 0);
}
