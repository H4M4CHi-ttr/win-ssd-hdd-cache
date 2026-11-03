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

### Metadata regression correction

The first PR #76 host run, `41567714712`, failed two `xtask` tests on both OSes: their exact repository baseline still expected 52 tasks after F11 made the manifest contain 53. The standalone planning command had passed but did not execute those tests. The same two failures were reproduced locally before editing. Only those two fixed expectations were updated to 53; exact-count assertions, malformed-input tests, dependencies, approval validation and production checker logic were not relaxed. Full workspace tests must pass on the corrected snapshot before this metadata PR is accepted.

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

## Follow-up: missing capture-preparation artifact

F07/F11 were subsequently accepted and merged in PR #77/#78. A further independent Astra audit found that `experiments/stack-trace/` still contains only its plan: **there is no D01 capture producer, collector or install script**. D09's build-only entries always fail and are not observation probes. Thus the next handoff is not simply “run an existing driver in Codex.”

The audit identified one bounded useful continuation before lab access: **P01 #79**, a [post-collection capture envelope and completeness checker](contracts/capture-envelope-v1.md). Missing records/correlations/loss counters must not be mistaken for full mutation coverage. It preserves unknowns and only reports internal completeness; producer/auth/coverage/kernel flags are always false. It does not invent real Windows timing, decode arbitrary IOCTL/SRB buffers, select attachment or define kernel transport/ABI.

P01 contract checkpoint `dd9ef8f` received two must-fix review findings: duplicate submit target ownership and undefined range units. Both were fixed by specifying immutable first-submit identity and checked target-relative byte units (otherwise null), with three hand-derived correlation counterexamples. Independent Astra accepted `46f2cd8cb4c869a63a829d372e48c41997c48ce8` before implementation. P01 is a separate task, not a substitute dependency for D01/D04/D10/F08/F09/G1.

The kernel producer still needs its own reviewed context/IRQL/forward/completion/rundown/attachment/loss-transport design. Pre-G1 source/build preparation is not inherently forbidden, but this envelope alone does not supply that design. Actual traces additionally need specified disposable Windows11 targets/build/stack, explicit operation permissions and rollback; a model or green CI cannot supply these facts. Read-only inventory on an authorized machine can inform that next slice; it is not mutation-coverage evidence.
