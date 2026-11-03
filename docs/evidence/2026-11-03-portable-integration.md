# Portable acceptance and observation preparation — 2026-11-03 JST

This records completed source acceptance separately from the next preparation contract. No device, driver install/load, signing, license or release operation was performed.

| Task | Source PR / normal merge | Actual host CI |
|---|---|---|
| F06 | [#74](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/74), e3561b4bada7c5b2ca993199e03ce2f1d7609ebe | run41567007805, Windows204 / Linux207 |
| F07 | [#77](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/77), e99a55bbbce2c3400f7d1ccadbe9fbfca02aa132 | run41568873777, Windows214 / Linux217 |
| F11 | [#78](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/78), 0e4ba23692abfdc15b2d56e0462fe3cc65bbf5e3 | run41569745074, Windows221 / Linux224 |

Individual [F06](F06.md), [F07](F07.md) and [F11](F11.md) records identify immutable source/tree/checkouts/jobs, exact hand expectations, review and actual commands. Counts are named passing tests and are not sums of repeated summaries or independent reruns. Windows Server host execution is not Windows11 driver qualification. F07 adds8 literal traces/10 tests; F11 adds5/7. Root implemented; expectation/code reviews were independent.

F06 metadata and F11 scope were separately merged in PR #76 (`5a50d57cd3ff9b19c09b28b006063b70738633e1`), with actual Windows204/Linux207 in corrected run41568153880. Its original failed run and narrow fixed-count correction remain in the boundary audit. Issue #20 and the four affected successor/parent snapshots were read back exactly after that publication.

## Current bounded continuation

[P01 #79](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/79) is a new source-only observation-preparation task. Its [contract](../contracts/capture-envelope-v1.md) was independently accepted at `46f2cd8cb4c869a63a829d372e48c41997c48ce8` after correcting two explicit design findings. Implementation/tests/CLI and their independent acceptance remain unfinished at this checkpoint. No real producer is implemented by adopting a post-collection envelope.

The manifest now records **54 tasks: done19 / ready1 / blocked33 / needs-decision1**. Minimum routing is Luna17 / SOL29 / Astra8. Only F07/F11 were newly accepted; P01 is the only newly ready task. Every pre-existing dependency, owner-approval field and blocked gate is preserved. D01 has no accepted interception position; F08/F09, D04/D10, G1 and owner gates remain unchanged.

Adding P01 made two exact `xtask` repository-baseline tests fail locally: expected53, actual54. The targeted run was15 passed/2 failed. Only those two exact baseline literals were updated to54; validation logic, rejection fixtures, state/approval checks and DAG rules were not changed. This is a manifest fixture update, not relaxing the checker to tolerate unknown task counts.

The metadata review also fixes stale current-status wording in ADR005/ADR007/ADR017 and the index. Historical failures and then-pending publication statements remain explicitly historical; their successful integration is supported by the [Phase 2 record](2026-11-03-phase2-integration.md), not inferred from a closed Issue.

At local metadata checkpoint `feefc787440f0e4f1f8492936cf336e48504d6a2`, root actually ran `cargo test --locked --offline --workspace`:224 Linux tests passed, none failed/ignored. Fmt, clippy (`--workspace --all-targets --locked --offline -- -D warnings`), planning28 labels/54 tasks/59 documents/171 links and `git diff --check` passed. Both formerly failing exact-count regressions passed.

This metadata increment itself still requires independent review, normal publication/two-OS CI/merge and Issue synchronization. Those later results are not asserted here in advance. P01 source may use the reviewed contract on its explicit branch, but must not presume an unpublished artifact is on main.
