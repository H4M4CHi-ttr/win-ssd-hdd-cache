# D09 — build-only Rust/WDK experiment

Research image only. **No driver installation/loading, no disk/device I/O, no INF/catalog/signing, no Verifier, and no security-setting changes.** Both entries return failure for every input. The workflow uploads diagnostic text/JSON/map files, not `.sys`/`.exe` binaries. There is no product release here.

## Scope and fixed ABI

The separate Cargo workspace contains a `no_std`, `forbid(unsafe_code)`, allocator-free pure core; a tiny `extern C` static-library wrapper; and a Rust WDM entry. The ordinary repository host workspace does not acquire WDK dependencies or flags.

`win_cache_probe_range(u64 offset, u64 length, u64 capacity) -> RangeReply` uses the x64 Windows C ABI. `RangeReply` is `repr(C)` with `u32 status`, `u32 reserved`, `u64 start`, `u64 end`: size 24, alignment 8, offsets 0/4/8/16. All fields are initialized. Status 0 is a checked half-open range, 1 is offset past capacity, 2 is length past capacity (offset check first). Error ranges and reserved are zero. Zero length at EOF is valid. No bool/enum/reference/explicit application pointer appears in the source-level signature; see the hidden return-storage qualification below.

Ownership is synchronous by value: no explicit application pointer, retained pointer, callback, async operation, mutable global, allocation, lock, blocking, or OS-backed Drop. The Windows x64 ABI lowers this 24-byte return value to a compiler-generated hidden pointer to caller-owned return storage. That buffer is exclusively used for this synchronous call and is never retained. The C executable validates that lowering against the real Rust library. This is a deliberately smaller contract than the future D04 ABI, not its implementation. Fixed-size code that performs no allocation/wait does **not** prove real kernel IRQL validity.

The export wrapper's only unsafe boundaries are its globally unique exported name and the nonreturning C panic hook. The host executable supplies `abort`; the C kernel shim supplies `KeBugCheckEx`. This lets the **same Rust static library** link into both proofs without pretending the host executable executes a kernel panic. Both implementations have no parameters and never unwind/return. The Rust WDM path separately uses pinned `wdk-panic` (bugcheck on panic). A panic is a fatal defect, not a recovery strategy. Ordinary inputs use checked arithmetic and do not panic.

## Tests and commands

```sh
cd experiments/rust-wdk
cargo test -p wdk-probe-core --test contract --locked
cargo test -p wdk-probe-core --test contract --release --locked
cargo clippy -p wdk-probe-core --all-targets --locked -- -D warnings
node tools/test-pe.mjs
```

Linux Rust 1.99.1: three core tests pass in each profile (layout, eight literal vectors, 51,200 bounded range triples using a widened independent arithmetic oracle). The initial core test failed on missing core exports before implementation. The manifest setup failure is not counted as that RED. PE precheck tests initially failed on a missing checker; a review regression then exposed unvalidated delayed imports. After rejecting that entire unsupported directory, 22 cases pass. Windows build/ABI evidence is **pending** until an actual Actions result is recorded.

`tools/build.ps1` runs only in an ephemeral Windows Actions runner. It installs the pinned LLVM **compiler**, not a kernel driver. It checks three NuGet archive SHA-256 values plus NuGet signature validation, merges versioned kit content with collision-digest checks, runs host core tests and the C→Rust eight-vector executable, and compiles/links both research images. C uses `/GS` plus WDK `GsDriverEntry`/`BufferOverflowK.lib`. Its return value depends on the Rust call, and a map check requires the Rust export to survive linking; merely passing a `.lib` filename is not evidence of mixed-language execution.

The PE checker requires AMD64/native, nonzero executable entry, 4096-byte section alignment, NX/dynamic-base/integrity flags, no W+X section, no certificate directory, and only `ntoskrnl.exe`/`hal.dll` imports. This conservative check is neither a general PE parser nor an HVCI/Windows loader certification. Compiler/linker hashes, runner image, source SHA, logs, headers/imports/disassembly and map are recorded. `/IGNORE:4257` matches the upstream Rust/MSVC `/KERNEL` code-generation limitation; C glue does not make Rust object code `/KERNEL`-validated.

## Not established

No supported Windows 11 build, actual loadability, HVCI/Secure Boot compatibility, pageable/nonpaged allocation, callback IRQL, IRP/MDL/DMA lifetime, cancel/rundown, power management, real crash recovery, storage throughput, driver signing or HLK/WHCP qualification. Those require the separate D01/D04/D05/D08/D10 and Windows/lab gates. Never run these images on a useful machine to fill a missing CI checkbox.

See [toolchains](../../docs/toolchains.md) and [ADR-007](../../docs/adr/0007-rust-kernel-boundary.md).
