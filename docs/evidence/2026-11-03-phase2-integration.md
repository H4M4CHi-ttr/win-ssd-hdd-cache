# Phase 2 publication and acceptance — 2026-11-03 JST

The owner asked to resume the interrupted GitHub operations. All four task trees were compared exactly against their preserved independently reviewed local tips before publication. Normal expected-head merges were used, without force/admin bypass. No device, driver-load, signing, license or release action was performed.

## Published and merged

| Task | PR | Published head | Merge commit |
|---|---|---|---|
| D09 | [#68](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/68) | 710547c3da9f677bb61743e789d6c7d9b99ed1dc | 5342a95a394d36e763077c5c7eb27f79645dc768 |
| F05 | [#70](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/70) | 67e4737c59759c49890a52d638a21ae5c126ba34 | 04edf32bfdb2db87142b61664b92f6a9ab4c408c |
| D07 | [#71](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/71) | 17602f00be313252e01e75f8df75095454c8b086 | 262f838c0c1eb3374175a1da09f29dabad18f747 |
| D03 | [#72](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/pull/72) | d9ef9d2217a67943ae86def3d79b54f1bcaefbc8 | 11cd3be50703802145c7a63590e3bfcf770843c5 |

Reviewed source/checkpoint and limitations remain in [F05](F05.md), [D03](D03.md), [D09](D09.md) and [D07 ADR](../adr/0017-test-oracle.md). F05 includes the post-review enum-map regression fix. D07 O01–O18 are hand-derived specification examples, not executed F06 tests.

## Fresh Actions evidence

Each host job passed format, lint, nonzero-test guard, test and planning checks. Counts below are actual named passing tests, not the doubled summary lines printed again by the workflow's result guard. Windows has platform-specific test selection.

| Task | Run | Windows job / tests | Linux job / tests |
|---|---|---|---|
| D09 host | [41561687994](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/actions/runs/41561687994) | 125673534310 / 100 | 125673534495 / 102 |
| F05 | [41561791234](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/actions/runs/41561791234) | 125673864185 / 129 | 125673864392 / 131 |
| D07 | [41561795071](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/actions/runs/41561795071) | 125673876853 / 100 | 125673876512 / 102 |
| D03 | [41561798093](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/actions/runs/41561798093) | 125673886592 / 114 | 125673886110 / 116 |

D09 build-only [run 41561687986](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/actions/runs/41561687986), job 125673534323, also completed successfully: core debug3/release3, PE precheck22, C-to-Rust ABI8 literal vectors. This rerun does not upgrade the evidence to kernel runtime/IRQL/HVCI qualification. Its actual PR merge checkout was ebade1c282944103f1fac91693962812bf530744.

Before publication, the retained combined local checkpoint ca2e2db480f9437ff7fa1f095a36b75b4238457b again passed Linux workspace145 tests, fmt, clippy and planning (28 labels / 52 tasks / 50 documents / 123 links). The final merged-source metadata PR is checked independently; do not misattribute a pre-merge run to a later commit.

## Acceptance and next gate

D03/D07/D09/F05 artifacts are accepted and merged. The backlog now has done16 / ready1 / blocked34 / needs-decision1. F06 is ready because D07/F05 and its earlier F03/F04 dependencies have both artifacts and required evidence. F06 implementation and F07/F08/F09 qualification are still unfinished.

D01 still requires separately authorized disposable-Windows-lab traces. D04/D10, G1 and product driver development retain their dependency gates. Owner license/contribution/release/signing decisions remain separate. Public repository visibility and ordinary Actions authorization do not grant those authorities.

GitHub issue bodies/labels are synchronized after this metadata commit is published and verified; this document does not claim that an unexecuted future sync has completed.
