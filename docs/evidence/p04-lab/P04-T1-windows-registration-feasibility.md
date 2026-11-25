# P04　元のT1への個体別登録をWindowsの公開APIで実現できるか

2026年11月23日。**実登録はNO-GO。** 指定したdevnodeのプロパティを設定する経路はある。調べた経路では、他の書き手による遅延更新を含めてT1の登録を保護し、用途・世代を保ったまま1回だけ再始動する条件までは満たせなかった。追加のVM試行や署名を行う根拠はない。

判定対象は[受入済みの個体別登録契約](P04-T1-instance-registration-contract.md)（SHA-256 `F35A16684ED8785989078AEF258294FC6942D5060F8BC7D4899738A9B9B3194B`）。[Astraのレビュー](P04-T1-instance-registration-review.md)が受け入れた版を変更していない。本資料はその契約で未立証だったWindows経路の調査記録である。

## 何を照合したか

対象の既存証拠はWindows 11 x64、build `26300.9974`。11月22日の実測では、同じT1を別の仮想NVMeコントローラーへ移しても、D01と共通のHardware IDsは8件、Compatible IDsは2件、T1固有は0件だった。T1を元の `NVMe 0:1` に戻した後も同じだった。今回はVM、R0/R1、空の第2コントローラーへの照会・操作をしていない。このビルドはMicrosoftの26H2一般提供チャネルの公開表にも掲載されている。[Windows 11のリリース情報][release]

上流mainは `eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c` のまま。レビュー済みソースの `boundary.rs` 242–269行にある処理は「旧値を再読、設定、読戻し」で、`real_apply_gate` は `RealApplyUnavailable` を返す。Windows側についてはMicrosoft Learnに加え、Microsoft公開の `win32metadata` をコミット `a40ed3186943620f304649a077b78f8271034736` で固定して関数宣言を調べた。公開ヘッダーは呼出し側が渡せる引数の根拠になる。対象ビルドの内部スケジューリングや非公開のロックの動作はこれから推定していない。[既存ソースの審査](P04-source-build-review.md)／[SetupAPIの宣言][set-header]／[Configuration Managerの宣言][cm-header]

## 使える機能と、保証が途切れる位置

| 審査した経路 | 得られる保証 | 契約の判定 |
|---|---|---|
| `DevCreateObjectQuery` の `DEVPROP_OPERATOR_NOT_EXISTS` と個体別setter | 未作成の条件でオブジェクトを検索でき、結果の追加・更新通知も受けられる。`SetupDiSetDevicePropertyW` と `CM_Set_DevNode_PropertyW` には対象・型・新値を渡せる。両setterのFlagsは0指定。従来の `SetupDiSetDeviceRegistryPropertyW` にも期待値引数はない。 | 検索の述語は更新呼出しに引き継がれない。検索後の新設や同名個体の再登場を排除できず、条件付き新設としては不採用。[検索の仕様][query]／[検索関数の宣言][query-header]／[述語の定義][operator]／[SetupAPI][set-api]／[CM][cm-api] |
| `AddReg NOCLOBBER`、レジストリの新規作成判定、TxR | `NOCLOBBER` は既存の名前付き値を保持する。`RegCreateKeyExW` の作成結果は新しい**キー**か既存キーかを返す。`RegOpenKeyTransactedW` の保証対象はそのトランザクションに参加するキー操作である。 | `UpperFilters` は既存個体に属するリスト型のプロパティ。補助キーの新設に勝っても、このプロパティやPnPの世代は予約されない。TxRの文書にはコミット前の非トランザクション操作によるロールバックが記載されている。コミット後の遅延書込み、用途変更、SetupAPIによる再始動を同じ保護で囲う根拠は得られない。OS管理の `Enum` を直接編集する案は除外する。[個体別UpperFilters][upper]／[AddReg][addreg]／[作成結果][regcreate]／[TxR][txr]／[PnPレジストリ][regsafe] |
| サービス名の新設、SCM・MSI・PnPの排他 | `CreateServiceW` は同名サービスが存在すれば失敗する。`LockServiceDatabase` はVista以降データベースに効力を持たない。`_MSIExecute` の区間はMSIの指定された実行シーケンスに限られる。`CM_WaitNoPendingInstallEvents` が示すのは呼出し時点の状態である。 | 専用サービス名についての結果を、T1の個体別値の予約やOS全体の排他に転用できない。既存fixtureの `AddService` が `CreateServiceW` と同じ新規作成専用の意味を持つとも仮定しない。[サービス名の衝突][service]／[SCMロック][scm]／[MSIの範囲][msi]／[PnP待機][wait] |
| ハンドルへのPnP通知・remove lock・volume lock | 通知の `DEVICEQUERYREMOVE` には拒否を返せる。ドライバーのremove lockは自身の未完了I/Oと安全なdetach/deleteを管理する。`FSCTL_LOCK_VOLUME` の対象は既存のvolumeである。 | 通知やI/O参照は、区画作成・用途割当・別installerのプロパティ更新を止めない。予告なしの除去経路もある。当該T1は過去の記録でRAW・区画0。volumeロックをディスク全体の保証とはせず、実行もしない。[通知][notify]／[予告なしの除去][surprise]／[remove lock][remove-lock]／[volumeロック][volume] |
| `DIF_PROPERTYCHANGE / DICS_PROPCHANGE` による明示再始動 | 個体を指定できる。`DICS_PROPCHANGE` は有効な `Scope` 値があってもその情報を無視して対象を停止・再始動する。インストーラーが処理に参加し、OS再起動が必要になる場合がある。 | `Scope` は他の書き手や依存デバイスを排除する手段にはならない。子や `RemovalRelations`、参加する処理、ファイル更新、D01・親への波及を別途確定する必要がある。`SetupDiRestartDevices` には対象だけに限れる条件もあるが、特定の `DIF_INSTALLDEVICE` を扱うクラスインストーラー向けで、新規ファイルをコピーせず既設ファイルも書き換えない場合と、共有ドライバーの他機器まで再始動する場合が区別されている。この条件を今回の呼出しには移せない。[状態変更][propchange]／[クラスインストーラーの処理][dif]／[関係デバイス][relations]／[再始動APIの制限][restart] |

