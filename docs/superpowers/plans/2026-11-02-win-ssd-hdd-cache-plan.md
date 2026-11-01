# Windows SSD/HDD Cache Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 既存データHDDを原本とするWindows向けclean SSD read cacheを、Rust主体で安全に段階実装する。

**Architecture:** kernel data path、選定cache engine、Rust管理service/CLI、独立したoffline modelを分離する。必要なGUIはTauri + Vue/TypeScript。Windows核心部分は実証後、Rust no_stdまたは薄いC/WDK shimを局所採用する。

**Tech Stack:** Rust edition 2024のhost workspace（F01でstable exact pin）、Tauri（U01でexact pin）、WDK/必要なC17（D09でexact pin）、OCF WI候補（D03で採否/commit pin）。

**Spec:** [正式設計](../specs/2026-11-02-win-ssd-hdd-cache-design.md)、[契約](../../contracts.md)、[検証](../../validation.md)。

## Global Constraints

- 主要実装はRust、GUIはTauri。低レベルの言語/SDK例外はADRで理由と範囲を残す。
- HDDが唯一の原本。初期はclean read cache / Write-Invalidate、write-backなし、cache永続再利用なし。
- INV-01〜12、docs/contracts.mdの数値/型/受入条件を守る。
- 対応は実証したWindows 11 x64専用data disk構成だけ。現在対応済みbuildはない。
- hostモデル/純粋型はunsafe・実disk・Windows API・管理者操作・networkを使わない。
- kernel/FFI/権限/整合性の未知をLunaに推測させない。
- blocked/owner decisionの前提を解決するまで依存taskを実装しない。
- 実機/driver install/初期化/証明書購入/公開は別途明示されたlab/owner承認の範囲だけ。

このplanは実装依頼ではない。今回の成果物は設計・Issue・ラベルであり、製品codeはまだない。冒頭のsuperpowers手順は利用可能な開発エージェント環境で使う補助手順。外部contributorに特定AI製品を必須とせず、[AGENTS.md](../../../AGENTS.md)と[担当ルール](../../agent-routing.md)をリポジトリの正本とする。

## Review Focus

- T06/T08: cancel/timeout後の遅延completionがcaller bufferを再利用しないこと — F08/W03/Q01。
- T07: epoch切替後、旧SSD writeが再利用slotを物理上書きしないこと — F08/W04/W07/Q01。
- T09/T22: 通常write以外のmutation/raw/control経路の取りこぼし — D01/W06。
- T10/T13: flushの追越しとorigin errorの成功化を防ぐ — D07/W05/Q01。
- T14/T15/T19: identity再利用/unknown属性/再開/対象外boot時の誤判定 — D05/C05/W07/Q02。

## ファイル境界と実装順

[contracts.md §1](../../contracts.md)のworkspace構成に従う。各IssueのFilesはplanned path。C/WDK採否が未決のpathは、D09が決定してIssue本文を更新するまで実装しない。

- 最初にF01。D01/D02/D09の調査は並行可能。GOV01は所有者判断。
- F01後、F02/F03/B01/C01等を進める。cache-model新設はF03、F04はその後。同じCargo.toml/lib.rs追加は短い統合順で調整する。
- D02/D04/D07を凍結してからF05〜F09。G1前にkernel本実装を始めない。
- W01 passthrough → W02〜W07局所実装 → W08 shadow → G2。
- W09 lab hit-serving → Q01/G3 → Q02/Q03 → Q04/G4。
- U01〜U03は管理API安定後の独立branch。GUI未完成はCLIのlab検証を妨げない。

## 検証コマンドの約束

F01後に使えるhost commands: `cargo fmt --all -- --check`、`cargo clippy --all-targets --locked -- -D warnings`、`cargo test --locked`。個別test targetは下記とIssue本文に記載。WDK/HLK/GUI commandはそれぞれD09/D08/U01がpinした環境で具体化する。存在しないcommandを実行済みと書かない。

## 課題別計画

詳細な依存と最新Issueリンクは[バックログ](../../backlog.md)、machine-readable snapshotは[backlog.json](../../backlog.json)。次の契約はIssue本文にも埋め込み、Issueだけを渡されても禁止範囲/期待結果がわかるようにする。

### GOV01: OSSライセンス・外部貢献条件・公開範囲を所有者と確定する

**担当 / review:** SOL / ASTRA  
**Depends on:** なし  
**Files (planned):** `docs/adr/0011-license-and-governance.md`, `LICENSE-MIT (採択時のみ)`, `LICENSE-APACHE (採択時のみ)`, `CONTRIBUTING.md`, `SECURITY.md`

**Interfaces**
- Consumes: 本設計の固定契約と docs/contracts.md / INV-01〜12。
- Produces: MIT OR Apache-2.0を提案として比較し、original codeの採択・第三者通知・DCO/CLA要否・公開承認・private reporting責任者を所有者の決定として記録。採択前にLICENSEを追加しない。

**受入条件**
- 候補licenseとOCF BSD-3-Clause等の通知義務の対応表を作る
- 所有者が未決なら明示的に保留し、外部コード導入/公開をgateで止める
- 公開範囲変更や証明書購入はこのIssueを起票しただけで実行しない

- [ ] 先行契約と一次資料を読み、決める問い・代替案・停止条件を記録する。
- [ ] 受入条件の反例/小さなlab-only検証を設計し、未検証を明示する。
- [ ] Evidence付きADR/契約/schemaを作成し、後続taskのConsumes/Producesへ反映する。
- [ ] 指定reviewを通し、commit/PRと採択/保留の根拠をIssueへ残す。owner decisionは所有者が決める。

**Escalation:** 権利帰属・license互換性・署名主体が不明なら所有者/適切な専門家へ。

### D01: WindowsのI/O捕捉位置と全変更経路の対応表を決める

**担当 / review:** ASTRA / ASTRA  
**Depends on:** なし  
**Files (planned):** `docs/adr/0004-interception-boundary.md`, `docs/contracts/io-coverage.md`, `experiments/stack-trace/README.md`

**Interfaces**
- Consumes: 本設計の固定契約と docs/contracts.md / INV-01〜12。
- Produces: storage/volume/disk filter候補の実stack図、request分類→capture/reject/quiesce+bypass表、未捕捉経路の非対応条件。lab-onlyのtrace/probe手順と証拠。

**受入条件**
- 通常/paging/mapped/raw physical write、DSM/TRIM、zero/offload、pass-through、resize/reformat、BypassIOの到達経路を一次資料と使い捨てlab traceで確認
- 対象外OS disk、未設定状態、boot時のglobal registrationの影響を図と試験で説明
- 未知potential mutationをcache有効のまま通さない停止条件と、原本を変更しないattach経路を固定

- [ ] 先行契約と一次資料を読み、決める問い・代替案・停止条件を記録する。
- [ ] 受入条件の反例/小さなlab-only検証を設計し、未検証を明示する。
- [ ] Evidence付きADR/契約/schemaを作成し、後続taskのConsumes/Producesへ反映する。
- [ ] 指定reviewを通し、commit/PRと採択/保留の根拠をIssueへ残す。owner decisionは所有者が決める。

**Escalation:** 捕捉できないmutationが1つでも残るならsupportedとしない。方式変更/対象縮小をADRで判断する。

### D02: clean-cacheの整合性モデル・世代・公開条件を反例付きで凍結する

**担当 / review:** ASTRA / ASTRA  
**Depends on:** なし  
**Files (planned):** `docs/adr/0013-coherence-model.md`, `docs/contracts/trace-v1.md`, `spec/trace-v1.schema.json`, `experiments/coherence/README.md`

