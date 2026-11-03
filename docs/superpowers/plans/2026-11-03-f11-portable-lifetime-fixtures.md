# F11 — bounded portable lifetime fixtures

> Use superpowers:executing-plans and test-driven-development. Existing trace/oracle contracts, not an extension of Windows semantics.

Task: [F11 #75](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/75). Approved scope is recorded in the [boundary audit](../../portable-boundary-audit-2026-11-03.md) and PR #76. Dependencies F06/D02/D07 are accepted. Exactly five literal JSON fixtures plus one Rust integration target and evidence, no production logic changes. All reference traces drain; no added OOM/timer/MDL/IRQL model. F08/F09/D04/D10/G1 and owner approvals are unchanged.

## Hand-derived expectations before replay

| Fixture / seed | Fixed order and expected results | Events / assertions / cache-lineage checks |
|---|---|---|
| cancel-held-lease /301 | Fill sourceA, cache write pending → cancel parent and child → bind8/retire10 pending → actual cancelled completion13 → bind14 accepted, publish16 rejected → origin readA | 30 /8 /0 |
| epoch-held-lease /302 | Pending cache write → epoch6 → bind7/resume9/retire11 pending → actual completion14 → publish15 rejected → bind17/resume19 accepted cold → originA | 32 /9 /0 |
| fallback-late-cache /303 | WarmA → cache read child pending → origin writeB → cancel cache flag → origin fallbackB committed20 → bind22/retire24 pending → late cache samplesA/completes27 → caller remainsB → retire30/bind32 accepted | 37 /8 /2 |
| failed-cache-overwrite /304 | WarmAA valid → new same-slot cache-write14 invalidates before transfer → partial transfer16 and io_error/0 completion17 → publish18 rejected → cache22 rejected → originAA | 33 /7 /1 |
| failed-origin-result /305 | WarmAA → accepted cache child fails io_error/0 after one sample → cache commit13 rejected → origin sampleA completesio_error/1 → caller preservesio_error/1/dataA | 23 /3 /2 |

The first two exact traces must also detect `late_slot_reuse` as `safety_violation/LIVE_LEASE_REUSED` at bind8 and bind7, before assertions. All counts follow oracle-v1 §9.1 evaluation points, not byte counts; cache rejection is not a fabricated successful lineage observation. A/B are known41/42; explicit caller status/count and mask are checked. No timer expires: cancel is a flag and actual child completion is a later independent event.

## Execution

- [x] Add seven named integration tests referencing the five missing fixtures; record RED.
- [x] Add literal events and assertions from the fixed table; require GREEN for five references and two precise mutant detections.
- [x] Record file hashes, clauses, actual counts, replay instructions, limits and any mismatch diagnosis.
- [x] Run workspace/fmt/clippy/planning; independent Astra review of the fixed diff and hand expectations. Root workspace224; reviewer target7 (subset, not additive); five hashes match. Actual reference/mutant CLI exits0/3 inspected.
- [x] Publish bounded PR, verify exact tree, inspect Windows/Linux CI logs and merge normally: PR #78, Windows221/Linux224.
- [x] Synchronize acceptance, Issue #75 and parent roadmap with readback: PR #80, closed/done Issue body/labels read back exactly. No claim of F08 acceptance or actual DMA quiescence.
