# P04の対象限定登録とcollector停止の検証記録

元のT1へP04を実登録する条件は満たせていない。実装の対象は、指定ディスクの選択、登録の途中失敗、同期I/Oが戻らないときのcollectorの停止通知を検査する合成試験と、Windows API境界のコンパイル・リンクまで。実デバイスへの適用入口は閉じている。

| 確認したいこと | 記録 |
| --- | --- |
| 検証したソースと独立レビュー | [48ファイルを適用したソース](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/tree/153fac3473186241ccf3f354e59322e63b1f183f)、[検証結果](P04-source-build-evidence.md)、[実装レビュー](P04-source-build-review.md) |
| 対象取り違え、部分失敗、不帰還時の合否 | [仕様](P04-T1-cancel-spec.md)、[試験表](P04-T1-cancel-test-matrix.md)、[仕様レビュー](P04-T1-cancel-review.md) |
| Windowsで実行に移せるか | [排他と配備範囲](P04-T1-registration-exclusion-and-package-scope.md)、[宣言的登録の判定](P04-T1-declarative-filter-decision.md)、[個体別登録案](P04-T1-instance-registration-contract.md)、[公開APIでの成立性とNO-GO判定](P04-T1-windows-registration-feasibility.md) |
| VMの準備・復元条件を決めた経緯 | [当初の手順案](P04-VM-A-T1-runbook.md)、[登録・再始動・初回ロードを扱う別契約案](P04-T1-registration-restart-first-load-contract.md) |

各文書は作成時点の記録である。ソースの基準は `eb2ee057`、検証ツリーは `f1fb5c4f9f459edd2aeebfc490bef246f4a2f7d7`。[実測の概要](evidence/verification-summary.json)とOS別の証跡は当時の引き渡し資料から取り出した。`basis/reviewed-inputs.json`と`evidence/index.json`のハッシュは当時の配布バイトに対するもの。公開版ではローカルZIPへのリンクを上記ソースに置き換え、VM内外のファイル名を一部匿名化した。誤登録検出の抜粋ログは末尾の空行を整えた。元ZIPや同じソースの複製、実行バイナリは配布しない。
