# D09 reproducible build experiment toolchains

2026-11-02. This is a build investigation, not a supported-product matrix.

| Component | Selection / provenance |
|---|---|
| Rust | 1.99.1, edition 2024; x86_64-pc-windows-msvc; panic=abort; no LTO in the experiment |
| Windows runner | GitHub `windows-2025`; mutable runner image/version and exact MSVC binary versions/hashes must be recorded each run |
| MSVC | VS 2022 major 17 selected with vswhere, x64 host/target; not claimed bit-for-bit compiler-pinned |
| WDK, SDK neutral, SDK x64 NuGet | each 10.0.26100.6584; kit directory version 10.0.26100.0; all three exact archive digests in [pins.json](../experiments/rust-wdk/tools/pins.json) |
| LLVM/libclang | 17.0.6 x64, digest checked, dedicated runner-temporary installation |
| wdk-build / wdk-sys / wdk-panic | 0.6.0 / 0.5.2 / 0.5.0; exact package selections and transitive registry checksums in the experiment Cargo.lock |
| wdk-macros | must match wdk-sys 0.5.2 through the committed lockfile |
| wdk-alloc | not included; no allocator is needed by this probe |

The Microsoft packages were downloaded from NuGet and SHA-256 measured locally. Signature verification is an additional Actions step; the presence of a `.signature.p7s` file alone is not verification. WDK-only package roots lack `shared` headers, so the experiment merges all three packages under one temporary kit root, refusing unequal duplicate files. It does not fall back to an arbitrary installed SDK header directory for kernel bindgen.

Normal registry dependency use with upstream licenses/checksums is permitted by the project contract; no third-party source is vendored here. This does not choose a license for this repository. The Microsoft Rust driver project and the used wdk crates use MIT OR Apache-2.0; inspect Cargo.lock and registry metadata again when upgrading. LLVM and Microsoft SDK/WDK have their own upstream terms, not the project's future source license.

## Primary sources

- [windows-drivers-rs at 04523040779d80125f280c4f9421f54a6059f470](https://github.com/microsoft/windows-drivers-rs/tree/04523040779d80125f280c4f9421f54a6059f470): early-stage warning, WDM example, static CRT config, wdk-build linker behavior, panic implementation.
- [Microsoft WDK downloads](https://learn.microsoft.com/en-us/windows-hardware/drivers/download-the-wdk): SDK/WDK compatibility and VS-generation requirements. A newer kit is not assumed interchangeable.
- [LLVM 17.0.6 release](https://github.com/llvm/llvm-project/releases/tag/llvmorg-17.0.6).
- [Microsoft winget LLVM manifest](https://github.com/microsoft/winget-pkgs/blob/master/manifests/l/LLVM/LLVM/17.0.6/LLVM.LLVM.installer.yaml): x64 installer digest cross-check, not a floating download selection.
- [GitHub Windows 2025 runner manifest](https://github.com/actions/runner-images/blob/main/images/windows/Windows2025-Readme.md): run-time image/compiler versions are logged rather than inferred from this mutable page.

Upgrade any pin in a separate reviewed change with fresh debug/release core, ABI, compile/link, PE and future lab evidence. A new Rust compiler or runner image passing this experiment does not silently expand the supported OS matrix.
