# Portable continuation boundary — 2026-11-03 JST

Independent Astra read-only audit inspected all unfinished entries at source checkpoint `5f4d9613c40b2ffe4e73505d300592edd2ec5f12`. No tests, GitHub changes, devices or Windows operations were performed by that audit. F06 was subsequently accepted and merged as [PR #74](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/74); see [its execution evidence](evidence/F06.md).

## Finding

F07 is the next ready **existing** work item after F06. The present graph then reaches D01's real capture-boundary evidence. That is a valid gate for the existing Windows-mapped work, but not a technical prohibition on more bounded portable testing.

- [ADR-017](adr/0017-test-oracle.md) rejects waiting for real hardware before all model testing and separates portable oracle from Windows mapping.
- [The earlier Issue audit](issue-audit-2026-11-02.md) deliberately retained D04/D10 for F08 and G1. Do not erase that decision to make more issues appear ready.
- [Trace v1](contracts/trace-v1.md) already specifies cancel/epoch versus physical leases, immutable fallback results, failed slot overwrites and lower error preservation. F06 exercises them as implementation tests; reusable named JSON fixtures can add repeatability without inventing Windows semantics.

## Bounded follow-up adopted: F11

[F11 #75](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/75) depends on accepted F06/D02/D07. Its five small fixtures cover cancel-held lease, epoch-held lease, fallback followed by late cache completion, failed cache overwrite visibility and failed origin result preservation. A hand-reviewed table fixes clauses, seeds, order, decisions/results/terminal and observation categories. The two lease fixtures must pass the reference and detect `late_slot_reuse` as `safety_violation/LIVE_LEASE_REUSED` on the same trace. Input rejection, assertion mismatch, panic or inconclusive outcomes cannot stand in for detection.

Use separate `tests/fixtures/portable-lifetime/`, `crates/cache-model/tests/portable_lifetime_traces.rs` and `docs/evidence/F11.md`; do not occupy F08's acceptance paths. This is a new bounded test unit, not “F08 complete except paperwork.” **F08/F09/D04/D10/G1 statuses and dependencies remain unchanged. F11 does not substitute for F08 in F09.**

New events/faults/allocator models, actual timer/timeout claims or changed expectations require separate contract review. Logical budget exhaustion is not real allocation-failure/OOM injection. The five cases introduce none of those. Qualification flags stay false, and null CLI provenance is not silently filled from a shell HEAD. Independent review and ordinary two-OS host CI are required before acceptance.

## Genuine remaining boundaries

| Family | Missing prerequisite |
|---|---|
| D04/D05/D06/D10 | D01-selected, observed Windows interception/ownership/target/control boundary |
| F08/F09/B03 | Windows ownership/fault mapping, mapped fixture acceptance and candidate/budget evidence |
| C05/C06 | Frozen target/control contracts, even though later functions can be pure |
| W01–W10/G02/C04/C07/C08 | G1 and later Windows lifecycle, endpoint, service and preparation gates |
| U01–U03 | Existing live control/service/G02 scope, not an offline GUI mock |
| Q01–Q04 | Driver/lab/hardware, performance and human release acceptance |
| GOV01/D08 | Owner license/contribution/signing/distribution decisions |

Once F07 and F11 are complete, this finite portable continuation does not unlock the existing engine/Windows chain. D01 needs separately authorized disposable-lab correlation traces; its position is not selected. No live-disk or driver permission is inferred from the owner's ordinary Actions authorization. [Validation gates](validation.md) and [D01 ADR](adr/0004-interception-boundary.md) remain authoritative.
