# D09 reproducible build experiment toolchains

2026-11-02. This is a build investigation, not a supported-product matrix.

| Component | Selection / provenance |
|---|---|
| Rust | 1.99.1, edition 2024; x86_64-pc-windows-msvc; panic=abort; no LTO in the experiment |
| Windows runner | GitHub `windows-2022` for VS 2022; mutable runner image/version and exact MSVC binary versions/hashes must be recorded each run |
| MSVC | VS 2022 major 17 selected with vswhere, x64 host/target; not claimed bit-for-bit compiler-pinned |
| WDK, SDK neutral, SDK x64 NuGet | each 10.0.26100.6584; kit directory version 10.0.26100.0; all three exact archive digests in [pins.json](../experiments/rust-wdk/tools/pins.json) |
| LLVM/libclang | 17.0.6 x64, digest checked, dedicated runner-temporary installation |
| wdk-build / wdk-sys / wdk-panic | 0.6.0 / 0.5.2 / 0.5.0; exact package selections and transitive registry checksums in the experiment Cargo.lock |
| wdk-macros | must match wdk-sys 0.5.2 through the committed lockfile |
| wdk-alloc | not included; no allocator is needed by this probe |

The Microsoft packages were downloaded from NuGet and SHA-256 measured locally. All three signature verifications succeeded in the separate Windows WDK job; the presence of a `.signature.p7s` file alone is not verification. WDK-only package roots lack `shared` headers, so the experiment merges all three packages under one temporary kit root, refusing unequal duplicate files. It does not fall back to an arbitrary installed SDK header directory for kernel bindgen.

Normal registry dependency use with upstream licenses/checksums is permitted by the project contract; no third-party source is vendored here. This does not choose a license for this repository. The Microsoft Rust driver project and the used wdk crates use MIT OR Apache-2.0; inspect Cargo.lock and registry metadata again when upgrading. LLVM and Microsoft SDK/WDK have their own upstream terms, not the project's future source license.

## Primary sources

- [windows-drivers-rs at 04523040779d80125f280c4f9421f54a6059f470](https://github.com/microsoft/windows-drivers-rs/tree/04523040779d80125f280c4f9421f54a6059f470): early-stage warning, WDM example, static CRT config, wdk-build linker behavior, panic implementation.
- [Microsoft WDK downloads](https://learn.microsoft.com/en-us/windows-hardware/drivers/download-the-wdk): SDK/WDK compatibility and VS-generation requirements. A newer kit is not assumed interchangeable.
- [LLVM 17.0.6 release](https://github.com/llvm/llvm-project/releases/tag/llvmorg-17.0.6).
- [Microsoft winget LLVM manifest](https://github.com/microsoft/winget-pkgs/blob/master/manifests/l/LLVM/LLVM/17.0.6/LLVM.LLVM.installer.yaml): x64 installer digest cross-check, not a floating download selection.
- [GitHub Windows 2025 runner manifest](https://github.com/actions/runner-images/blob/main/images/windows/Windows2025-Readme.md): run-time image/compiler versions are logged rather than inferred from this mutable page.
- [GitHub Windows 2022 runner manifest](https://github.com/actions/runner-images/blob/main/images/windows/Windows2022-Readme.md): selected VS 2022 build environment after the first run below.

The first [WDK run 41466549825](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/actions/runs/41466549825) failed before any compiler/package operation: `windows-2025` actually selected `windows-2025-vs2026` image 20261021.278.1, so the deliberately constrained VS 17 lookup returned no installation. The old Windows2025 README still described VS 17 and was insufficient evidence of label resolution. The workflow now requests `windows-2022`, whose manifest contains VS 2022, and records the actual inventory. The VS major constraint and SDK pins are unchanged; no silent compiler-generation fallback is allowed. This does not change the product target to Windows Server 2022.

Upgrade any pin in a separate reviewed change with fresh debug/release core, ABI, compile/link, PE and future lab evidence. A new Rust compiler or runner image passing this experiment does not silently expand the supported OS matrix.

## Observed build, not an image lock

[D09 evidence](evidence/D09.md) records the clean run `41469142668`, compiler/linker versions and hashes, both image hashes and all prior failures. Its job setup reports image version `20261025.346.1`, while the same run's `$env:ImageVersion` captured in its artifact is `20261026.347.1`. Artifact identity/source were independently verified; the cause of this metadata discrepancy remains unresolved. Keep both source-labelled values rather than inventing an explanation or claiming bit-identical reproduction. Rust's own LLVM 23.1.1 is distinct from the pinned libclang 17.0.6 used by bindgen.