**Interfaces**
- Consumes: 本設計の固定契約と docs/contracts.md / INV-01〜12。
- Produces: D02 state machine、read/write線形化条件、active-writer数・epoch・slot所有権、trace v1の型/イベント/期待値。有限状態モデルで反例を説明。

**受入条件**
- read A→write B完了→late fill A、2 overlapping writers、partial overlap、failed writeで古いhitを禁止する公開条件を固定
- cache data writeとmetadata publish、cancelとcomplete、原本apply/completion/durabilityを別event化し全正常/異常JSON例を示す
- INV-03/04に対して単にinvalidate-before-write/epoch変更だけでは失敗する反例を残す

- [ ] 先行契約と一次資料を読み、決める問い・代替案・停止条件を記録する。
- [ ] 受入条件の反例/小さなlab-only検証を設計し、未検証を明示する。
- [ ] Evidence付きADR/契約/schemaを作成し、後続taskのConsumes/Producesへ反映する。
- [ ] 指定reviewを通し、commit/PRと採択/保留の根拠をIssueへ残す。owner decisionは所有者が決める。

**Escalation:** Windowsが保証しないmulti-sector atomicityを仮定せず、許容する並行readの結果集合を明記する。

### D03: OCF WIと最小Rust coreを比較しcache engineの採否を決める

**担当 / review:** SOL / ASTRA  
**Depends on:** D02  
**Files (planned):** `docs/adr/0005-cache-engine.md`, `docs/contracts/engine.md`, `experiments/ocf-portability/README.md`

**Interfaces**
- Consumes: D02の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: OCF upstream commit/license、必要env wrapper、WI意味論、memory計測、RustとのFFI一覧、OCF vs 独自最小Rust clean-cacheのGo/No-Go。kernel言語採否はD09と分離。

**受入条件**
- WIとWAを混同せずD02の反例をcandidate harnessで再現する
- cacheline/sector pair、nonpaged想定memory、async callback、cancel、allocator依存を列挙して未対応を明記
- OCF No-Go時の代替、試験再利用、後続W02/F09契約更新をADRに記す。第三者コードを取り込む前にlicense条件を確認

- [ ] 先行契約と一次資料を読み、決める問い・代替案・停止条件を記録する。
- [ ] 受入条件の反例/小さなlab-only検証を設計し、未検証を明示する。
- [ ] Evidence付きADR/契約/schemaを作成し、後続taskのConsumes/Producesへ反映する。
- [ ] 指定reviewを通し、commit/PRと採択/保留の根拠をIssueへ残す。owner decisionは所有者が決める。

**Escalation:** upstreamの動作を推測で包まない。C/Rust ownershipやlicenseが不明ならAstra/所有者判断まで止める。

### D04: IRP・MDL・DMA・FFIの所有権とcancel/timeout契約を固定する

**担当 / review:** SOL / ASTRA  
**Depends on:** D01, D02, D03, D09  
**Files (planned):** `docs/contracts/async-ownership.md`, `docs/contracts/ffi-v1.md`, `docs/adr/0014-async-lifetime.md`

**Interfaces**
- Consumes: D01, D02, D03, D09の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: caller/child IRP、MDL、private buffer、slot、device、Rust/C handleごとのowner/最終解放者/callback IRQL。wireとFFIを別定義。resource数値上限とlock順。

**受入条件**
- timeout→HDD fallback→caller完了→late SSD completeでもcaller bufferに触れないmock順序図
- 旧SSD write未完でslotを再利用しない、cancel要求で解放しない、複数writerの参照回収を示す
- panic/unwind/OOM、invalid C enum/length、Send/Sync、Drop可能IRQL、low-memory/queue-full/flush順序を明文化

- [ ] 先行契約と一次資料を読み、決める問い・代替案・停止条件を記録する。
- [ ] 受入条件の反例/小さなlab-only検証を設計し、未検証を明示する。
- [ ] Evidence付きADR/契約/schemaを作成し、後続taskのConsumes/Producesへ反映する。
- [ ] 指定reviewを通し、commit/PRと採択/保留の根拠をIssueへ残す。owner decisionは所有者が決める。

**Escalation:** Pin/Arc/RAIIをdevice completionの代用にしない。unbounded waitを有限回復として書かない。

### D05: 対象identity・暗号化境界・power・解除の状態遷移を固定する

**担当 / review:** SOL / ASTRA  
**Depends on:** D01, D04  
**Files (planned):** `docs/contracts/target-policy.md`, `docs/contracts/lifecycle.md`, `docs/adr/0015-target-lifecycle.md`, `spec/target-v1.schema.json`

**Interfaces**
- Consumes: D01, D04の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: TargetDescriptorの三値属性、安定identity/sector/range、allow/reject理由、TOCTOU再検証、DISABLED〜REMOVINGとPnP/powerの対応。

**受入条件**
- origin/cache同一媒体、OS/pagefile/dump、RAID/Storage Spaces、暗号化/geometry不明をfail-safe拒否
- disk number/letter再利用、clone/resize/reformat、sleep/hibernate、cache喪失、drain不能の期待遷移を記す
- cache partition専有と非再帰経路、cold reset、原本保持のuninstall/recovery手順を固定

- [ ] 先行契約と一次資料を読み、決める問い・代替案・停止条件を記録する。
- [ ] 受入条件の反例/小さなlab-only検証を設計し、未検証を明示する。
- [ ] Evidence付きADR/契約/schemaを作成し、後続taskのConsumes/Producesへ反映する。
- [ ] 指定reviewを通し、commit/PRと採択/保留の根拠をIssueへ残す。owner decisionは所有者が決める。

**Escalation:** 必要属性が取得不能ならfalseとして許可せず非対応にする。曖昧なdisk消去/初期化は所有者へ。

### D06: 管理サービス・IOCTL・IPCの認可とversioned ABIを設計する

**担当 / review:** SOL / ASTRA  
**Depends on:** D04, D05  
**Files (planned):** `docs/contracts/control-v1.md`, `spec/control-v1.schema.json`, `docs/adr/0016-control-security.md`

**Interfaces**
- Consumes: D04, D05の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: Rust service/CLI/後期Tauri間のlocal IPC、kernel binary ABI、ACL、read-only statusとmutation権限、size/opcode/rate制限と固定golden vectors。

**受入条件**
- 一般user/偽peer/不正size/unknownversion/opcode/overflow/reservedbitsの拒否結果を具体化
- raw disk/任意IOCTL/pointerを公開せず、service側でtargetを操作直前に再検証
- Tauri非特権、remote contentなし、管理service停止とI/O経路分離、diagnostics allowlistを確認

- [ ] 先行契約と一次資料を読み、決める問い・代替案・停止条件を記録する。
- [ ] 受入条件の反例/小さなlab-only検証を設計し、未検証を明示する。
- [ ] Evidence付きADR/契約/schemaを作成し、後続taskのConsumes/Producesへ反映する。
- [ ] 指定reviewを通し、commit/PRと採択/保留の根拠をIssueへ残す。owner decisionは所有者が決める。

**Escalation:** frontendの確認だけに安全性を依存させない。暗号化層/権限の未確定はAstraへ。

### D07: 整合性・flush耐久性・故障注入のoracleを定義する

**担当 / review:** ASTRA / ASTRA  
**Depends on:** D02, D04  
**Files (planned):** `docs/contracts/oracle-v1.md`, `spec/evidence-v1.schema.json`, `docs/adr/0017-test-oracle.md`

**Interfaces**
- Consumes: D02, D04の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: completed/flushed/volatile write、TRIM unknown、許容read結果、seed replay、fault注入点、証拠schemaとバグ検出の対照条件。

