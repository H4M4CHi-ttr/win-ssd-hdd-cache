# 次のWindows工程: inventory → 対象適合性・登録/回復レビュー → 別承認の観測

2026-11-03 JST。D01 #3に戻すための引き継ぎです。**製品が完成して実機調整だけ残った状態ではありません。** portable側の成果と、実Windowsへ接続するための未完部分を分けます。この文書自体は、特定の機械へのアクセスや管理者操作を許可しません。

## 現在あるもの・ないもの

| 成果物 | 現在地 | それだけでは証明しないこと |
|---|---|---|
| [F06 replay](../evidence/F06.md)、[F07](../evidence/F07.md)、[F11](../evidence/F11.md) | 純粋Rustの独立oracle、13固定trace、順序・寿命・error回帰 | 実IRP/MDL/DMA、全mutation捕捉、driverの動作 |
| [P01 audit](../evidence/P01.md) | 収集後のログの欠落・相関・counterを検査するRust CLI | producerの正直さ、取得権限、対象mapping、Windows coverage |
| [D09 proof](../evidence/D09.md) | 小さいRust coreとRust/C-WDK imageのcompile/link・host ABI | driver load、実callback・IRQL・rundown、HVCI適合 |
| [P02 bounded producer](../evidence/P02.md) | Rust core32testsとC scalar56cases、実WDK object-only buildまで受入済み | live callback・transport・rundown・実machine mapping |
| [P03 capture v2](../evidence/P03.md) | PR88で実装・受入済み。Windows296/Linux299のhost CI | producerの診断・実boundaryを自動投影しない |
| [P04観測probe/collector](../evidence/P04.md) | PR91でsource/build受入。host Windows342/Linux344、C69/両OS、Win32 collector link・実WDK4TU/kernel link成功 | 未登録・未load・実traceなし。source/build成功も実機安全性ではない |
| 製品driver・installer・service・Tauri GUI | **未実装**。依存gate待ち | build成功やIssue closeを製品の安全認定にしない |
| 実Windows11 capture・実機性能・電源/撤去試験 | **未実施** | hosted Windows Serverのhost testsでは代替できない |

主実装はRustのまま。Windows境界だけ必要性をADRで説明した小さいC/WDK等を許容します。HDDが唯一の正本、clean read cache / Write-Invalidate、write-backとpersistent cache reuseなし、という方針は変えません。

## 次の小さい受入単位

1. **指定されたWindowsのread-only inventory。** OS/build、実stack、controller/bus、候補targetの関係とqueryの可否を記録する。アクセスできない情報はUNAVAILABLE、空結果はEMPTY。実験成功や対応済みに置換しない。
2. **P04の対象適合性・登録/回復手順のレビュー。** upper候補の限定source/buildは受入済み。inventoryと照合し、既存boot/pagefile/dump等の特殊用途除外、実filter順序、target-specific登録、実効ACL、停止と回復経路を具体化する。lower-SRB候補が必要なら別契約とする。これは製品位置の採択でもD04一般契約の完了でもない。既存sourceをそのままloadしてよいという判断を先取りしない。
3. **必要と判明した差分だけをsource/buildで再検証。** inventoryや独立reviewがP04の未対応条件を示した場合は、限定Issue・契約・固定negative testsを先に追加する。受入済み部品を理由なく再実装せず、source/compiler/WDK/image digestを更新して実CIを通す。install/loadの許可はsource変更許可から推測しない。
4. **別承認の使い捨てlabで観測。** 対象、install/load、信頼設定、許可workload、rollbackを明示してから実行する。[coverage matrix](../contracts/io-coverage.md)と相関logを照合し、独立review後にD01の採否を判断する。

