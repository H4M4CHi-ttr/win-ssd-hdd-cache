# P04手順案の独立レビュー記録

2026年11月19日 JST。対象は [VM-AのT1限定でP04を観測し、R1へ戻す手順案](P04-VM-A-T1-runbook.md)。独立レビュアーは、作成担当と別コンテキストで稼働したCodexのAstraレビュアー。対象mainは `eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c`。

**最終判定は文書 ACCEPT、実行 NO-GO。** レビュアーは最終稿の原本を読み、ハッシュと変更箇所を照合した。机上の設計案として重大な修正必須事項は残っていない。ドライバーの安全認定、実機での適合確認、所有者の実行承認は今回の判定に含まれない。

## 対象と確認方法

- 基準にした証拠は、固定mainのP04契約、WDKドライバー、Rust collector、ビルド証拠、ADR-004、D01の引き継ぎ、MicrosoftのSetupAPI・filter順序・デバイスセキュリティ資料と、所有者が中継した匿名の `AFTER_R1` 記録。現在のゲストを新たに測定したものではない。[引き継ぎ][handoff]・[ソース／ビルドの受入][build]
- 初稿を通読し、配備パッケージと対象インスタンスの登録を区別できているか、標準ユーザーの拒否試験が共有違反と混同されないか、同期呼び出しが戻らない場合にも実行者が誤って強制停止しないか、R1外に証跡が残るかを点検した。
- 最終稿のSHA-256は `37f8c86ec02576cc4ef703d09a01a5112c6b9a326155def4bf41d6d5e0ac1451`。UTF-8、26,995バイト。配布する手順案はこの版と同一である。

## 各稿の判定と指摘の処理

| 稿 | 独立レビュアーの判定 | 処理 |
|---|---|---|
| 初稿 | 文書 ACCEPT／実行 NO-GO | 対象限定、既存順序、実効DACL、所有者によるSTOP、R1外保全・復元の区別を確認。修正必須事項なし |
| 第2稿 | 当初は文書 CHANGES REQUIRED／実行 NO-GO | 作成者が読み戻し、CDO実体の照会、権限別のopen試験、証跡経路の事前確認、producer faultへの即時停止の限界を具体化。レビューでCDOパスの区切りが二重に見えるというP2指摘あり |
| 最終稿 | **文書 ACCEPT／実行 NO-GO** | 両者が実ファイルの区切り文字を数え、第2稿・最終稿とも各1文字と独立に確認。第2稿のP2は表示上のエスケープによる誤検出としてレビュアーが撤回。最終稿ではサービスのキーと値の関係、Object Managerパスの書き方を2行で明確化した |

レビュアーが直接確認した初稿のSHA-256は `b0af55b2a285ea560288afade728b3e4db38a6d52ab17ae67fc9b4cadb77fcdb`、第2稿は `2d422acdb7df3884d9efec11faeaee7f1680c461d5a403aaa48902f7763dea99`。対象名の表記に実ファイルの欠陥があったとの判定は残っていない。

## 実行前に残る条件

| 対象 | デスクレビューで一致した判断 | 未充足の条件 |
|---|---|---|
| T1限定登録 | ドライバーは最初のロード時にselectorを読み、個別のPDOで完全一致を検査する。配備や登録の範囲はこれだけでは決まらない | 配布物・署名、T1のdevnodeに限った実装、競合や部分適用の固定試験、D01非変更と装着位置の実機確認方法を別に受け入れる。[対象照合][driver]・[インスタンスへの設定API][setup] |
| 装着位置・アクセス権 | 既存filterを保存し、同level順は保証しない。CDOの実効DACLと実トークンによるopen結果を確認する | 想定順・権限を実際に判別できる検査手段が必要。実効DACLと実stackの検査は、将来の許可されたロード後、観測開始前に行う。[Windowsの順序][order]・[P04のCDO契約][cdo] |
| 中止・証跡 | STOPは同じ所有者のハンドルを要する。同期IOCTLのハングをポーリング上限で打ち切れず、別プロセスからの早期停止機能もない | 所有者への中止通知、失敗時の試験、監視期限とホスト側対応、バイナリが欠けずにR1の外へ届く経路を具体化する。[collectorの収集ループ][loop] |
| 復元 | 既存R1について、準備段階の停止時復元と両ディスクの再同定を確認済み | P04装着中の異常からの回復は未実証。正常終了不能時の操作が許可されていない限り、強制操作を伴う回復には進めない。R1後の実測buildは `26300.9974`、同一buildの再現性は未確認 |

今回の作業は文書作成・資料照合・独立レビューまで。実際の登録・署名・ロード・観測、VM設定変更、GitHubへの投稿・Issue更新は実施していない。

[handoff]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/docs/handoff/2026-11-03-windows-next-step.md#L20-L25
[build]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/docs/evidence/P04.md#L338-L362
[driver]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/experiments/stack-trace/probe/wdk/driver.c#L33-L125
[setup]: https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdisetdeviceregistrypropertyw
[order]: https://learn.microsoft.com/en-us/windows-hardware/drivers/develop/device-filter-driver-ordering
[cdo]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/docs/contracts/upper-candidate-probe-v1.md#L84-L98
[loop]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/crates/cache-probe-collector/src/capture.rs#L98-L170
