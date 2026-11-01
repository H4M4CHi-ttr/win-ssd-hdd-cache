# 設計・Issue整備の検査記録

日付: 2026-11-02 JST  
対象: 設計文書、Issue、ラベル、依存計画。**製品の機能/安全性試験ではありません。**

## 実施結果

| 検査 | 結果 |
|---|---|
| GitHub Issue読み戻し | 50件（親1 + 作業49）、重複なし |
| 作業の最低担当 | Luna 16 / SOL 27 / Astra 6 |
| 管理ラベル | 26種類。Issue上の適用を確認 |
| 作業本文の突合 | 49件の受入条件・予定path・依存リンクが計画と一致 |
| dependency graph | 49頂点、cycle/missing IDなし |
| initial ready | D01 #3、D02 #4、D09 #6、F01 #5 |
| 初期の状態 | ready 4 / blocked 44 / owner decision 1（親を除く） |
| 相対文書リンク | 作成した文書集合で解決先を検査 |
| JSON | labels/backlogをparseし重複/ID/型分類/ready条件を検査 |
| Issue form YAML | task/design/integrity/configの4ファイルをPyYAMLでparse、PASS |
| Rust/Tauri要件 | README/spec/contracts/plan/AGENTS/Issueへ反映 |
| repository visibility | privateを維持、公開設定変更なし |

これらは作成時のセッション内の静的検査とGitHub readback。継続実行できるrepo内チェッカー/CIはF02/F10の実装対象で、既に実装済みとは扱わない。

## 独立レビュー

- `74766798b766ef6a9f17c580b94883f3ed6d8680` のspec/contracts/validation/security/decisions/routingを新しいreviewerが全文確認。計画を立てるために未回答の重大な問いはなし。D系で明示した将来の設計判断は、解決済みと見なしていない。
- `07c1c24d799f4de6e68de2ee6f6fd2449c3190a7` の49作業/契約/routingを別担当が確認。DAGにcycle/missing IDなし。以下を修正して反映した:
  1. C04へC02依存を追加し、CLI crateの順序を確保。
  2. F02のstable error出力を同一入力に対する決定性へ修正。配列index変更を不変と要求しない。
  3. F04はF03後と明記。
  4. 通常Cargo/npm依存と、第三者ソースvendoring/外部contribution受付を区別。
  5. 必要なworkspace登録/lock/module exportの付随変更を共通ルールで許可。

AIレビューは人間のWindowsストレージ専門家の出荷レビューを代替しません。

## 実施していないこと

- Rust/C/driver/service/GUIの製品実装・build・テスト。
- WDK/Driver Verifier/HVCI/HLK/実機・電源断試験。
- SSD/HDDへの書込み、format、partition変更、driver install。
- OCF採用確定、kernel Rustのproduction認定。
- ライセンス採択、証明書購入、署名鍵投入、repository公開、製品release。
- GitHub native sub-issue/dependencyの設定（本文リンクとbacklog DAGで管理）。

## 再確認の入口

[親ロードマップ](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/1)、
[バックログ](backlog.md)、
[ラベル/担当規則](agent-routing.md)を参照。進捗や契約が変わったら、live Issueとsnapshotを更新してください。