この順序は、すべてのsource/build準備がinventory完了を待つという意味ではありません。[P02 #83](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/83)はbounded producerと非loadable snapshot objectまで受入済み。[P04 #89](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/89)では、未確認の対象機械を仮定せずcallback・forward・rundown・control transportの限定契約、同じdriver sourceの疑似DDI試験、実WDK link、Rust collectorのsource/buildを受入済みです。残る実Windowsの事実はhost modelやhosted Windows Serverのbuildで補えません。source準備と別承認の実機観測を区別します。

source/build準備をG1前に行うこと自体は禁止していません。ただし、実stackの未確認事項を都合のよい仮定で埋め、既存のD01/D04/D10/F08/F09/G1を解除しません。新しい小作業は依存・scope・受入条件を独立reviewしたうえでIssue化します。単にready件数を増やすためにgateを消しません。

## 最初に指定が必要な情報

| 項目 | 必要な指定・確認 |
|---|---|
| inventoryの対象 | どのWindows11 VM/PCでread-only調査を行ってよいか。実験用targetへのアクセス許可とは別 |
| exact environment | Windows build/revision、architecture、候補bus/controller/driver stack。現在値は実際に確認し、過去の会話から確定しない |
| 将来のlab候補 | 廃棄可能なデータ用target、OS/boot/pagefileとの分離、snapshot/backup/console/rollbackの可否。disk番号だけで恒久identityとしない |
| 変更操作の権限 | install/load、mutation、署名mode、Verifier、power/removeは操作別に明示。read-only inventory許可から推論しない |
| 所有者判断 | GOV01のlicense/外部貢献/製品配布、D08の署名主体・鍵・費用等は別。public化とActions許可だけでは完了しない |

最初の工程にformat、raw write、lock/dismount/offline、boot/security設定変更、証明書購入は不要です。VMware/Hyper-V/App Sandbox等の過去の検討だけで現在のlab構成を推測しません。

## Codexへ渡す最初のプロンプト

以下は**ユーザーがinventory対象として指定したWindows環境**で使う文面です。指定がなければ対象だけ確認して止めます。driverをinstallするプロンプトではありません。

```text
H4M4CHi-ttr/win-ssd-hdd-cache の D01 #3 のため、指定されたWindows環境のread-only inventoryを行ってください。
まず最新main、AGENTS.md、docs/handoff/2026-11-03-windows-next-step.md、experiments/stack-trace/README.md、ADR-004、io-coverage.md、D09/P01/P02/P03/P04の証拠と未完gateを確認してください。既存のdirty worktreeを保存してください。

目的は実stackの事実と不明点を返すことだけです。P04の観測candidate sourceはありますが実機qualificationは未完で、製品driverは未実装です。D09/P04 imageをloadせず、P01のinternally_completeやP04のterminal/reconciliationをD01 PASSにしないでください。

1. 対象Windowsの正確なbuild/revision・architecture、Git/Rust/VS/SDK/WDKの導入状態を、既存のread-only queryで確認する。新たなインストールはしない。
2. Microsoftの当該build向けPnPUtil構文と pnputil /? を確認し、許可された権限の範囲でdisk deviceのstack/services/driversを照会する。必要なら以下のenum queryだけを使う:
   pnputil /enum-devices /class DiskDrive /stack /services /drivers
   権限不足、非対応flag、失敗、空結果は区別して報告。自動昇格や設定変更で回避せず、必要な限定queryと理由を提示して確認を求める。
3. origin候補、cache候補、OS/boot/pagefileを取り違えないための既存情報を整理する。serial/instance ID/ユーザーpath等を含む原本はlocal/privateに保持する。公開文書にはfreshな匿名ラベルと必要な非機密要約だけを使い、raw出力をIssue/Actionsへuploadしない。
4. 不明なrange変換、SRB形式、filter順序、target専有性を推測しない。query実装がまだないなら「未実装/未確認」とする。PnP inventoryは実mutation coverageの証拠ではない。
5. 得られた事実、実行command/exit、未確認事項、probe限定契約で先に解決すべき問いを返す。この工程でsource/driver/registry/storage/boot設定を変更しない。

禁止: driver install/load、format/raw write、lock/dismount/offline、BypassIO変更、Secure Boot/HVCI無効化、test-signing/Verifier変更、障害注入、購入・release。これらは別の明示承認とreview済みlab手順が必要です。
```

inventoryが戻れば、Pro/独立review側でP04の適用可否、target-specific登録、実効ACL、特殊用途の除外、停止/回復手順と必要なsource差分を検討できます。その後の観測は別承認です。必ずしも同じ製品名のエージェントである必要はありませんが、指定Windows環境にアクセスできる実行者が必要です。

## 一次資料で再確認した注意点

2026-11-03にMicrosoft資料を再確認しました。

- [PnPUtil構文](https://learn.microsoft.com/en-us/windows-hardware/drivers/devtest/pnputil-command-syntax): `enum-devices`の`stack/services`と`drivers`には導入versionの違いがあります。文書は管理者command promptを案内しており、すべての環境で非昇格の成功を保証しません。権限不足は情報不足として扱います。
- [storage filter dispatch](https://learn.microsoft.com/en-us/windows-hardware/drivers/storage/storage-filter-driver-s-dispatch-routines): class/port間ではSRB形式の識別が必要です。文書のsampleは対象構成の全経路・寿命・回復の証拠ではありません。
- [FSCTL_LOCK_VOLUME](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ni-winioctl-fsctl_lock_volume): cached dataのflushとvolume状態の変更を伴います。read-only inventoryに混ぜません。

既知のD09とP04のrunner-image表示差は原因未解決のまま保持します。P04成功runではstartup bannerが20261025.346.1、manifestの環境値が20261026.347.1でした。両方を記録し、runner imageが厳密にpinされたとは主張しません。署名/HVCI・人間の専門家レビュー・実機qualificationも未完のままです。これらをmodel件数やドキュメント量で代替しません。