**受入条件**
- 通常write completionをpower-loss durabilityと誤認しないoracleを小例で示す
- stale-fill/early ACK/late-slot overwriteの故障modelが各試験で必ず失敗する条件を定義
- 原本failureとcachefailure、同時変更中shadow比較、未観測をPASSにしないルールを定義

- [ ] 先行契約と一次資料を読み、決める問い・代替案・停止条件を記録する。
- [ ] 受入条件の反例/小さなlab-only検証を設計し、未検証を明示する。
- [ ] Evidence付きADR/契約/schemaを作成し、後続taskのConsumes/Producesへ反映する。
- [ ] 指定reviewを通し、commit/PRと採択/保留の根拠をIssueへ残す。owner decisionは所有者が決める。

**Escalation:** oracleとengineが同じ誤ったロジックを共有しないよう独立review。証拠不足ならgateを閉じない。

### D08: 署名・HLK・HVCI・配布/更新の実行可能性を確認する

**担当 / review:** SOL / ASTRA  
**Depends on:** D01, D09  
**Files (planned):** `docs/adr/0010-distribution.md`, `docs/release-readiness.md`, `docs/lab/signing-plan.md`

**Interfaces**
- Consumes: D01, D09の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: testing/preproduction/HLK-WHCP/retail WUを分けた経路、対象build/playlist、署名主体/費用区分、HVCI/SecureBoot、rollbackのchecklist。

**受入条件**
- その時点のMicrosoft一次資料で要件を再確認しURL/dateを記録
- attestationがload可能なことと製品配布条件を区別し、普通の利用者にHVCI/SecureBoot無効化を要求しない
- 証明書/主体/lab所有者/専門家reviewの未決はowner decisionとして残し、購入や鍵登録を勝手にしない

- [ ] 先行契約と一次資料を読み、決める問い・代替案・停止条件を記録する。
- [ ] 受入条件の反例/小さなlab-only検証を設計し、未検証を明示する。
- [ ] Evidence付きADR/契約/schemaを作成し、後続taskのConsumes/Producesへ反映する。
- [ ] 指定reviewを通し、commit/PRと採択/保留の根拠をIssueへ残す。owner decisionは所有者が決める。

**Escalation:** アカウント資格/契約/費用の確定に新権限が必要なら調査成果を先に残して所有者判断を求める。

### D09: Rust kernel/WDKの適合性とC shimの最小境界を決める

**担当 / review:** SOL / ASTRA  
**Depends on:** なし  
**Files (planned):** `docs/adr/0007-rust-kernel-boundary.md`, `docs/toolchains.md`, `experiments/rust-wdk/README.md`

**Interfaces**
- Consumes: 本設計の固定契約と docs/contracts.md / INV-01〜12。
- Produces: Rust no_std/WDK候補の正確なversions、build/link/panic/allocator/IRQL/HVCI検証計画。必要な場合だけC17 WDK shimの責務とRust境界を採択。

**受入条件**
- windows-drivers-rs公式成熟度注意と実際の対応構成を確認し、compileだけでproduction可としない
- user-mode/service/modelはRustを維持し、kernel側の例外は理由/範囲/再検討条件をADR化
- FFI unwind不可、allocation/DropのIRQL、C/Rust ABI/layout、toolchain再現性を小さなlab-only proofで確認

- [ ] 先行契約と一次資料を読み、決める問い・代替案・停止条件を記録する。
- [ ] 受入条件の反例/小さなlab-only検証を設計し、未検証を明示する。
- [ ] Evidence付きADR/契約/schemaを作成し、後続taskのConsumes/Producesへ反映する。
- [ ] 指定reviewを通し、commit/PRと採択/保留の根拠をIssueへ残す。owner decisionは所有者が決める。

**Escalation:** Rust優先と安全/WDK制約が衝突したら無理に全面Rust化せず、狭い代替をAstraレビューで決める。

### F01: Rust workspaceとchecked byte-range/line-spanを実装する

