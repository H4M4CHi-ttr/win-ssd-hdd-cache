# T1限定登録・collector中止通知　独立レビュー記録

2026年11月19日 JST。対象は[ソース・ビルド検証仕様](P04-T1-cancel-spec.md)と[試験条件](P04-T1-cancel-test-matrix.md)。基準main `eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c`、[既レビュー手順案](P04-VM-A-T1-runbook.md)のSHA-256は `37f8c86ec02576cc4ef703d09a01a5112c6b9a326155def4bf41d6d5e0ac1451`。

**最終判定は文書 ACCEPT、実機実行 NO-GO。** 最終の仕様書・試験表に修正必須事項はない。提案したソース／ビルド試験の実施状況は全件 NOT RUN。

## 独立性と対象

作成者は設計の際にAstraから排他・通知・不帰還の扱いについて助言を得た。独立判定は、その設計助言者とも作成者とも異なる、新しいコンテキストのAstraレビュアーが担当した。レビュアーは依頼原文、前回の手順案、固定mainのソース・契約、MicrosoftのAPI仕様、ハッシュを凍結した本仕様と試験表を直接照合した。P04の実機やドライバーを起動して得た判定ではない。

| 査読した版 | 仕様書 SHA-256 | 試験表 SHA-256 |
|---|---|---|
| 初稿 | `9bdf2bff6c784b9515300e071a637725aee9adbd4607252b130fc1361ff82d3b` | `797cb46cccafbd53538e5d2d88f03730b321416753264741ee6716ceb74660d0` |
| 配布する最終稿 | `bff471e8877da83c98596afa438df97e036e2cda403b855dade0fc465101bcaf` | `d46bc7f8e52487fe2d1149bc3a9e5913dd16bcc0f86d05b1582f27146711cd4c` |

## レビューで確認したこと

- **誤登録**　T1と別に保護ディスクD01の基準を置く。最終状態が元通りでも、D01・他インスタンス・クラスへの変更を一度でも発行したら試験失敗とする。独自mutexと読戻しを、他者の変更まで排除できる保証に使わない。[Microsoftの設定API][setup]
- **途中失敗**　P・S・K・Fの各変更について、呼出し前、応答消失、変更後のエラー、読戻し／台帳の失敗を変えて同じ状態機械を試す。外から観測した実効果とツールの認識を比較し、自動再送や旧リストでの修復を認めない。
- **中止**　無名イベントによる要求発行と、同じFileObjectを持つownerによる受理・STOPを区別する。応答喪失後のDRAIN再送禁止、cleanup STOPの上限、native v1の形式を保持する。[収集ループ][loop]・[要求と不確実性][builder]
- **同期I/Oの不帰還**　実際に返答を保留するfake workerと外側の監視者を用いる。ownerが返答待ちのままなら「停止未確認」。5,000 msポーリングや15秒の運用目安を、同期呼出し中断の保証にしない。出力write／flushも別の失敗点として扱う。[同期I/O][dcio]・[既存の期限試験][oldtest]

初稿の技術上の修正必須事項は0件。独立レビュアーがコード表記の区切り記号残りを指摘し、最終稿で修正した。作成者は合わせて、REG／ABORT要件ID、selectorの長さ境界、新モードの起動情報、終端後通知、登録モデル・Windows境界・中止通知の個別判定を明確化した。レビュアーは最終稿2点の実ファイルのハッシュ・バイト数と前稿との差分を確認し、表記の修正と追加条件の両方を受け入れた。

## 受入の限界

41行は試験およびビルドの**判定項目**で、実行件数ではない。特にR13は変更段階ごとに展開する。引き渡し時に確認したのは、凍結稿のハッシュ、参照と固定ソース範囲、項目IDの一意性・欠落、文章の整合性である。新規実装・P04ビルド・動作試験・実機操作・GitHubへの投稿は今回実施していない。

Windows実環境での他installer・PnPとの排他、署名済み配備物、制御経路の来歴、実際の装着・CDO権限、応答しないゲストからの回復は後続の実機判断に残る。仕様の受入から実行許可を導かない。

[setup]: https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdisetdeviceregistrypropertyw
[loop]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/crates/cache-probe-collector/src/capture.rs#L98-L170
[builder]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/crates/cache-probe-core/src/archive.rs#L408-L494
[dcio]: https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-deviceiocontrol
[oldtest]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/crates/cache-probe-collector/src/capture.rs#L329-L355
