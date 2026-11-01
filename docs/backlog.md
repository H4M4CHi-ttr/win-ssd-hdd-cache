# 実装バックログ

基準: 2026-11-02 JST。**作業49件 + [親ロードマップ #1](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/1) = 50 Issues**。製品実装は未着手。現在の進捗はGitHub Issueを確認し、contract/依存/完了時にはこのsnapshotも更新する。

## 最初の着手点

| Issue | 担当 | 今できること |
|---|---|---|
| [F01 #5](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/5) | Luna / SOL review | Rust workspaceとpure checked range。実disk不要 |
| [D01 #3](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/3) | Astra / independent Astra review | Windows I/O捕捉経路の調査・契約化 |
| [D02 #4](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/4) | Astra / independent Astra review | 整合性モデル・反例・trace契約 |
| [D09 #6](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/6) | SOL / Astra review | Rust kernel/WDKの境界とtoolchain実証 |

GOV01 [GOV01 #2](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/2) はライセンス等のowner decision。通常のCargo/npm依存利用と、ソースvendoring/外部contribution受付/公開を区別する。

F01完了後、F02/F03/B01/C01が並列候補。F04はF03でcache-model crateを作成した後。blockedラベルは自動解除されない。先行Issueの証拠と承認を確認して更新する。

## 担当別

| 最低担当 | 作業数 | 主な範囲 |
|---|---:|---|
| Luna | 16 | 固定契約のRust純粋関数、codec、fixtures、静的検査、後期GUI表示 |
| SOL | 27 | Windows/FFI/service統合、運用、性能、配布調査 |
| Astra | 6 | I/O捕捉、整合性、oracle、G1/G2/G4判断 |

親ロードマップは別枠のAstra担当。review:astraは実装者と別のレビューを意味し、人間の出荷承認を代替しない。

## 全作業

| Issue | 課題 | 担当 → review | 初期状態 | Depends on |
|---|---|---|---|---|
| [GOV01 #2](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/2) | OSSライセンス・外部貢献条件・公開範囲を所有者と確定する | SOL → Astra | needs-decision | なし |
| [D01 #3](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/3) | WindowsのI/O捕捉位置と全変更経路の対応表を決める | Astra → Astra | ready | なし |
| [D02 #4](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/4) | clean-cacheの整合性モデル・世代・公開条件を反例付きで凍結する | Astra → Astra | ready | なし |
| [D03 #8](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/8) | OCF WIと最小Rust coreを比較しcache engineの採否を決める | SOL → Astra | blocked | [D02 #4](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/4) |
| [D04 #11](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/11) | IRP・MDL・DMA・FFIの所有権とcancel/timeout契約を固定する | SOL → Astra | blocked | [D01 #3](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/3), [D02 #4](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/4), [D03 #8](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/8), [D09 #6](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/6) |
| [D05 #15](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/15) | 対象identity・暗号化境界・power・解除の状態遷移を固定する | SOL → Astra | blocked | [D01 #3](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/3), [D04 #11](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/11) |
| [D06 #19](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/19) | 管理サービス・IOCTL・IPCの認可とversioned ABIを設計する | SOL → Astra | blocked | [D04 #11](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/11), [D05 #15](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/15) |
| [D07 #17](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/17) | 整合性・flush耐久性・故障注入のoracleを定義する | Astra → Astra | blocked | [D02 #4](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/4), [D04 #11](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/11) |
| [D08 #9](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/9) | 署名・HLK・HVCI・配布/更新の実行可能性を確認する | SOL → Astra | blocked | [D01 #3](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/3), [D09 #6](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/6) |
| [D09 #6](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/6) | Rust kernel/WDKの適合性とC shimの最小境界を決める | SOL → Astra | ready | なし |
| [F01 #5](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/5) | Rust workspaceとchecked byte-range/line-spanを実装する | Luna → SOL | ready | なし |
| [F02 #10](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/10) | ラベルとバックログの静的整合性を検査するxtaskを作る | Luna → SOL | blocked | [F01 #5](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/5) |
| [F03 #7](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/7) | cancelとcompletionを分ける決定的fake-I/Oキューを実装する | Luna → SOL | blocked | [F01 #5](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/5) |
| [F04 #12](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/12) | discardをunknownとして扱うin-memory originを実装する | Luna → SOL | blocked | [F01 #5](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/5), [F03 #7](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/7) |
| [F05 #14](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/14) | 凍結済みtrace v1のstrict Rust parserを実装する | Luna → SOL | blocked | [F01 #5](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/5), [D02 #4](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/4) |
| [F06 #20](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/20) | traceを再生し独立oracleと照合するRust harnessを作る | SOL → Astra | blocked | [D07 #17](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/17), [F03 #7](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/7), [F04 #12](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/12), [F05 #14](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/14) |
| [F07 #23](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/23) | 基本WI・partial write・複数writer・stale-fillの回帰traceを追加する | Luna → Astra | blocked | [F06 #20](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/20) |
| [F08 #25](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/25) | late completion・slot再利用・resource faultの回帰traceを追加する | Luna → Astra | blocked | [F06 #20](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/20), [D04 #11](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/11) |
| [F09 #27](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/27) | 選定engineをuser-mode harnessで独立oracleと対照試験する | SOL → Astra | blocked | [D03 #8](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/8), [D09 #6](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/6), [F06 #20](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/20), [GOV01 #2](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/2), [F07 #23](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/23), [F08 #25](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/25) |
| [G01 #29](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/29) | 設計契約とoffline modelのG1ゲートをレビューする | Astra → Astra | blocked | [D01 #3](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/3), [D02 #4](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/4), [D03 #8](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/8), [D04 #11](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/11), [D05 #15](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/15), [D06 #19](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/19), [D07 #17](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/17), [D08 #9](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/9), [D09 #6](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/6), [F07 #23](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/23), [F08 #25](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/25), [F09 #27](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/27), [F10 #13](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/13) |
| [F10 #13](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/13) | 秘密・管理者操作なしのRust host CIを構成する | Luna → SOL | blocked | [F02 #10](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/10) |
| [W01 #31](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/31) | 専用lab向けpassthrough-only driverを構築する | SOL → Astra | blocked | [G01 #29](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/29) |
| [W02 #34](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/34) | 選定cache engineとWindows境界を最小FFIで接続する | SOL → Astra | blocked | [W01 #31](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/31), [D03 #8](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/8), [D04 #11](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/11), [F09 #27](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/27) |
| [W03 #32](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/32) | owned bufferと非同期I/O rundownを実装する | SOL → Astra | blocked | [W01 #31](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/31), [D04 #11](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/11) |
| [W04 #36](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/36) | 専有SSD cache backendとslot再利用障壁を実装する | SOL → Astra | blocked | [W03 #32](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/32), [D05 #15](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/15) |
| [W05 #35](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/35) | write-invalidateと原本write/flush順序を実装する | SOL → Astra | blocked | [W01 #31](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/31), [D02 #4](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/4), [D04 #11](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/11) |
| [W06 #33](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/33) | DSM・raw/control・未知mutationの安全な処理を実装する | SOL → Astra | blocked | [W01 #31](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/31), [D01 #3](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/3), [D05 #15](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/15) |
| [W07 #37](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/37) | attach・power・disable・removeのcold-reset lifecycleを実装する | SOL → Astra | blocked | [W01 #31](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/31), [W03 #32](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/32), [D05 #15](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/15), [C05 #26](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/26) |
| [W08 #39](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/39) | 原本だけを返すshadow cache観測を統合する | SOL → Astra | blocked | [W02 #34](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/34), [W03 #32](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/32), [W04 #36](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/36), [W05 #35](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/35), [W06 #33](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/33), [W07 #37](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/37) |
| [G02 #41](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/41) | passthrough・shadowのG2ゲートを独立レビューする | Astra → Astra | blocked | [W08 #39](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/39), [C04 #38](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/38), [C05 #26](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/26), [C06 #24](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/24) |
| [W09 #43](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/43) | 検証済みlab構成でのみread-cache hit提供を有効にする | SOL → Astra | blocked | [G02 #41](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/41), [B03 #30](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/30) |
| [C01 #16](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/16) | offline status v1のRust型とstrict JSON codecを実装する | Luna → SOL | blocked | [F01 #5](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/5) |
| [C02 #22](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/22) | offline statusを表示するRust CLIを作る | Luna → SOL | blocked | [C01 #16](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/16) |
| [C03 #21](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/21) | 診断情報をallowlistだけで出力するRust関数を実装する | Luna → Astra | blocked | [C01 #16](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/16) |
| [C04 #38](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/38) | Rust管理サービスと最小権限IPCを実装する | SOL → Astra | blocked | [D06 #19](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/19), [C01 #16](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/16), [W01 #31](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/31), [C02 #22](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/22) |
| [C05 #26](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/26) | 凍結済みtarget descriptorの純粋な許可/拒否判定器を作る | Luna → Astra | blocked | [F01 #5](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/5), [D05 #15](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/15) |
| [C06 #24](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/24) | control ABIの固定golden vectorsと不正入力回帰を追加する | Luna → Astra | blocked | [F01 #5](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/5), [D06 #19](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/19) |
| [C07 #40](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/40) | cache専用partitionのdry-run準備と明示確認を実装する | SOL → Astra | blocked | [D05 #15](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/15), [C04 #38](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/38), [C05 #26](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/26) |
| [C08 #42](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/42) | install・upgrade・rollback・uninstallの安全な運用を作る | SOL → Astra | blocked | [D08 #9](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/9), [W07 #37](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/37), [C07 #40](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/40) |
| [B01 #18](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/18) | Rustでlatency統計とstrict benchmark v1 codecを実装する | Luna → SOL | blocked | [F01 #5](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/5) |
| [B02 #28](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/28) | 条件が一致するベンチだけを比較するRustレポートを作る | Luna → SOL | blocked | [B01 #18](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/18), [C02 #22](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/22) |
| [B03 #30](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/30) | cache policy・kernel資源予算・性能採否基準を測定前に固定する | SOL → Astra | blocked | [D03 #8](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/8), [D04 #11](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/11), [F09 #27](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/27), [B02 #28](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/28) |
| [Q01 #45](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/45) | lab active-modeの障害注入matrixを実行しG3証拠を揃える | SOL → Astra | blocked | [W09 #43](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/43), [C07 #40](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/40), [F08 #25](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/25), [C06 #24](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/24) |
| [Q02 #48](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/48) | 実機・HVCI・Verifier・電源/撤去のqualificationを実施する | SOL → Astra | blocked | [Q01 #45](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/45), [C08 #42](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/42) |
| [Q03 #46](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/46) | 開発作業の効果とmiss時overheadを実測して採否を報告する | SOL → SOL | blocked | [W09 #43](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/43), [B03 #30](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/30), [B02 #28](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/28) |
| [Q04 #50](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/50) | 人間の専門レビューを含む限定beta公開のG4判定を行う | Astra → Astra | blocked | [Q01 #45](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/45), [Q02 #48](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/48), [Q03 #46](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/46), [D08 #9](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/9), [GOV01 #2](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/2), [C08 #42](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/42) |
| [U01 #44](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/44) | 非特権Tauri GUIの境界と画面/操作契約を設計する | SOL → Astra | blocked | [D06 #19](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/19), [C04 #38](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/38), [C02 #22](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/22), [C03 #21](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/21), [G02 #41](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/41) |
| [U02 #47](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/47) | Tauri/Vueで状態・診断の読み取り専用画面を作る | Luna → SOL | blocked | [U01 #44](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/44) |
| [U03 #49](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/49) | Tauri GUIから安全な準備・enable/disable・解除を統合する | SOL → Astra | blocked | [U02 #47](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/47), [C07 #40](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/40), [C08 #42](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/42) |

## 利用上の約束

- [backlog.json](backlog.json)がmachine-readable snapshot。IDはIssue番号と別に安定して保持する。
- 全Issue本文に目的、予定path、Consumes/Produces、具体的受入条件、エスカレーション、Doneを含む。
- GitHubのDepends onは本文のチェックリストで表す。GitHubネイティブのsub-issue/dependency機能を設定済みとは主張しない。
- gateはcloseだけで通過しない。契約/証拠/independent reviewが必要。
- initial-ready 4件、blocked 44件、owner decision 1件。これは作成時の分類であり、将来のlive進捗の自動集計ではない。
