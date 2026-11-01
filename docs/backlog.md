# 実装バックログ（初回起票中）

49件の作業を定義し、依存DAGを検査済みです。GitHubの起票後、同じIDとIssue番号を対応させます。各課題のscope/受入条件の正本は[backlog.json](backlog.json)です。

| ID | 課題 | 最低担当 | 必須review | 依存 |
|---|---|---|---|---|
| GOV01 | OSSライセンス・外部貢献条件・公開範囲を所有者と確定する | sol | astra | なし |
| D01 | WindowsのI/O捕捉位置と全変更経路の対応表を決める | astra | astra | なし |
| D02 | clean-cacheの整合性モデル・世代・公開条件を反例付きで凍結する | astra | astra | なし |
| D03 | OCF WIと最小Rust coreを比較しcache engineの採否を決める | sol | astra | D02 |
| D04 | IRP・MDL・DMA・FFIの所有権とcancel/timeout契約を固定する | sol | astra | D01, D02, D03, D09 |
| D05 | 対象identity・暗号化境界・power・解除の状態遷移を固定する | sol | astra | D01, D04 |
| D06 | 管理サービス・IOCTL・IPCの認可とversioned ABIを設計する | sol | astra | D04, D05 |
| D07 | 整合性・flush耐久性・故障注入のoracleを定義する | astra | astra | D02, D04 |
| D08 | 署名・HLK・HVCI・配布/更新の実行可能性を確認する | sol | astra | D01, D09 |
| D09 | Rust kernel/WDKの適合性とC shimの最小境界を決める | sol | astra | なし |
| F01 | Rust workspaceとchecked byte-range/line-spanを実装する | luna | sol | なし |
| F02 | ラベルとバックログの静的整合性を検査するxtaskを作る | luna | sol | F01 |
| F03 | cancelとcompletionを分ける決定的fake-I/Oキューを実装する | luna | sol | F01 |
| F04 | discardをunknownとして扱うin-memory originを実装する | luna | sol | F01, F03 |
| F05 | 凍結済みtrace v1のstrict Rust parserを実装する | luna | sol | F01, D02 |
| F06 | traceを再生し独立oracleと照合するRust harnessを作る | sol | astra | D07, F03, F04, F05 |
| F07 | 基本WI・partial write・複数writer・stale-fillの回帰traceを追加する | luna | astra | F06 |
| F08 | late completion・slot再利用・resource faultの回帰traceを追加する | luna | astra | F06, D04 |
| F09 | 選定engineをuser-mode harnessで独立oracleと対照試験する | sol | astra | D03, D09, F06, GOV01, F07, F08 |
| G01 | 設計契約とoffline modelのG1ゲートをレビューする | astra | astra | D01, D02, D03, D04, D05, D06, D07, D08, D09, F07, F08, F09, F10 |
| F10 | 秘密・管理者操作なしのRust host CIを構成する | luna | sol | F02 |
| W01 | 専用lab向けpassthrough-only driverを構築する | sol | astra | G01 |
| W02 | 選定cache engineとWindows境界を最小FFIで接続する | sol | astra | W01, D03, D04, F09 |
| W03 | owned bufferと非同期I/O rundownを実装する | sol | astra | W01, D04 |
| W04 | 専有SSD cache backendとslot再利用障壁を実装する | sol | astra | W03, D05 |
| W05 | write-invalidateと原本write/flush順序を実装する | sol | astra | W01, D02, D04 |
| W06 | DSM・raw/control・未知mutationの安全な処理を実装する | sol | astra | W01, D01, D05 |
| W07 | attach・power・disable・removeのcold-reset lifecycleを実装する | sol | astra | W01, W03, D05, C05 |
| W08 | 原本だけを返すshadow cache観測を統合する | sol | astra | W02, W03, W04, W05, W06, W07 |
| G02 | passthrough・shadowのG2ゲートを独立レビューする | astra | astra | W08, C04, C05, C06 |
| W09 | 検証済みlab構成でのみread-cache hit提供を有効にする | sol | astra | G02, B03 |
| C01 | offline status v1のRust型とstrict JSON codecを実装する | luna | sol | F01 |
| C02 | offline statusを表示するRust CLIを作る | luna | sol | C01 |
| C03 | 診断情報をallowlistだけで出力するRust関数を実装する | luna | astra | C01 |
| C04 | Rust管理サービスと最小権限IPCを実装する | sol | astra | D06, C01, W01 |
| C05 | 凍結済みtarget descriptorの純粋な許可/拒否判定器を作る | luna | astra | F01, D05 |
| C06 | control ABIの固定golden vectorsと不正入力回帰を追加する | luna | astra | F01, D06 |
| C07 | cache専用partitionのdry-run準備と明示確認を実装する | sol | astra | D05, C04, C05 |
| C08 | install・upgrade・rollback・uninstallの安全な運用を作る | sol | astra | D08, W07, C07 |
| B01 | Rustでlatency統計とstrict benchmark v1 codecを実装する | luna | sol | F01 |
| B02 | 条件が一致するベンチだけを比較するRustレポートを作る | luna | sol | B01, C02 |
| B03 | cache policy・kernel資源予算・性能採否基準を測定前に固定する | sol | astra | D03, D04, F09, B02 |
| Q01 | lab active-modeの障害注入matrixを実行しG3証拠を揃える | sol | astra | W09, C07, F08, C06 |
| Q02 | 実機・HVCI・Verifier・電源/撤去のqualificationを実施する | sol | astra | Q01, C08 |
| Q03 | 開発作業の効果とmiss時overheadを実測して採否を報告する | sol | sol | W09, B03, B02 |
| Q04 | 人間の専門レビューを含む限定beta公開のG4判定を行う | astra | astra | Q01, Q02, Q03, D08, GOV01, C08 |
| U01 | 非特権Tauri GUIの境界と画面/操作契約を設計する | sol | astra | D06, C04, C02, C03, G02 |
| U02 | Tauri/Vueで状態・診断の読み取り専用画面を作る | luna | sol | U01 |
| U03 | Tauri GUIから安全な準備・enable/disable・解除を統合する | sol | astra | U02, C07, C08 |
