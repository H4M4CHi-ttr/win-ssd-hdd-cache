# ADR-007: Rust-first with a bounded WDK escape hatch

Status: D09 build-only artifact independently accepted; final PR integration pending. Related: [D09 #6](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/6). 2026-11-02.

## Context and options

The owner prefers Rust, Tauri for UI, and narrow justified language/SDK exceptions. The Microsoft Rust driver tooling remains early-stage; Rust ownership cannot establish OS callback IRQL, device completion or Windows certification.

| Option | Investigation |
|---|---|
| Rust user-mode/core and Rust WDM entry | First candidate; pinned bindgen/WDK/no_std build, no allocator, no real I/O |
| Same Rust core with a small C17 WDK entry | Build alternative for WDK-specific boundaries; explicit C ABI and `/GS` entry wrapper |
| Move cache policy/host/service/UI into C | Rejected; not required by this experiment or the owner's preference |
| Install an unsigned prototype or disable HVCI to prove the build | Rejected; it changes the safety/authority scope and proves neither ABI nor product suitability |

## Provisional decision

Keep the portable model/core, service/CLI and future GUI backend in Rust. Compare both **build-only** routes in [the isolated experiment](../../experiments/rust-wdk/README.md). Do not select the production entry/async glue until the actual D01/D04/D10 contracts exist. A successful Rust-WDM build supports keeping Rust as first candidate; C fallback is limited to documented OS/WDK glue and cannot grow into policy/ownership code without review.

The experiment's all-integer descriptor has no explicit application pointer; Windows x64 supplies a synchronous caller-owned hidden return buffer for the 24-byte struct, as documented in its README. Safe core forbids unsafe; exports/panic hook/OS entry are small separate crates. Both entries inspect their explicit OS pointer nullness only, never dereference/retain them, never register callbacks, and always return failure. Async ownership, `Send/Sync`, allocation/Drop at IRQL, and storage I/O are absent, not magically safe.

## Evidence

Local Linux: core tests pass in debug/release; PE checker tests pass. The separate Windows [WDK run 41469142668](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/actions/runs/41469142668) passes both image builds, C/Rust ABI 8 vectors, core debug 3/release 3, PE 22 cases and three package signature checks. Kernel C uses `/WX`; no C compiler warnings remain. [D09 evidence](../evidence/D09.md) records exact source/tree/checkout/artifact identities, toolchain hashes, failed runs and the unresolved difference between two runner image-version fields. Ordinary host CI is additional evidence, not a replacement for this job.

## Consequences and revisit

- Rust abort configuration and nonreturning panic hooks forbid cross-FFI unwind by construction in this probe; actual kernel panic is not executed and is never a normal error path.
- The same Rust static library is linked into the host C harness and C shim, but the two panic hooks have different environments. Neither successful host execution nor the WDM compilation verifies kernel execution.
- WDK NuGet headers/libs are fixed and verified; hosted MSVC itself is recorded rather than fully pinned. Reproduction means reconstructable pins plus explicit compiler/image evidence, not guaranteed bit-identical builds.
- `/KERNEL` warning suppression is documented; adding C does not make Rust machine code validated by MSVC's `/KERNEL` checker.
- No INF, signed package or binary release is produced. Signing identity, license, public product release and lab installation remain separate owner gates.
- Revisit on D01 attachment choice, D04 callback/IRQL requirements, missing required Rust binding, allocator need, toolchain upgrade, or actual HVCI/Verifier/HLK evidence. If either build route fails, retain the failure and diagnose it rather than claiming both routes qualified.

## Reviewer

Independent Astra review accepted immutable local source/evidence commit `56b184198bf43e3a595a1c80dbe32c19754e3b4d` for this build-only scope. Source code/workflow/pins are unchanged since the clean Windows proof. Final PR publication/merge and Issue close remain separate, unconfirmed operations; this record does not claim main integration.