**名前の似たINFフラグについて。** `FLG_ADDPROPERTY_NOCLOBBER` は実在し、既存のプロパティ値を置き換えないためのフラグである。これは個体別setterのFlagsには指定できない。また、`UpperFilters` は従来の `SPDRP_UPPERFILTERS` に対応する。Microsoftは、従来からAddRegで設定するこの種類のプロパティには引き続きAddRegを使うよう案内している。仮に新設の一点が不可分でも、その後の別の書き手やPnPに対する保護は下の反例のまま残る。[AddPropertyの仕様][addproperty]／[適用対象の指針][addproperty-guide]／[個体別UpperFilters][upper]

MicrosoftのToasterサンプルも確認した。固定版の `filter.inx` は `[Manufacturer]` のModelsを経由し、`DDInstall.HW` 相当の節で個体別 `UpperFilters` を設定する。DevConの `classfilter` はクラスキーのリストを設定するサンプルである。前者では同じモデルIDの2台を区別できず、後者はD01保護の境界を越える。いずれも今回の仲裁機構には使えない。[ToasterのINF][toaster]／[DevConの処理][devcon]／[DDInstall.Filtersの必須参照][filters]

## 机上で確認した反例

論点は、正しいT1を指す書き手同士でも他者の変更が消えるかどうかである。初期状態は個体別値が未作成。P04側をA、既に読んだリストを保持して自分のサービスQを追加する別installerをBとした。AとBの各内部順序を保つ並びをすべて列挙した。Aが条件を満たさず中止した場合、その後のAの手順は何もしない。以下の件数はこの有限モデルの結果で、Windowsでの発生確率や実機試験値には使わない。

| Aの手順 | Bの手順 | 並びの総数 | Aが既に書かれたQを消す | Aの成功読戻し後にP04が消える |
|---|---|---:|---:|---:|
| 未作成を読む → P04を設定 → 1件を読戻し | 読む → Qを追加した手元のリストを設定 | 10 | 2 | 2 |
| 仮想の不可分な「未作成ならP04新設」 → 読戻し | 同上 | 6 | 0 | 1 |

後者の1通りでは、Bが未作成を読み、Aが不可分に新設して成功を読み戻し、その後Bが古い読み取りに基づいてQだけを書き込む。**Aの新設が不可分でも、別の書き手Bによる遅延書き戻しまでは防げない。** ここで試した仮想操作は、Windowsに実在するAPIの主張ではない。

取消しにも別の境界がある。ゲストの「許可確認 → Windows呼出し」の間にホストが失効を確定できるとする。2操作と失効を並べる3通りのうち、`許可確認 → 失効 → 最初のWindows呼出し` では、呼出しが1回だけでも契約違反になる。契約が許すのは、失効より先に**要求そのもの**を取り消せない形で実行主体に引き渡した場合だけである。許可やログを保存しただけでは引渡しを立証できない。ファイルI/Oの `CancelIoEx` も、任意のSetupAPI要求の発行防止・取消完了を保証する仕組みにはならない。[I/O取消しの保証範囲][cancel]

## 判定を変えられる証拠と、未送信の照会文

次に必要なのは、対象のWindowsリリースに適用できる**具体的なサポート対象の機構、構成、保証区間**である。複数機構の組み合わせでもよい。書き手Bの遅延更新、用途／世代の変化、失効後の初回発行のどれかを防げないなら、その経路は不採用とする。「対応機構なし」という製品担当の回答も、候補の検討を打ち切る根拠になる。肯定回答でも、署名済み一式、実装着位置、実効ACL、R1回復などの既存条件と別の実験承認が残る。

既存の技術保証文書があれば参照先だけで再審査できる。次は、組織内のWindows基盤担当または製品サポートにそのまま渡せる本文である。実名、機体の識別値、VMDKパス、試行用ハッシュは含めていない。

