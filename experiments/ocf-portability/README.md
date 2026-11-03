# D03 cache-engine feasibility

Despite the directory's historical name, only an **original Rust** candidate is executable here. OCF is investigated at a pinned upstream source revision, not vendored, built or run. See [ADR-005](../../docs/adr/0005-cache-engine.md), [engine boundary](../../docs/contracts/engine.md) and [measurement record](../../docs/evidence/D03.md).

## Reproduce without devices

From the repository root with the pinned Rust toolchain and existing Cargo dependencies:

```sh
cargo test --locked --offline -p cache-model --test engine_feasibility
cargo test --locked --offline -p cache-model --release --test engine_feasibility -- --nocapture
cargo fmt --all -- --check
cargo clippy --locked --offline --workspace --all-targets -- -D warnings
cargo test --locked --offline --workspace
cargo --offline xtask check-planning
```

Do not share a `CARGO_TARGET_DIR` between worktrees: the planning executable embeds its manifest location. These commands require no administrator, Windows SDK, cache SSD, HDD, raw-disk handle, driver installation or network at runtime. `--offline` assumes dependencies are already installed and reports failure otherwise; it is not a fallback that skips tests.

The test source is [engine_feasibility.rs](../../crates/cache-model/tests/engine_feasibility.rs). It includes [rust_spike.rs](rust_spike.rs) as a test-only module. No new Cargo package/dependency is needed.

## Fixed schedules

| Schedule | Correct expectation | Deliberate wrong policy |
|---|---|---|
| Fill samples old data; origin write completes; fill then completes/writes SSD | Publish rejected | Captures generation after the source read instead of before |
| Two overlapping writes; only the first completes | New fill still rejected until the second actual completion | Boolean writer state clears on the first completion |
| Two-byte write at 4095; only byte4096 actually changes; error reports zero | Both cached lines stay invalid; origin keeps the one applied byte; caller sees error/zero | Invalidates first line only |
| New fill becomes visible; older source later physically overwrites the same slot | Old visibility removed even without rebind; stale publish rejected | Keeps old visibility/ignores physical provenance |

There are 14 tests, including a separate superseded-publication mutant check and two concurrent hit readers retaining the last physical lease after one completes. Other checks cover final hit validation, immutable retained results, full-count/short-coverage cache completion, short/error/unknown source fill, cancellation and epoch invalidation retaining physical leases, exact lower status/count, duplicate completion/commit, transfer overlap, checked ranges, resource limits, counter retirement and zero-length record exhaustion.

The four wrong policies are intentionally executed; their specific required outcome is false. They are not discarded as parser errors. No test uses the candidate's own output to rewrite an expected fixture. This is still a small author-written feasibility harness, not an independent safety oracle or exhaustive state search.

## Measurements and limitations

`D03_HOST_MEMORY` prints actual host `Vec::capacity × size_of` values for one bounded resource-pressure fixture. It is not an RSS or Windows nonpaged measurement, not necessarily the largest reachable allocation, and not an OCF comparison. The model stores fake medium contents in RAM solely for testing.

The experiment uses byte fragments, two 4 KiB lines and two slots; no physical sector/atomicity assumptions are proven. No persistent cache reuse, write-back, flush/FUA, real async callback, MDL, DMA, HVCI, cancellation primitive or Windows I/O stack is present. Windows host CI is separately required before task acceptance; a Linux pass is not a substitute.

## What an actual OCF experiment would require

If ADR-005 is revisited, build the exact pinned upstream commit through a reviewed adapter with allocation instrumentation and fake volumes of supported geometry (cache at least 40 MiB). Explicitly choose WI and volatile metadata. Inject partial-applied origin errors and late read/backfill completions, and observe OCF's own callbacks/metadata effects rather than a hand-written imitation. Compare equal workloads, capacity and queue bounds; include all environment allocations plus buffers, not just metadata footprint. That work, a Windows port and any vendoring are **not performed here**.
