# Luna / SOL / Astraの担当とエスカレーション

ラベルは**最低限の作業難度/レビュー強度**を示す。GitHubの実ユーザーへのassignee設定や、自動実行の許可ではない。

## 1. ラベル

| 系統 | 意味 |
|---|---|
| agent:luna | 入出力・期待値・変更範囲が固定された局所実装 |
| agent:sol | 複数component統合、Windows SDK、未知の実装判断が必要 |
| agent:astra | 整合性モデル、捕捉境界、gate採否など破損に直結する設計判断 |
| review:sol | 独立したSOL以上のレビュー必須 |
| review:astra | 独立したAstra級の設計/安全性レビュー必須 |
| status:ready | 必要契約が本文にあり、依存課題/必要承認が完了して着手可能 |
| status:blocked | 前提の契約/実装/検証が未完。実装を先行しない |
| status:needs-decision | 所有者のライセンス/公開等の明示決定が必要 |
| status:done | 指定成果物・証拠・独立reviewの受入完了。製品安全認証ではない |
| status:cancelled | 実施しないと決定。依存の完了には数えない |
| priority:p0 | 安全性・実装開始に必要 |
| priority:p1 | MVP/lab実用化に必要 |
| priority:p2 | GUI等の後段機能 |
| type:design / task / gate / tracking | 設計決定 / bounded実装 / 証拠判定 / 親トラッカー |
| area:* | 責務分類。担当モデルとは別 |
| decision:owner | ライセンス、公開、購入、実機使用等をモデルだけで決められない |

各作業Issueはagent・review・status・priority・typeをそれぞれ一つ持つ。areaは基本一つ。decision:ownerは追加ラベル。agent:luna + review:astraの併用を認める。固定traceを書くだけでも、その期待値が難しい場合がある。

上位モデル/rootは下位難度の課題も実装できる。担当ラベルを「実装したモデル」に合わせて不必要に昇格しない。backlog v2のowner_approval/evidenceで承認待ちと承認済みを区別し、done=accepted/cancelled=not-plannedを守る。URLだけで承認を機械認定しない。

## 2. 着手前

1. 本文のDepends onと設計契約を読む。closeだけでなく、必要artifactと承認がそろっているか確認する。
2. status:blocked / needs-decisionのIssueは勝手にreadyへ変えない。
3. Issueに着手コメントを残し、同じCargo.toml/lib.rs等を触る作業の競合を確認する。
4. 一つのbranch/PRで一つの独立受入単位を扱う。共有manifestの追加は短い統合順で直列化する。
5. 下位依存にないscopeを発見したら、関連Issueを追加/更新してレビューする。隠れた前提を作らない。

製品driverは未実装。host基盤とD02はmain統合済み。D01は資料調査まで統合済みで、実traceと受入は別承認のWindows lab待ち。次のreadyはbacklogの依存・受入状態を正本とする。二OS host試験はhost課題の受入gateであり、kernel runtime/実機の未実施は必ず明記する。

## 3. Luna課題の標準完了条件

- 指定pathとcontractを変更範囲の基本とする。必要なcrateのworkspace登録、Cargo.lock、lib.rs/module export、test登録のみ付随変更を許可し、統合順を調整して差分を説明する。無関係な依存更新はしない。
- 先に失敗するテストを作り、FAILの理由を確認してから最小実装。
- 指定test + fmt + clippyがPASSし、テスト0件を合格扱いしない。
- cargo lock/toolchain変更を説明。unsafe、Windows API、network、raw diskを指示なしに追加しない。
- 想定結果を実装都合で変更しない。
- 独立review後にmergeし、Issueへcommit/PR/test結果/残制約を記録。
- accepted/doneは指定artifactの受入状態。PR未mergeならGitHub Issueはopenのまま統合待ちと明記する。後続作業は受入artifactを含むbranchを使い、mainに存在すると推測しない。
- 未実行のWindows/実機試験を「PASS」と書かない。

## 4. エスカレーション

**SOLへ**: schema/APIの穴、複数crateの責務衝突、unknown Windows API挙動、依存追加が必要、受入条件を維持した実装方法が不明。

**Astraへ**: stale data、write/flush順序、raw mutation取りこぼし、IRP/MDL/DMA寿命、slot/epoch reuse、unsafe Send/Sync/FFI所有権、origin/cache誤認、権限/暗号化境界、既存安全契約の変更。

コメント形式:

- 止めた位置と変更していない範囲
- 再現入力/trace/seedと期待値・実際値
- 不明な問い（1つずつ）
- 影響する契約/Issue
- 安全な選択肢と判断が必要な点

安全判断は一度で昇格してよい。「何回も失敗してから」ではない。元のagentラベルを上位へ置換し、reviewを弱めない。statusをblockedにし、解決後は本文/contract/fixturesを更新してから再開する。

## 5. AIレビューと人間の責任

モデル名は品質保証ではない。とくに出荷判定、署名主体、実機障害試験、ライセンスは責任を持てる人間の確認が必要。Astraのコメントだけで「Windowsストレージ専門家による承認」を代行しない。kernel変更の最終reviewは作者と別担当にする。

## 6. 既存ラベルの扱い

管理taxonomyは `.github/labels.json`。今回のIssueはそのprefixで統一。GitHubの既定ラベルや利用者が追加したラベルを一括削除しない。意味の重なる新ラベルを作る前にtaxonomyを更新してレビューする。色は管理せず、モデル/状態は名前と本文から識別できるようにする。