**担当 / review:** LUNA / SOL  
**Depends on:** なし  
**Files (planned):** `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `crates/cache-types/Cargo.toml`, `crates/cache-types/src/lib.rs`, `crates/cache-types/src/range.rs`, `crates/cache-types/tests/range_contract.rs`

**Interfaces**
- Consumes: 本設計の固定契約と docs/contracts.md / INV-01〜12。
- Produces: docs/contracts.md §1/2のByteRange/LineSpan/RangeError/LineSize、checked_range/line_span。host Rustだけ、forbid(unsafe_code)、stable exact pin、edition2024。

**受入条件**
- line_span(4095,2,8192,K4)=(0,2)、EOF zero-length成功、u64::MAX境界でwrapしない失敗テストを先に追加
- cargo test -p cache-types --test range_contract が1件以上を実行してPASS。cargo fmt --all -- --check / cargo clippy -p cache-types --all-targets -- -D warnings がPASS
- Windows/Linux hostで再現可能なcommandとtoolchain/MSRVを記録。未使用crate、driver、C/FFI、networkを追加しない

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** sector alignmentや製品I/O semanticsをhelperへ加える必要があればSOLへ。共有Cargo manifestの同時変更は調整する。

### F02: ラベルとバックログの静的整合性を検査するxtaskを作る

**担当 / review:** LUNA / SOL  
**Depends on:** F01  
**Files (planned):** `xtask/Cargo.toml`, `xtask/src/main.rs`, `xtask/src/labels.rs`, `xtask/src/backlog.rs`, `xtask/tests/repository_contract.rs`, `.cargo/config.toml`

**Interfaces**
- Consumes: F01の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: validate_label_manifestとcargo xtask check-planning。labels.json、docs/backlog.jsonのID/依存DAG/label/ready条件/相対doc linkを読み取り専用で検査。

**受入条件**
- 正本manifest/backlogをPASS。duplicate name、unknownprefix、duplicateID、missingdep、cycle、依存未完readyのfixtureをFAIL
- 違反は契約どおり(code,path)でstable sortし、入力配列順を変えても同じ結果
- cargo test -p xtask / cargo xtask check-planning がPASS。GitHub API・token・自動修正/label削除を一切使わない

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** taxonomyやready判定契約を変えないと通らない場合、fixture/期待値を緩めずSOLへ。

### F03: cancelとcompletionを分ける決定的fake-I/Oキューを実装する

**担当 / review:** LUNA / SOL  
**Depends on:** F01  
**Files (planned):** `crates/cache-model/Cargo.toml`, `crates/cache-model/src/lib.rs`, `crates/cache-model/src/completion.rs`, `crates/cache-model/tests/completion_contract.rs`

**Interfaces**
- Consumes: F01の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: docs/contracts.md追加契約のCompletionQueue/RequestToken/Completion。reserve、request_cancel、deliver。host-only、unsafe/thread/clock/I/Oなし。

**受入条件**
- reserve→cancel→deliverでpayload保持、cancel_requested=true。B/Aの逆順completeを明示配送できる
- 二重deliver/未知token拒否、cancel冪等、token非再利用、pending1024/payload1MiB/total16MiBの上限を不変条件付きで検査
- cargo test -p cache-model --test completion_contract がPASS。cancelをrequest解放/完了として扱わない

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** Windows DMA/MDLやIoCancelIrpをこのhost queueに導入しない。実機安全性と混同しそうならSOLへ。

### F04: discardをunknownとして扱うin-memory originを実装する

**担当 / review:** LUNA / SOL  
**Depends on:** F01, F03  
**Files (planned):** `crates/cache-model/src/origin.rs`, `crates/cache-model/tests/origin_contract.rs`

**Interfaces**
- Consumes: F01, F03の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: docs/contracts.md §3のMemoryOrigin::new/read/write/discard。1MiB上限、byte-known map、checked ranges。F03後に共通crateを拡張。

**受入条件**
- 4095から2bytes writeは指定範囲だけ変える。不正write/discard前後で状態不変
- discardをゼロにせずUnknownRange。部分再writeでその部分だけknownへ戻る。zero lengthは成功
- cargo test -p cache-model --test origin_contract がPASS。flush、threads、disk、durabilityを追加しない

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** 並行性/永続化の期待値が必要ならD02/D07へ。仕様にないsector atomicityを仮定しない。

### F05: 凍結済みtrace v1のstrict Rust parserを実装する

**担当 / review:** LUNA / SOL  
**Depends on:** F01, D02  
**Files (planned):** `crates/cache-types/src/trace.rs`, `crates/cache-types/tests/trace_format.rs`, `tests/fixtures/trace-format/`

**Interfaces**
- Consumes: F01, D02の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: D02が確定したTraceV1/TraceErrorとparse_trace(&str)。typed serdeで解析するだけでeventを実行しない。

**受入条件**
- D02の正常/異常golden JSONを再利用。unknownfield/version/event、duplicatekey、bool/float/negative整数欄、末尾別documentを拒否
- model1MiB/events10000/payload16MiBの境界ちょうどと+1を検査し、過大入力をallocation前に制限
- cargo test -p cache-types --test trace_format がPASS。エラーはcode/index、元payloadを不用意に出力しない

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** event意味論やexpected valueが未定義なら実装せずD02へ。JSON Value変換で重複keyを消さない。

### F06: traceを再生し独立oracleと照合するRust harnessを作る

**担当 / review:** SOL / ASTRA  
**Depends on:** D07, F03, F04, F05  
**Files (planned):** `crates/cache-model/src/replay.rs`, `crates/cache-model/src/oracle.rs`, `crates/cache-model/tests/replay_contract.rs`, `crates/cache-model/examples/replay.rs`

**Interfaces**
- Consumes: D07, F03, F04, F05の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: D02/D07の凍結契約に従うReplayResult/run_trace。seed/replay order、known/unknown/allowed outcomes、completed vs flushedを記録するoffline CLI。

**受入条件**
- 同じtrace/seedで結果・順序・errorが再現し、wall clock/thread schedulerに依存しない
- 故障model stale-fill/early ACK/late-slot overwriteがそれぞれFAILし、正しいreference modelがPASS
- cargo test -p cache-model --test replay_contract と指定fixture replayがPASS。oracleをcandidateengineのロジックで代用しない

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** D02/D07の矛盾をharness内で勝手に解決しない。期待結果集合の追加はAstraレビュー。

### F07: 基本WI・partial write・複数writer・stale-fillの回帰traceを追加する

**担当 / review:** LUNA / ASTRA  
**Depends on:** F06  
**Files (planned):** `tests/fixtures/coherence/`, `crates/cache-model/tests/coherence_traces.rs`

**Interfaces**
- Consumes: F06の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: D02で承認された順序/期待結果だけをJSON trace化。T01〜T05/T11の小さな固定seed再現セット。

**受入条件**
- cold fill/hit、4095境界write、2 overlappingwritersの片方完了、readA→writeB→latefillAを各1fixture以上
- discard/partialrewriteはunknownを含むrangeの値を断定しない。古いcacheを受理する故障modelに対してFAIL
- cargo test -p cache-model --test coherence_traces がPASS。fixture期待値の根拠をD02へリンク

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** どのread結果が許容されるか迷ったらAstraへ。runnerの結果に合わせて期待値を書き換えない。

### F08: late completion・slot再利用・resource faultの回帰traceを追加する

**担当 / review:** LUNA / ASTRA  
**Depends on:** F06, D04  
**Files (planned):** `tests/fixtures/lifetime/`, `tests/fixtures/faults/`, `crates/cache-model/tests/lifetime_traces.rs`

**Interfaces**
- Consumes: F06, D04の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: D04/D07の固定反例をtrace化。cancel≠complete、epoch≠quiesce、cache/origin failureを別fixtureで表現。

**受入条件**
- timeout→fallback→caller完了後のSSD完了、epoch変更後の物理slot遅延writeを再現
- pending cap/low memory/slotABA/二重complete/failed originwriteの不変条件と資源回収を確認
- cargo test -p cache-model --test lifetime_traces がPASSし、対応する意図的故障modelにはFAIL

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** fake queueが実DMA停止を証明したと解釈しない。期待値や所有権が不明ならAstraへ。

### F09: 選定engineをuser-mode harnessで独立oracleと対照試験する

**担当 / review:** SOL / ASTRA  
**Depends on:** D03, D09, F06, GOV01, F07, F08  
**Files (planned):** `crates/cache-engine/Cargo.toml`, `crates/cache-engine/src/host_adapter.rs`, `crates/cache-engine/tests/differential.rs`, `docs/evidence/engine-selection.md`

**Interfaces**
- Consumes: D03, D09, F06, GOV01, F07, F08の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: D03選定engineのhost adapter。OCFならupstream pin/notice/最小FFI、独自Rustなら同じ公開contract。kernel I/Oなし。

**受入条件**
- F07/F08相当のtraceを独立oracleと比較し違反0、injectedbugで差分検出
- line/sector候補とmemory/CPU/fill量を計測して予算決定資料を残す
- cargo test -p cache-engine --test differential とreplay commandsを記録。Cが必要な場合だけtoolchainを隔離

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** OCF GoとRust kernel Goを混同しない。hostで通ってもWindows適合完了にしない。

### G01: 設計契約とoffline modelのG1ゲートをレビューする

**担当 / review:** ASTRA / ASTRA  
**Depends on:** D01, D02, D03, D04, D05, D06, D07, D08, D09, F07, F08, F09, F10  
**Files (planned):** `docs/gates/g1-design-model.md`, `docs/contracts/`, `docs/backlog.json`

**Interfaces**
- Consumes: D01, D02, D03, D04, D05, D06, D07, D08, D09, F07, F08, F09, F10の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: INV-01〜12→contract→testのcoverage表、未決ゼロ/明示非対応、証拠manifest、独立review。G1合否と後続ready候補を記録。

**受入条件**
- 状態/ABI/type/IRQL/limitsが文書間で一致し、Luna Issueに未定義判断が漏れていない
- stale-fill/earlyACK/latephysicalwriteの故障modelが検出され、oracleが独立
- 不合格/未検証ならW01以降をblockedに保つ。closeだけでなくレビュー証拠を残す

- [ ] 依存Issueのartifact/test/reviewer証拠を一覧化する。
- [ ] 受入条件を1つずつ照合し、反証・未検証条件を記録する。
- [ ] 作者とは別のreviewerがGo/No-Goを記録する。未完なら後続はblocked。
- [ ] gate文書とbacklog snapshotをcommit/PRにし、人間の承認が必要な部分は代行しない。

**Escalation:** モデルで捕捉不能な物理/OS前提がある場合、kernel進行条件/非対応範囲を再設計する。

### F10: 秘密・管理者操作なしのRust host CIを構成する

**担当 / review:** LUNA / SOL  
**Depends on:** F02  
**Files (planned):** `.github/workflows/host-tests.yml`, `docs/development.md`, `xtask/tests/workflow_policy.rs`

**Interfaces**
- Consumes: F02の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: Linux/Windows hostのfmt/clippy/tests/planning検査。Cargo default-membersはportable hostのみ。trusted kernel/lab/signingとは分離。

**受入条件**
- contents:read、secretsなし、pull_request_targetなし、self-hostedなし、raw disk/driver install/署名/formatコマンドなしを静的fixtureで検査
- actionsは確認したimmutable SHA、Rustはrepo exact toolchain、Cargo.lockを使う。test0件や失敗をcontinue-on-errorで通さない
- cargo fmt --all -- --check / cargo clippy --all-targets --locked -- -D warnings / cargo test --locked / cargo xtask check-planning を両hostで実行し記録

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** WDK/HLK/管理者/署名鍵が必要なら別trusted pipelineとしてSOLへ。PR jobへ権限を足さない。

### W01: 専用lab向けpassthrough-only driverを構築する

**担当 / review:** SOL / ASTRA  
**Depends on:** G01  
**Files (planned):** `drivers/windows/README.md`, `drivers/windows/ (D01/D09採択のproject/INF/entry files)`, `tests/windows/passthrough/`

**Interfaces**
- Consumes: G01の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: D01のattach方式、D09のRust優先/必要C shimを使う最小driver。cache hit/fillなし、通常read/write/controlを下位契約どおり転送する。

**受入条件**
- 固定WDK/toolchainでbuildしINF/static checksを通す。署名はlab-only手順
- disposable targetでbaselineと成功/error/bytes/orderを対照。targetなし/対象外OS diskをcache化しない
- load/unload/install失敗時の回復、global registrationの影響を確認。実データには導入しない

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** 対象外diskやbootへ影響したら直ちに止めD01/D05へ。C shim境界を無断拡大しない。

### W02: 選定cache engineとWindows境界を最小FFIで接続する

**担当 / review:** SOL / ASTRA  
**Depends on:** W01, D03, D04, F09  
**Files (planned):** `crates/cache-engine/src/windows_adapter.rs`, `crates/cache-engine/src/ffi.rs`, `drivers/windows/engine_bridge/`, `tests/windows/ffi/`

**Interfaces**
- Consumes: W01, D03, D04, F09の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: D03/D04/D09のopaquehandle/固定幅ABI、callback/allocator、所有権。OCFならCの既存engineを限定接続、独自Rustなら不要FFIを作らない。

**受入条件**
- invalid enum/length/callback order、panic/unwind、allocation failureで契約どおり失敗
- unsafe/Send/SyncごとにSAFETY根拠、最終解放者とIRQLをレビュー
- host differential再実行とWindows mock completion試験を通し、hitはまだ利用者へ返さない

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** Rust安全wrapperが実際にはaliasing/IRQLを隠す場合はAstraへ。FFI越境panicを許容しない。

### W03: owned bufferと非同期I/O rundownを実装する

**担当 / review:** SOL / ASTRA  
**Depends on:** W01, D04  
**Files (planned):** `drivers/windows/io/`, `tests/windows/io_lifetime/`

**Interfaces**
- Consumes: W01, D04の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: D04 ownership表に沿うchild I/O/privatebuffer/MDL/ref/rundown、boundedqueue、cancel/timeout/low-memory。

**受入条件**
- T06/T08/T16/T21: timeout後fallback、latecomplete、二重cancel/remove、queuefullでUAF/二重completion/無制限allocationなし
- foreground完了後callerbufferをfillが参照せず、privatebufferはdeviceownership終了後だけ解放
- mock順序の全検査と専用labのVerifier計画/結果を記録。spinlock下のI/O待ちをレビュー

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** cancel/drainが確認不能ならbypass固定/参照維持の契約に戻り、強制freeで隠さない。

### W04: 専有SSD cache backendとslot再利用障壁を実装する

**担当 / review:** SOL / ASTRA  
**Depends on:** W03, D05  
**Files (planned):** `drivers/windows/cache_backend/`, `tests/windows/cache_backend/`

**Interfaces**
- Consumes: W03, D05の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: 専有cachepartitionだけへのbounded reads/fills、slot incarnation、checksum、停止中slot隔離、raw backendエラー処理。format/partition変更は別C07。

**受入条件**
- T07/T12: oldphysicalwrite未完slotを再利用しない。checksum mismatch/read/write/full/readonlyエラーでhit拒否
- origin/cache overlapと自己再帰を拒否し、partialsector/sectorpairはD05allowlist外なら拒否
- 所有者承認済みdisposable partitionとmockdeviceで検証。origin HDDへcachemetadataを書かない

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** devicehangからのquiesceを証明できなければ同一領域再開禁止。CRC一致でfreshnessを保証しない。

### W05: write-invalidateと原本write/flush順序を実装する

**担当 / review:** SOL / ASTRA  
**Depends on:** W01, D02, D04  
**Files (planned):** `drivers/windows/mutation/write_flush/`, `tests/windows/write_flush/`

**Interfaces**
- Consumes: W01, D02, D04の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: D02のrangegeneration/activewriters、partial/failedwrite失効、D04のHDDcompletion/barrier契約。writeback経路なし。

**受入条件**
- T02〜T05/T10/T13: multiplewriter、failedpartialwrite、保留writeをflushが追い越さない、error/bytesを保持
- payloadをcacheだけに保存してACKせず、FUA/write-through/flushを下位へ正しく渡す
- 固定traceとpassthroughbaselineで結果比較し、stale-fill故障モデルがFAILすることを再確認

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** 原本orderingとcachepublishの線形化点が合わないならAstraへ。性能優先でbarrierを省略しない。

### W06: DSM・raw/control・未知mutationの安全な処理を実装する

**担当 / review:** SOL / ASTRA  
**Depends on:** W01, D01, D05  
**Files (planned):** `drivers/windows/mutation/control/`, `tests/windows/mutation_coverage/`

**Interfaces**
- Consumes: W01, D01, D05の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: D01request表どおりのcapture/reject/quiesce+bypass。DSM/TRIM/offload/zero/passthrough/resizeおよびBypassIOの診断。

**受入条件**
- T09/T22の各経路で監視漏れとstalehitがない。unknownpotentialmutationをcache有効で単純転送しない
- discard後のbytesをゼロと仮定しない。拒否/unsupported codeを意図どおり伝える
- 現在のBypassIO適用範囲を超える対応を宣伝せず、非対応時は診断可能なveto

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** captureできない経路を見つけたらD01のsupport条件を縮める/方式を改める。

### W07: attach・power・disable・removeのcold-reset lifecycleを実装する

**担当 / review:** SOL / ASTRA  
**Depends on:** W01, W03, D05, C05  
**Files (planned):** `drivers/windows/lifecycle/`, `tests/windows/lifecycle/`

**Interfaces**
- Consumes: W01, W03, D05, C05の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: D05の状態遷移、target再検証、newwork停止/rundown、coldreset、degraded固定、解除/回復。

**受入条件**
- T14/T15/T19/T20: sleep/hibernate/offlineonline/交換/resize/no-target/driver撤去で旧cache再利用なし
- epochだけでslotを再利用せず、旧I/O未完なら再開拒否。serviceにdata-pathの進行を依存させない
- 対象外boot、uninstall失敗、再起動必要状態とmanualrecoveryを専用labで記録

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** 対象誤認/未知powertransition/参照漏れはAstraへ。原本formatで回復しない。

### W08: 原本だけを返すshadow cache観測を統合する

**担当 / review:** SOL / ASTRA  
**Depends on:** W02, W03, W04, W05, W06, W07  
**Files (planned):** `drivers/windows/shadow/`, `tests/windows/shadow/`, `docs/lab/shadow.md`

**Interfaces**
- Consumes: W02, W03, W04, W05, W06, W07の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: cache候補hit/fillの検証・counterを行うが、利用者へ返すreadは常にHDD。valid世代だけを比較するlabmode。

**受入条件**
- shadowでSSD由来bytesがcaller結果として使われないことをtest hookで確認
- concurrentwrite中の別世代比較を誤ってcorruptionとしない一方、故障cacheは検出
- cache/service停止/queuefullでも原本結果が変わらず、overheadと資源量を記録

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** shadowの比較根拠/同世代条件に未定義があればD02/D07へ。hit提供を先行しない。

### G02: passthrough・shadowのG2ゲートを独立レビューする

**担当 / review:** ASTRA / ASTRA  
**Depends on:** W08, C04, C05, C06  
**Files (planned):** `docs/gates/g2-shadow.md`, `docs/lab/support-matrix.md`

**Interfaces**
- Consumes: W08, C04, C05, C06の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: shadow証拠・target拒否・controlsecurity・no-targetboot・cache喪失/解除のreview。lab hit-serving許可範囲。

**受入条件**
- shadowでcacheからcallerへbytesを返していない証拠とbaseline比較を確認
- D01coverageの全経路、D04ownership、servicecrash、unknown属性拒否を実証
- fail/未検証ならW09はblockedのまま。限定したlabdevice/buildだけに許可

- [ ] 依存Issueのartifact/test/reviewer証拠を一覧化する。
- [ ] 受入条件を1つずつ照合し、反証・未検証条件を記録する。
- [ ] 作者とは別のreviewerがGo/No-Goを記録する。未完なら後続はblocked。
- [ ] gate文書とbacklog snapshotをcommit/PRにし、人間の承認が必要な部分は代行しない。

**Escalation:** 証拠欠落を『たぶん安全』で埋めない。人間の実機承認は別に確保する。

### W09: 検証済みlab構成でのみread-cache hit提供を有効にする

**担当 / review:** SOL / ASTRA  
**Depends on:** G02, B03  
**Files (planned):** `drivers/windows/read_path/`, `tests/windows/hit_serving/`, `docs/lab/active-mode.md`

**Interfaces**
- Consumes: G02, B03の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: ACTIVE状態でepoch/range/slot/ownershipを検証したcleanhitだけ返し、cachefailureはoriginへfallback。明示opt-in以外disabled。

**受入条件**
- T01〜T08/T12/T13: cachehitbytes一致、latefill/ABA拒否、cacheerrorfallback、観測したoriginerrorを成功化しない
- D04/B03予算内でlowmemory/timeout時動作を検証、cache fill失敗で成功したorigin readを失敗へ変えない
- G3/Q01へ渡すtrace/evidenceを作り、署名済み/本番対応とはまだ表示しない

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** データ差異、UAF、origin/cache対象混同は即停止。性能のため安全条件を緩めない。

### C01: offline status v1のRust型とstrict JSON codecを実装する

**担当 / review:** LUNA / SOL  
**Depends on:** F01  
**Files (planned):** `crates/cache-types/src/status.rs`, `crates/cache-types/tests/status_format.rs`, `tests/fixtures/status/`

**Interfaces**
- Consumes: F01の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: docs/contracts.md §6のStatusSnapshot/StatusError、parse_status。counterはJSONではdecimal string/null、Rust内部u64/Option、固定state/mode/errorcode。

**受入条件**
- 全7statesをroundtrip。nullと0、u64::MAXを精度損失なく区別する
- unknown/duplicatefield/version/state/mode、leadingzero/negative/float/boolのcounter、入力65536B超を拒否
- cargo test -p cache-types --test status_format がPASS。serdeの生errorや入力値を公開diagnosticsへ出さない

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** 新state/writebackmode/livecontrolの追加が必要ならD06/SOLへ。ここでkernel ABIを発明しない。

### C02: offline statusを表示するRust CLIを作る

**担当 / review:** LUNA / SOL  
**Depends on:** C01  
**Files (planned):** `crates/cache-cli/Cargo.toml`, `crates/cache-cli/src/main.rs`, `crates/cache-cli/src/status.rs`, `crates/cache-cli/tests/status_cli.rs`, `examples/status/`

**Interfaces**
- Consumes: C01の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: cache-cli status --input <file> --format json|text。render_status、状態/counter/固定error説明の表示だけ。service/driverへ接続しない。

**受入条件**
- 全statesのgolden出力、未取得と0の区別、unknown version拒否
- 正常0/schemaerror2/fileI/Oerror3。activeを『安全性保証済み』と表示しない
- cargo test -p cache-cli --test status_cli がPASS。network/admin/disk APIなし

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** live接続・権限変更・state遷移を表示taskへ追加しない。D06/C04へ分離する。

### C03: 診断情報をallowlistだけで出力するRust関数を実装する

**担当 / review:** LUNA / ASTRA  
**Depends on:** C01  
**Files (planned):** `crates/cache-types/src/diagnostics.rs`, `crates/cache-types/tests/diagnostics_allowlist.rs`

**Interfaces**
- Consumes: C01の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: docs/contracts.md §6のpublic_diagnostics。strict inputから既定status/counter/errorcodeだけを新しい型へコピーし元payloadを出力しない。

**受入条件**
- path/username/serial/content/任意nested秘密sentinelを含む入力は拒否され、出力/errorにもsentinelが出ない
- allowlist正常fixtureだけ成功し、型違いをstringifyして漏らさない
- cargo test -p cache-types --test diagnostics_allowlist がPASS。regexで任意textを洗う方式に置換しない

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** 復旧診断に識別子追加が必要ならD06でprivacy/権限/保存期間を再設計。

### C04: Rust管理サービスと最小権限IPCを実装する

**担当 / review:** SOL / ASTRA  
**Depends on:** D06, C01, W01  
**Files (planned):** `crates/cache-service/Cargo.toml`, `crates/cache-service/src/ipc.rs`, `crates/cache-service/src/inventory.rs`, `crates/cache-service/src/driver_control.rs`, `tests/windows/service/`

**Interfaces**
- Consumes: D06, C01, W01の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: D06 localIPC/ACL/authz、Windows inventory収集とunknown保持、driver status/controlの固定adapter。データ面と非同期に分離。

**受入条件**
- 非認可caller/偽peer/不正message/rate超過を拒否。statusと管理操作の権限を区別
- D05 descriptorの各重要属性の取得証拠を残し、不明をfalseとして許可しない
- service crash/restart/hangでも原本I/O経路がservice待ちにならない。CLI live接続のmock/実lab試験を通す

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** OS属性の信頼性、TOCTOU、privilege scopeが不明ならD05/D06へ。必要以上のrawIOを公開しない。

### C05: 凍結済みtarget descriptorの純粋な許可/拒否判定器を作る

**担当 / review:** LUNA / ASTRA  
**Depends on:** F01, D05  
**Files (planned):** `crates/cache-types/src/target.rs`, `crates/cache-types/tests/target_policy.rs`, `tests/fixtures/targets/`

**Interfaces**
- Consumes: F01, D05の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: D05で固定したTargetDescriptor/PolicyViolationとvalidate_target_pair。収集済み値だけを判定、Windows API/実diskを呼ばない。

**受入条件**
- 完全なallowlist組合せだけPASS。critical unknown/missing属性は固定codeで拒否
- OS/pagefile/dump/暗号化/複合storage、origincache同一媒体、geometry未知/不一致、ID再利用条件を固定fixtureで拒否
- cargo test -p cache-types --test target_policy がPASS。fieldを取得する実装はC04へ分離

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** descriptorに判定に必要な情報がなければD05へ。存在しない情報を推測しない。

### C06: control ABIの固定golden vectorsと不正入力回帰を追加する

**担当 / review:** LUNA / ASTRA  
**Depends on:** F01, D06  
**Files (planned):** `tests/fixtures/control/`, `crates/cache-types/src/control_vectors.rs`, `crates/cache-types/tests/control_vectors.rs`

**Interfaces**
- Consumes: F01, D06の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: D06で承認したbyte layout/version/length/opcode/reservedbitsのoffline decoderとvectors。IOCTL発行なし。

**受入条件**
- 正常vectorsの全field/endianness一致、短buffer/length不一致/unknownversion/opcodeを拒否
- reservedbits、u64範囲/overflow、paddingの異常を固定expectederrorで検証
- cargo test -p cache-types --test control_vectors がPASS。Rust native layoutをwireとしてmemcpyしない

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** C/Rust structとwireが食い違う、互換変換が必要ならD06/Astraへ。

### C07: cache専用partitionのdry-run準備と明示確認を実装する

**担当 / review:** SOL / ASTRA  
**Depends on:** D05, C04, C05  
**Files (planned):** `crates/cache-service/src/provision.rs`, `crates/cache-cli/src/prepare.rs`, `tests/windows/provisioning/`, `docs/lab/cache-preparation.md`

**Interfaces**
- Consumes: D05, C04, C05の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: 既存の専用未mount cachepartitionのみを対象に、dry-run plan→fingerprint付き明示承認→直前再検証/専有の二段階手順。原本partitionは変更しない。

**受入条件**
- OS/origin/同一media/mounted/identity変更/expired確認tokenで書込み前に拒否
- dry-runは書込み0。commit可能な範囲を専用cachepartition内に制限しmock writerで検証
- 実初期化試験は別途承認済みdisposablelabだけ。既存diskを自動partition/formatする機能を追加しない

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** 破壊対象や所有権が曖昧なら止める。Issue実装許可だけで利用者のSSDを初期化しない。

### C08: install・upgrade・rollback・uninstallの安全な運用を作る

**担当 / review:** SOL / ASTRA  
**Depends on:** D08, W07, C07  
**Files (planned):** `packaging/windows/`, `docs/operations/recovery.md`, `tests/windows/install_lifecycle/`

**Interfaces**
- Consumes: D08, W07, C07の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: service/driver packageのstage導入、disabled既定、解除/rundown/reboot、rollback、データ保持uninstall。署名鍵を含めない。

**受入条件**
- 部分install/upgrade失敗/service不在/cache喪失時の復旧。対象外bootと原本保持
- uninstallでHDD/cachepartitionをformatせず、停止不能ならreboot/安全なmanualrecoveryを明示
- package/version/config互換とsupportmatrixを記録。一般ユーザーへHVCI/SecureBoot無効化を要求しない

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** boot/filter残留や署名条件不明ならD05/D08へ。安全なdrainを強制freeで置換しない。

### B01: Rustでlatency統計とstrict benchmark v1 codecを実装する

**担当 / review:** LUNA / SOL  
**Depends on:** F01  
**Files (planned):** `crates/cache-types/src/benchmark/statistics.rs`, `crates/cache-types/src/benchmark/format.rs`, `crates/cache-types/src/benchmark/mod.rs`, `crates/cache-types/tests/benchmark_contract.rs`

**Interfaces**
- Consumes: F01の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: docs/contracts.md §4/7のsummarize_ns/BenchmarkRun/parser。nearest-rank、u64decimalstrings、null未取得、input16MiB/100000samples。

**受入条件**
- [1,2,3,4,100]でp50=3,p95/p99=100。単一要素/u64::MAX/input不変/emptyを確認
- 正常roundtrip、unknown/duplicatefield、末尾document、負/float/bool/overflow/leadingzero、cachemode条件不整合を拒否
- cargo test -p cache-types --test benchmark_contract がPASS。外れ値除去/欠測補完/実機測定はしない

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** 比較条件や統計的有意性を追加する判断はB03/SOLへ。

### B02: 条件が一致するベンチだけを比較するRustレポートを作る

**担当 / review:** LUNA / SOL  
**Depends on:** B01, C02  
**Files (planned):** `crates/cache-cli/src/benchmark.rs`, `crates/cache-cli/tests/benchmark_report.rs`, `examples/benchmarks/`

**Interfaces**
- Consumes: B01, C02の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: compare_runsとcache-cli benchmark-report。契約で一致必須の条件が揃う時だけspeedup/p95/p99を表示し処置条件も併記。

**受入条件**
- 1.0s→0.8sは1.25x。workload/environment/toolchain/working_set/OS cache/repetitions不一致は比較不可
- unknown OS cache/0duration/欠測からInfinity/架空改善率を出さない。mode/SSD coldwarmを隠さない
- cargo test -p cache-cli --test benchmark_report がPASS。stdoutに実機serialやfilepathsを含めない

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** 比較対象の採用/除外ルールを変えたくなったらB03へ。

### B03: cache policy・kernel資源予算・性能採否基準を測定前に固定する

**担当 / review:** SOL / ASTRA  
**Depends on:** D03, D04, F09, B02  
**Files (planned):** `docs/adr/0018-resource-policy.md`, `docs/benchmarks/protocol.md`, `docs/contracts/resource-budgets.md`

**Interfaces**
- Consumes: D03, D04, F09, B02の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: cacheline/sector候補、maxcapacity、nonpaged/inflight/queue上限、promotion/scan-cutoff、数値性能閾値と実験プロトコル。

**受入条件**
- 128/256GB候補のmetadata量と計測値を区別し、予算超過はbypass/拒否でbounded
- OS RAM cache冷温/SSD冷温、RAMより小/大hotset、CPU/network律速の混入を制御
- 改善/overhead/p95p99/CPU/SSDwrite量の合否基準を結果を見る前に承認。未達時のNo-Go/直置き比較も書く

- [ ] 先行契約と一次資料を読み、決める問い・代替案・停止条件を記録する。
- [ ] 受入条件の反例/小さなlab-only検証を設計し、未検証を明示する。
- [ ] Evidence付きADR/契約/schemaを作成し、後続taskのConsumes/Producesへ反映する。
- [ ] 指定reviewを通し、commit/PRと採択/保留の根拠をIssueへ残す。owner decisionは所有者が決める。

**Escalation:** 性能を得るためINVを緩める必要があるならNo-Go。cachehit率だけで価値を主張しない。

### Q01: lab active-modeの障害注入matrixを実行しG3証拠を揃える

**担当 / review:** SOL / ASTRA  
**Depends on:** W09, C07, F08, C06  
**Files (planned):** `tests/windows/fault_matrix/`, `evidence/g3/`, `docs/gates/g3-lab-active.md`

**Interfaces**
- Consumes: W09, C07, F08, C06の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: T01〜T22の適用表、deterministicseed/mock+VMfault、known/unknown outcomes、資源回収とdegraded回復証拠。

**受入条件**
- cache/read/write/hang/corrupt/full、originerror、cancel/remove/lowmemory/slotreuse/flushを別々に注入
- データ差異/二重completion/UAF/持続的漏れ0。意図的故障modelでtest検出力を再確認
- 未観測をPASSにせず、sourcehash/config/seed/OSbuild/結果をevidence-v1で保存

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** 所有権/整合性の故障は直ちに停止してAstra。実disk使用は別途承認済みlabのみ。

### Q02: 実機・HVCI・Verifier・電源/撤去のqualificationを実施する

**担当 / review:** SOL / ASTRA  
**Depends on:** Q01, C08  
**Files (planned):** `docs/lab/hardware-matrix.md`, `evidence/hardware/`, `docs/operations/recovery.md`

**Interfaces**
- Consumes: Q01, C08の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: 承認されたdisposable実機に限定した機器/firmware/sectorpair/build/stackのmatrix、Verifier/HVCI/SecureBoot、sleep/hibernate/故障/回復証拠。

**受入条件**
- VMだけでは検証できないlateI/O/SSDfirmware/実電源条件を明示し、実施できないものは未検証
- Verifierは開発専用機。data/OSbackup、復旧手段、対象fingerprintを事前確認
- 一般利用者向けセキュリティ機能を維持した構成を検証。対応範囲外をsupported表示しない

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** lab機器/電源操作/破壊的試験の所有者承認がなければ停止し、独立にできる証拠整理だけ進める。

### Q03: 開発作業の効果とmiss時overheadを実測して採否を報告する

**担当 / review:** SOL / SOL  
**Depends on:** W09, B03, B02  
**Files (planned):** `benchmarks/workloads/`, `evidence/performance/`, `docs/benchmarks/results.md`

**Interfaces**
- Consumes: W09, B03, B02の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: 固定repo/syntheticworkload、HDD単独/disabled/shadow/activecold/warmを比較しB03の事前閾値で判定。

**受入条件**
- RAMに収まるhotset/超えるhotset、randomread/sequentialscan/mixedwrite、検索/index/buildを同条件で測る
- walltime/p50p95p99/CPU/nonpaged/SSDwrite量と比較不能理由を全て記録
- 改善が小さい/悪化するケースも報告し、SSD直置きとの複雑性/容量costを比較。実測なしの倍率を書かない

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** 閾値を結果に合わせて変えず、未達は再設計またはNo-Goとして報告する。

### Q04: 人間の専門レビューを含む限定beta公開のG4判定を行う

**担当 / review:** ASTRA / ASTRA  
**Depends on:** Q01, Q02, Q03, D08, GOV01, C08  
**Files (planned):** `docs/gates/g4-beta-release.md`, `docs/release-readiness.md`, `release/manifest.json (実配布時)`

**Interfaces**
- Consumes: Q01, Q02, Q03, D08, GOV01, C08の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: 人間のWindowsstorage専門review、署名/HLK条件、supportmatrix、license/notice、security窓口、rollback、owner公開承認を揃えGo/No-Go。

**受入条件**
- AIreview/署名だけで専門家承認/破損ゼロを代行しない。未解決criticalriskは公開を止める
- source/dependency/packagehash、SBOM、対応build、既知非対応、導入/撤去/回復手順を照合
- 公開範囲変更、証明書購入、stable命名、実releaseは明示承認後だけ

- [ ] 依存Issueのartifact/test/reviewer証拠を一覧化する。
- [ ] 受入条件を1つずつ照合し、反証・未検証条件を記録する。
- [ ] 作者とは別のreviewerがGo/No-Goを記録する。未完なら後続はblocked。
- [ ] gate文書とbacklog snapshotをcommit/PRにし、人間の承認が必要な部分は代行しない。

**Escalation:** 資格・費用・専門家・証拠不足ならblocked/No-Goを正直に残す。

### U01: 非特権Tauri GUIの境界と画面/操作契約を設計する

**担当 / review:** SOL / ASTRA  
**Depends on:** D06, C04, C02, C03, G02  
**Files (planned):** `docs/gui/design.md`, `docs/gui/capabilities.md`, `apps/gui/README.md`

**Interfaces**
- Consumes: D06, C04, C02, C03, G02の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: Tauri+Vue/TypeScript採用、status/診断/準備/解除の画面契約。exactversions、capabilities/CSP、serviceIPC、権限分離とmockfixtures。

**受入条件**
- remotecontent/任意shell/filesystem/rawIOCTLをfrontendへ公開しない。WebViewを常時adminにしない
- JSONu64decimalstringsをNumberへ変換せず、unknown/未取得/degradedを正しく表示
- 確認画面だけで承認済みとせず、serviceのtarget再検証/ACLとの対応を設計

- [ ] 先行契約と一次資料を読み、決める問い・代替案・停止条件を記録する。
- [ ] 受入条件の反例/小さなlab-only検証を設計し、未検証を明示する。
- [ ] Evidence付きADR/契約/schemaを作成し、後続taskのConsumes/Producesへ反映する。
- [ ] 指定reviewを通し、commit/PRと採択/保留の根拠をIssueへ残す。owner decisionは所有者が決める。

**Escalation:** UIから直接disk操作したくなる場合はD06へ。GUI都合でkernel/IPCを拡張しない。

### U02: Tauri/Vueで状態・診断の読み取り専用画面を作る

**担当 / review:** LUNA / SOL  
**Depends on:** U01  
**Files (planned):** `apps/gui/src/`, `apps/gui/src-tauri/`, `apps/gui/tests/status/`

**Interfaces**
- Consumes: U01の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: U01画面契約と固定mock/live-readonly APIでstatus/counters/knownerror/診断exportを表示。管理操作はまだ実装しない。

**受入条件**
- 7states、counter未取得/0/u64::MAX、service切断で誤った安全表示/精度損失なし
- 非特権起動、keyboard操作、smallwindow、light/darkで状態/警告が読める。UI視覚確認の証拠を残す
- U01のpinされたbuild/testコマンドがPASS、capabilitiesにwrite/rawIO/shellを追加しない

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** API/画面契約が不明、state遷移やadmin権限が必要ならSOLへ。

### U03: Tauri GUIから安全な準備・enable/disable・解除を統合する

**担当 / review:** SOL / ASTRA  
**Depends on:** U02, C07, C08  
**Files (planned):** `apps/gui/src/admin/`, `apps/gui/src-tauri/service_client.rs`, `apps/gui/tests/admin/`

**Interfaces**
- Consumes: U02, C07, C08の承認済み成果物と docs/contracts.md / INV-01〜12。
- Produces: U01/D06/C07の固定管理APIだけをGUIへ接続。dryrun/fingerprint/明示確認、service側権限と状態再検証を維持。

**受入条件**
- 確認後target変更/権限なし/timeout/servicecrash/二重clickを安全に拒否し、原本をformatしない
- cancelしたUIと実際のdriverquiesce状態を区別し、未完を成功表示しない
- WebViewにrawdevicehandle/任意IOCTL/鍵を渡さず、C07/C08の全安全regressionがGUI経由でも成立

- [ ] 受入条件の各具体例から失敗するテストを先に作る。予定test fileは上記Files。
- [ ] 指定targetを実行し、未実装/既存bugによるFAILと、テストが実行された件数を確認する。
- [ ] Producesの契約だけを実装する。未定義の選択はエスカレーションし、先行ADRを勝手に埋めない。
- [ ] 個別targetとhost全体の適用可能な検査を実行し、PASS/未実行を区別して記録する。
- [ ] 変更範囲のcommit/PRを作成し、指定review後にmerge。依存先のready判定を更新する。

**Escalation:** UIUXの都合でdrain/再認証/対象確認を省略しない。Astraへ安全レビュー。

## 計画の自己点検

- specの12安全契約をD/W/C/Q系とT01〜T22へ割り当てた。
- Rust/Tauri要件とkernel言語例外の所有者を明示した。
- initial-readyは依存なしのF01/D01/D02/D09のみ。GOV01はowner decision。
- 49作業の依存graphにcycle/missing IDがないことを機械検査する。
- 個別Issueの担当難度とmandatory reviewを別に表示する。
- 本文のplanned paths/commandsを実装済み・実行済みと表示しない。
- 実装開始は別依頼。Issueの作成だけで、原本を危険にさらす実機作業の権限は生じない。
