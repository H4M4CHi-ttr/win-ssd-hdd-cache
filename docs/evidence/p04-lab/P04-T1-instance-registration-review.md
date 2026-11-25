# P04　元のT1に限る個体別登録契約の独立レビュー

2026年11月23日。**設計受入はACCEPT。実登録はNO-GO。** 別コンテキストのAstraレビュアー（`reviewer_astra`）が、[契約案の最終版](P04-T1-instance-registration-contract.md)を保存済みファイルから読み、SHA-256を照合した。レビュアーは契約の作成、設計相談、文献調査の担当者とは別である。

| 対象 | SHA-256 | 判定 |
|---|---|---|
| 初稿 | `514220EA64BAFBA8ED93636719DCB0699A5D5C640023AC0D16C8768861CAAA8F` | CHANGES REQUIRED。高2件、中1件 |
| 最終版 | `F35A16684ED8785989078AEF258294FC6942D5060F8BC7D4899738A9B9B3194B` | 3件とも設計上解消。変更範囲に新たな重大指摘なし |

## 初稿の反例と改訂内容

| 指摘 | 改訂と再審査の結果 |
|---|---|
| **高　同一個体のまま用途が変わる。** PRE後に別の管理処理がT1を初期化したり、区画を作ったりしても、インスタンスIDと個体別フィルターの未作成状態が保たれる場合がある。旧条件では、利用中のT1に再始動を要求できた。 | 配置前から正常停止まで、T1の用途・子・停止依存先の保護をWindows側に要求した。新設の成立時と再始動の取消不能な引渡し時の両方で、用途と依存先の変更を防げることを受入条件とした。配備済み・新設0と、配備済み・新設1・再始動0を区別する。最終版30、54–59、109行で確認。 |
| **高　打切り後に最初の要求が届く。** 許可を得たワーカーがWindowsを呼ぶ前に停止し、ホストの期限切れ後に復帰できる。二重発行を防ぐだけではこの1件を防げない。 | 取消可能な許可、取消不能な引渡し、Windowsの受理を分離した。失効が先なら発行0件、引渡しが先なら当該1件まで、順序不明なら効果不明として後続段階を禁止する。切断・遅延復帰を負例に追加した。最終版71–75、102、110行で確認。 |
| **中　リンク公開前のNT名と既存ハンドル。** 名前付き制御デバイスを先に作れば、DOSリンクを遅らせてもNT名から開ける可能性がある。リンク削除後のハンドルも残り得る。 | 装着前のNT名・リンク・インターフェイスの公開とCREATE成功を禁じた。登録確認中は旧・継承・複製ハンドルを含めてBEGIN成功0件。装着後の制御デバイス生成・公開失敗を部分成功として、進行中のI/Oと参照の寿命を守る。最終版84、113行で確認。[Microsoftの名前付きデバイスオブジェクト](https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/named-device-objects) |

## 照合した原資料

レビュアーは[先行する登録・再始動契約](P04-T1-registration-restart-first-load-contract.md)、[排他・配備範囲の審査](P04-T1-registration-exclusion-and-package-scope.md)、[ソース審査記録](P04-source-build-review.md)と[レビュー時のソース](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/tree/153fac3473186241ccf3f354e59322e63b1f183f)を照合した。ZIPのSHA-256は `275980388AF9E334949346D9DD7EB02900017BB9D6A5FDB054846AED5800B3AD`。manifestが示す上流commitは `eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c`、受入済みソースツリーは `f1fb5c4f9f459edd2aeebfc490bef246f4a2f7d7` である。

ZIP内の `boundary.rs` 242–269・477行、`lib.rs` 469–470行、primitive INF、`driver.c` 72–170行を直接照合した。既存setterは旧値を再読後に全値を設定し、実適用の判定は `RealApplyUnavailable` を返す。既存ドライバーはselectorを一度だけ読み、実attachの前にCDOとリンクを準備する。ZIP内のINFは1件、CATとSYSは各0件だった。11月22日のVM測定は共有済みの匿名記録を使い、ゲストに再照会していない。

Microsoftの公開仕様とも照合した。個体指定setterが受け取る引数、primitiveのシステム単位の配備、他機器に及び得る再始動、呼出し時点での最上位へのattachは、契約の前提と整合する。[個体別setter](https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdisetdeviceregistrypropertyw)／[primitive driver](https://learn.microsoft.com/en-us/windows-hardware/drivers/develop/creating-a-primitive-driver)／[デバイス再始動](https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdirestartdevices)／[実attach](https://learn.microsoft.com/en-us/windows-hardware/drivers/ddi/ntddk/nf-ntddk-ioattachdevicetodevicestacksafe)

## 受入の限界

初回は契約全体、再審査は3件の修正とその影響を確認した。Windowsで条件付き新設・世代と用途の保護・失効と発行の順序を強制できる経路は未立証である。対象の現在の特殊用途、最終署名済みパッケージ、実オブジェクトの装着順、実効ACL、R1による回復について、新契約での検証結果はない。**最初のパッケージ配置も未許可**で、新方式の合成試験は将来の受入条件として定めた。今回の成果は契約案の作成と独立レビューまでである。
