# 一次資料と確認メモ

設計調査の基準日: 2026-11-02 JST。署名・対応build・release・crate/toolchainは実装/配布時に再確認する。以下は上流資料であり、本プロジェクトのWindows対応実績ではない。

## OCF / cache

- [OCF repository](https://github.com/Open-CAS/ocf) — 再利用候補の本体、上流test、license。採用時にcommitをpin。
- [What is OCF](https://open-cas.com/concepts/what-is-ocf/) — portability/環境ラッパー。
- [OCF quick start](https://open-cas.com/ocf/quick-start/) — standaloneのWindows製品ではない。
- [Cache configuration](https://open-cas.com/concepts/cache-configuration/) — WI/WA/WT/WBの違い。2026-11-02再確認。
- [I/O engines](https://open-cas.com/ocf/design/io-engines/) — read/backfill、discard、flush等。Windowsへの対応付けは別検証。
- [OCF license](https://github.com/Open-CAS/ocf/blob/master/LICENSE.md) — BSD-3-Clause通知の確認対象。
- [Linux bcache](https://docs.kernel.org/admin-guide/bcache.html) / [dm-cache](https://docs.kernel.org/admin-guide/device-mapper/cache.html) — Linux依存とbacking/cacheの参考設計。

## Windows I/O / testing

- [Storage filter driver I/O support](https://learn.microsoft.com/en-us/windows-hardware/drivers/storage/storage-filter-driver-s-support-of-i-o-requests) — dispatch/PnP/power等。
- [Synchronizing IRP cancellation](https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/synchronizing-irp-cancellation) — cancel/completionの競合。
- [IRP handling examples](https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/different-ways-of-handling-irps-cheat-sheet) — 所有権/完了の検討。
- [Memory-mapped files and filters](https://learn.microsoft.com/en-us/windows-hardware/drivers/ifs/memory-mapped-files-in-a-file-system-filter-driver) — paging I/Oを無視しない。
- [Data set management](https://learn.microsoft.com/en-us/windows-hardware/drivers/storage/data-set-management-overview) — TRIM等の制御経路。
- [BypassIO for storage](https://learn.microsoft.com/en-us/windows-hardware/drivers/storage/bypassio) — 現在の対象範囲とveto。SATA HDDへの全I/Oが既にこれで迂回すると誤解しない。
- [Windows file caching](https://learn.microsoft.com/en-us/windows/win32/fileio/file-caching) — RAM cacheとの比較の必要性。
- [Driver Verifier](https://learn.microsoft.com/en-us/windows-hardware/drivers/devtest/driver-verifier) — 開発専用環境で使用。
- [HVCI compatible code](https://learn.microsoft.com/en-us/windows-hardware/drivers/driversecurity/implement-hvci-compatible-code) — memory/実行権限の要件。

## Rust / GUI

- [Microsoft windows-drivers-rs](https://github.com/microsoft/windows-drivers-rs) — kernel Rust候補。2026-11-02に公式READMEを再確認。成熟度の注意を踏まえ、本番採用はD09で決める。
- [Microsoft windows-rs](https://github.com/microsoft/windows-rs) — userspace Windows API binding候補。
- [Rust Nomicon: FFI](https://doc.rust-lang.org/nomicon/ffi.html) — unsafe/FFIの参考。device lifetimeを自動保証するものではない。
- [Tauri security](https://v2.tauri.app/security/) — frontend/Rustのtrust boundary。2026-11-02再確認。
- [Tauri capabilities](https://v2.tauri.app/security/capabilities/) — GUI権限の最小化。
- [Serde container attributes](https://serde.rs/container-attrs.html) — strict typed deserialization、unknown fieldの扱い。

## Distribution

- [Driver signing offerings](https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/driver-signing-offerings) — 2026-11-02再確認。attestation testing / HLK / preproduction / retail WUを区別。
- [Hardware Program registration](https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/hardware-program-register) — 署名主体/EV/事業情報等の確認対象。
- [Code signing requirements](https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/code-signing-reqs) — 公開前に最新版を確認。
- [HLK prerequisites](https://learn.microsoft.com/en-us/windows-hardware/test/hlk/getstarted/windows-hlk-prerequisites) — 対象releaseの正確な環境/playlistを再確認。

## Alternatives

- [WinFsp NTFS compatibility](https://github.com/winfsp/winfsp/wiki/NTFS-Compatibility) — userspace FSと既存NTFS block cacheを同一視しない。
- [WinSpd](https://github.com/winfsp/winspd) — 仮想disk方式の参考、成熟度/配布条件は別確認。
- [OpenZFS on Windows](https://github.com/openzfsonwindows/openzfs) — ZFS移行を伴う代替。
- [OpenZFS pool concepts](https://openzfs.github.io/openzfs-docs/man/master/7/zpoolconcepts.7.html) — L2ARC等の概念。

この一覧は研究の入口であり、第三者コードを本repoへ取り込む許可の代わりではない。