> **件名　Windows 11でのディスク個体別フィルター登録とPnP競合のサポート範囲**
>
> Windows 11 Pro x64、26H2（確認したビルド26300.9974）上で、データ用ディスク1台だけに上位フィルターを登録する方式を設計しています。OSディスクとはHardware IDs・Compatible IDsが一致し、サービスは別途システム単位で用意する想定です。OSディスクのdevnodeへの登録・再構成・再始動・装着はいずれも許容できません。通常のOSによるパッケージ管理書込みは別に扱います。
>
> 1. 同じ現用devnodeの個体別 `UpperFilters` が未作成の場合だけ新設でき、別installerが以前に読んだリストを後から書き戻す場合にも更新を失わない、サポート対象の手段はありますか。拘束される書き手、保護の取得・解放時点、競合時の結果を教えてください。
> 2. その手段を使う場合、登録成立時と再始動の引渡し時まで、対象の世代、RAW・区画0・特殊用途なし、および除去／再始動の依存範囲を保護できますか。共有する機能ドライバーを使うOSディスクや親を再始動させない適用条件も知りたいです。
> 3. 再始動要求と失効を順序付け、失効が先なら新規発行0件、要求の取消不能な引渡しが先ならその1件までと確認できる境界はありますか。プロセス停止・切断・再接続時の扱いを含め、対応API、バージョン、前提、保証対象外の条件をご提示ください。サポート対象の組み合わせがない場合は、その旨をご回答いただければ十分です。

照会は送信していない。MicrosoftはWindows Developer Supportの `Driver Development (WDK)` をサポート契約が必要な有料インシデント窓口として案内している。既存契約の対象か、今回の設計照会を受け付けるかは未確認。契約・課金・送信に関する判断は所有者に残す。[Microsoftの公式窓口][support]

[release]: https://learn.microsoft.com/en-us/windows/release-health/windows11-release-information
[set-header]: https://github.com/microsoft/win32metadata/blob/a40ed3186943620f304649a077b78f8271034736/generation/WinSDK/RecompiledIdlHeaders/um/SetupAPI.h#L6341
[cm-header]: https://github.com/microsoft/win32metadata/blob/a40ed3186943620f304649a077b78f8271034736/generation/WinSDK/RecompiledIdlHeaders/um/cfgmgr32.h#L3971
[query]: https://learn.microsoft.com/en-us/windows/win32/api/devquery/nf-devquery-devcreateobjectquery
[query-header]: https://github.com/microsoft/win32metadata/blob/a40ed3186943620f304649a077b78f8271034736/generation/WinSDK/RecompiledIdlHeaders/um/devquery.h#L39
[operator]: https://github.com/microsoft/win32metadata/blob/a40ed3186943620f304649a077b78f8271034736/generation/WinSDK/RecompiledIdlHeaders/um/devfiltertypes.h#L41
[set-api]: https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdisetdevicepropertyw
[cm-api]: https://learn.microsoft.com/en-us/windows/win32/api/cfgmgr32/nf-cfgmgr32-cm_set_devnode_propertyw
[upper]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/devpkey-device-upperfilters
[addreg]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/inf-addreg-directive
[addproperty]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/inf-addproperty-directive
[addproperty-guide]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/using-the-inf-addproperty-directive-and-the-inf-delproperty-directive
[regcreate]: https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regcreatekeyexw
[txr]: https://learn.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regopenkeytransactedw
[regsafe]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/accessing-registry-keys-safely
[service]: https://learn.microsoft.com/en-us/windows/win32/api/winsvc/nf-winsvc-createservicew
[scm]: https://learn.microsoft.com/en-us/windows/win32/api/winsvc/nf-winsvc-lockservicedatabase
[msi]: https://learn.microsoft.com/en-us/windows/win32/msi/-msiexecute-mutex
[wait]: https://learn.microsoft.com/en-us/windows/win32/api/cfgmgr32/nf-cfgmgr32-cm_waitnopendinginstallevents
[notify]: https://learn.microsoft.com/en-us/windows/win32/api/cfgmgr32/nf-cfgmgr32-cm_register_notification
[surprise]: https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/handling-an-irp-mn-surprise-removal-request
[remove-lock]: https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/using-remove-locks
[volume]: https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ni-winioctl-fsctl_lock_volume
[propchange]: https://learn.microsoft.com/en-us/windows/win32/api/setupapi/ns-setupapi-sp_propchange_params
[dif]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/dif-propertychange
[relations]: https://learn.microsoft.com/en-us/windows-hardware/drivers/kernel/irp-mn-query-device-relations
[restart]: https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdirestartdevices
[toaster]: https://github.com/microsoft/Windows-driver-samples/blob/d49bb7d72f3651952b2b6e4ad28d14fe2247eb69/general/toaster/toastDrv/kmdf/filter/filter.inx#L50
[devcon]: https://github.com/microsoft/Windows-driver-samples/blob/d49bb7d72f3651952b2b6e4ad28d14fe2247eb69/setup/devcon/cmds.cpp#L1688
[filters]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/inf-ddinstall-filters-section
[cancel]: https://learn.microsoft.com/en-us/windows/win32/fileio/canceling-pending-i-o-operations
[support]: https://developer.microsoft.com/en-us/windows/support/
