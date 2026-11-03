# P01 — offline capture completeness audit

Use test-driven-development and executing-plans. Task [P01 #79](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/79); normative [contract](../../contracts/capture-envelope-v1.md) independently accepted at46f2cd8. PR #80 merged as `b740bd25f268b55a8321ee1a27fd28d031769ee6`; its tree exactly matches this branch's starting reviewed metadata.

Only pure Rust parsing/audit and a separate unprivileged ordinary-file CLI. Reuse private strict adapters and input reader, retain default-run compatibility, no new dependency. Post-collection evidence is not a producer/kernel ABI/coverage or authorization proof. D01/D04/D10/G1/owner/lab unchanged.

## Fixed fixture expectations before implementation

| Fixture | records/submits/completions/paired/pending/orphan | Reasons |
|---|---|---|
| complete-synthetic | 4/2/2/2/0/0 | none; write error with information3 for length1, read short1/2 are observations, not capture failure |
| complete-observed | 4/2/2/2/0/0 | none; plausible metadata is still unverified |
| missing-completion | 3/2/1/1/1/0 | BOUNDARY_INFLIGHT, UNPAIRED_REQUEST |
| dropped-submit | 3/1/2/1/0/1 | DROPPED_EVENTS, SEQUENCE_GAP, UNPAIRED_REQUEST |
| unknown-operation-range | 4/2/2/2/0/0 | UNKNOWN_RANGE, UNCLASSIFIED_OPERATION |
| unfinalized | 4/2/2/2/0/0 | NOT_FINALIZED |

All counters and order are literal and hand-derived. Three duplicate-target/orphan examples from contract review are fixed regressions. Sequence reorder must set ORDER and GAP, not sort input. Complete status/count never becomes a success/bytes inference.

- [x] Add integration tests and missing literal fixtures; observe RED before implementation.
- [x] Add minimum typed parser, immutable report and exact correlation/loss audit; GREEN without changing hand expectations.
- [x] Add process tests RED, then separate CLI reusing bounded file input; inspect0/4/2/3 and nonreflection.
- [x] Run full workspace/fmt/clippy/planning; record exact nonzero counts and external fixture hashes.
- [x] Independent Astra code/expectation review; reproduce and fix findings before re-review.
- [x] Publish exact reviewed tree, inspect two-OS CI logs and merge normally: PR #81, Windows248/Linux251, merge4cffcbc2a9a5480f1c2ff5740673c307a52d7231.
- [ ] Synchronize acceptance/Issue/roadmap and verify readback in the separate final metadata increment.

Root's pre-review checks: full Linux workspace251 named tests,0 failed/ignored; fmt/clippy `-D warnings` and planning28 labels/54 tasks/60 documents/172 links pass. Core target17 tests; CLI target10 Linux tests. The extra Windows namespace process case and reused reader's Windows unit case are not locally executed. Seven literal files were hashed with sha256sum. [Detailed evidence](../../evidence/P01.md) records the source checkpoint and independent review; this does not pre-accept the pending publication/CI/Issue gates.
