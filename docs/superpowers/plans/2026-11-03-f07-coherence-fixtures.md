# F07 — fixed coherence regressions

> Use superpowers:executing-plans and test-driven-development. The existing D02/D07 contract is the specification; this is not a new cache design.

Goal: implement [F07 #23](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/23) using the accepted F06 replay APIs. F06 was merged in PR #74 with independent review and Windows204/Linux207 host CI. Its acceptance metadata is a separate reviewed work unit. Primary implementation is Rust; this task contains only literal JSON fixtures, Rust integration tests and evidence documentation.

## Scope and fixed expectations

| Fixture | Hand-derived expectation | Normative source |
|---|---|---|
| T01 cold fill/hit | Cold cache reject and origin fallback, completed fill then identical known cache result | validation T01, trace §2/§4.2 |
| T02 partial write | Requested intersecting line invalidated on admission; updated byte read from origin | validation T02, oracle §2/§4 |
| T02 failed partial write | Whole requested line invalidated; only actual fragment changes; exact origin error/count retained | oracle O02/O10 |
| T03 cross-line write | offset4095,length2 invalidates two4096-byte lines; origin returns written bytes | validation T03, half-open range contract |
| T04 stale fill | Old sampledA may physically write but cannot publish after mutationB; fallback returnsB | oracle O11 |
| T04 ABA fill | A→B→A equal bytes do not restore old lineage | trace §2, oracle independent admission history |
| T05 two writers | First actual completion cannot clear second writer; fill/hit reject until both complete | validation T05, oracle O15 |
| T11 discard/rewrite | Discarded bytes unknown; partial rewrite restores only changed byte; unknown source cannot publish | validation T11, oracle O04 |

All normal fixtures drain and finish passed, with explicit JSON assertions and nonzero independent observations. T04 and ABA use the exact same schedule with `stale_fill`; only `safety_violation/STALE_PUBLICATION` counts as detection. Parser rejection, assertion mismatch or panic cannot qualify. Fixture seeds are fixed, but no RNG derives expected values. T03's long initial field is a literal repetition ofA bytes, not a replay-derived output.

## Steps

- [x] Write Rust integration tests referencing the eight missing fixture files; run the specified target and confirm RED for missing artifacts.
- [x] Write fixed JSON events and explicit decisions/results/cache state assertions from the table; no production code changes or contract changes.
- [x] Run `cargo test -p cache-model --test coherence_traces --locked --offline` GREEN. Verify exact assertion counts, meaningful categories and all eight reference outcomes; directly inspect both mutant safety codes.
- [x] Record fixture SHA-256 identities as external file evidence (not silently inserted into model report provenance), exact commands and results in `docs/evidence/F07.md`.
- [x] Run full workspace tests, fmt, clippy and planning. Independent Astra review must confirm fixture expectations rather than merely reproducing implementation output.
- [x] Publish one bounded PR, inspect two-OS Actions logs and merge normally: PR #77, Windows214/Linux217, exact reviewed tree.
- [ ] Synchronize acceptance and Issue #23 with readback in the separate metadata integration.

F08/F09/D04/D10/G1 and all owner/lab gates remain blocked as applicable. F11's five portable-only lifetime fixtures are a different Issue/PR. No real disk/device access, driver installation, Windows API, network in tests, binary release, license or signing action.
