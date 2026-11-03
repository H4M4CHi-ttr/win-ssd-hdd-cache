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

## Subsequent actual acceptance and synchronization

The preceding paragraphs are the pre-publication checkpoint. PR #80 subsequently published head `732e2e85029c570fb9101166612a60ac0077c42b`, reviewed local `ad52f6548a0842d7b2c7b92bee321763a0b769f3`, identical tree `cee6c2f78cbcb494af300abfbd56e234a734054e`. Independent Astra accepted the full18-file metadata diff. Actual run41570940663 passed Windows221/Linux224 named tests (jobs125703418859/125703419195), fmt/clippy/nonzero/planning28/54/59/171. Expected-head normal merge was `b740bd25f268b55a8321ee1a27fd28d031769ee6`.

After that merge, root updated and exactly read back Issue #23/#75 bodies, done labels and closed state; F09 #27 and G01 #29 gained only their F07 accepted dependency, remaining blocked. Parent #1 and P01 #79 were also synchronized and read back. Existing unrelated/user labels were preserved. Before P01 acceptance, all54 task IDs mapped one-to-one to GitHub Issues, with0 agent/review/status-label or open/closed-state mismatches. This comparison is distinct from the pure planning validator; neither grants authorization.

P01 then completed source implementation, independent review and two-OS CI in [PR #81](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/81), normal merge `4cffcbc2a9a5480f1c2ff5740673c307a52d7231`. [Its evidence](P01.md) records root251 Linux tests, independent27-test subset rerun, seven literal-file hashes, exact published tree and actual Windows248/Linux251 run41572371284. The final acceptance manifest is now **54 tasks: done20 / ready0 / blocked33 / needs-decision1**, routing unchanged at Luna17/SOL29/Astra8. Only P01 status/completion/output changes in this final increment; no existing dependency or owner approval changes.

This final increment also fixes the accepted-list omission of F07/F11 and adds a [Windows inventory handoff](../handoff/2026-11-03-windows-next-step.md). That handoff states that capture producer/collector and product driver remain unimplemented, separates ordinary query permissions from lab mutation, and leaves D09's runner-image discrepancy unresolved. Source/build preparation is not inherently prohibited before G1, but the missing probe-specific lifetime/attachment/transport contract is not supplied by P01.

The final metadata increment's own review, CI, merge and P01 Issue readback are subsequent operations, not pre-certified by this document. No real machine was accessed or modified in these portable acceptance steps.

Root pre-review verification of this final metadata worktree: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`, `cargo test --workspace --locked --offline`, `cargo xtask check-planning` and `git diff --check` all passed. Actual full test log contains251 named Linux passes,0 failures/ignored. Planning reports28 labels/54 tasks/62 documents/193 links. A structural manifest comparison found changes only to P01's status, output and completion; all other tasks and every dependency/approval field are unchanged.

The final metadata was then independently accepted and normally merged in PR #82:
head5a85b29f2d610846d89df950941f4b125c60a3e7, tree4aba16a2ede20f73b3440c57c2897df2bcbb198e,
merge bdd85c070f2f22136b17a28d97f157e22c10a367. Run41573233276 passed actual
Windows248/Linux251 named tests (jobs125710747443/125710747588), with zero
failed/ignored. Root fetched main and verified the exact tree. P01 #79 was closed
completed/done and exactly read back; parent #1 and D01 #3 were synchronized.
The full54-task/55-Issue audit found no role/status/state mismatch. Of137 dependency
edges,132 had explicit state annotations; stale W05 #35→D02 and W06 #33→D01
annotations were corrected and read back, leaving zero annotated-state mismatches.
No owner gate or live machine was changed. [P02](P02.md) is the next separately
reviewed preparation task, not a claim that all useful cloud work had ended.

## P02 accepted; P03 bounded format correction

The historical snapshots above are superseded by the [P02 final evidence](P02.md):
PR85 normal merge bcc0dc1cfd0eee1c108956c0157be89fc41bd365, reviewed source tree
7f95922668770a0258cdfb167b0a7125f9057156. Actual host run41580740303 passed
Windows280/Linux283 tests; proof run41580740357 passed WDK object compilation,
17 path cases,56 C cases,20 checker cases and32 optimized Rust tests per OS.
Failed build hypotheses and the exact optional SDK-data amendment remain recorded.

The next reviewed unit is [P03](P03.md), explicit offline capture v2 for unknown
completion Information. This metadata checkpoint records56 tasks:
done21/ready1/blocked33/decision1 and Luna18/SOL30/Astra8. Only P02 acceptance
and the new P03 record change; no pre-existing dependency/owner approval changes.
P03 is a schema correction, not the live producer/collector. Its source work
waits for this reviewed contract checkpoint's publication.

An independent backlog audit found that a future capture-only upper-candidate
probe integration contract is useful pre-G1 work worth investigating. Inventory
is not a blanket prohibition on source/build preparation. Conversely, no existing
blocked product task can simply be relabeled ready: D01/D04/D10/F08/F09/G1 and
lab/owner gates remain. A coherent callback/lifetime/transport contract, rather
than more unrelated fixtures or a mock GUI, must establish the next source scope.

## P03 actual source acceptance

The P03 planning checkpoint was published in PR87 and its implementation then
accepted in [PR88](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/88).
Published heade2a7b1c84f16627b47f936cb300736d7b24a30ea and tree
bff25e6a702e69f30b4f5848903e63bb494193fa passed actual host run41582572440:
Windows296/Linux299 named tests, zero failed/ignored, planning28/56/68/220.
The16 new format/CLI tests executed on each OS. Independent source review
accepted local3b19b97 and the final documentation delta7857a4b. Root normally
merged at17edb16ba14f6f4029c6485cb0cb652fc7cd4bd5 and verified main's exact tree.
[P03 evidence](P03.md) retains the real RED, compatibility witnesses and source
boundaries. Only this offline format was accepted; no actual collector/driver
or Windows runtime qualification was inferred.

P04 is being specified as a complete but never-loaded capture-only upper-candidate
probe and Rust collector. Its limited C/WDK lifecycle exception, exact framing,
finite recorder and same-source host DDI harness need independent acceptance
before source work. This does not change D01/D04/D10/F08/F09/G1 or any existing
owner approval. Public repository and ordinary Actions authorization are still
distinct from permission to attach/install/load or publish a product binary.
