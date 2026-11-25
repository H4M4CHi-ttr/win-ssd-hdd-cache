# P04　T1実登録前の排他と署名済みパッケージの配備範囲

2026年11月20日 JST。対象はVM-AのWindows 11、保護するOSディスクD01と、P04の観測対象に予定するデータディスクT1。T1は利用者のAFTER_R1記録で8 GiB、RAW、パーティション0件。今回の根拠はその記録、レビュー済みソース、Microsoftの公開仕様である。

## 判定

| 問い | 判定と理由 |
|---|---|
| 他のinstallerやPnPから、登録開始前～最終読み戻し完了までを保護できるか | **現行方式はNO-GO。** PnPの待機はその時点の状態を返す。T1のフィルター一覧を設定するAPIは、期待する旧値やデバイス世代を指定できない。両者を組み合わせても、その区間の全書き手と初回ロードを排除する保証は確認できない。 |
| 署名済みパッケージを、対象外ディスクのデバイス構成を変えずに準備できるか | **Driver Storeへの事前配置は、デバイスへのインストールと分離できる。** ただし配置先はゲスト共通。パッケージの適用条件、付随INF、他ディスクに残ったサービス参照、後のPnP処理は別に評価する必要がある。レビュー済み成果物には実配備用の署名済み一式がなく、T1限定の実登録はNO-GO。 |
| 対象外ディスクへのファイル書き込みもゼロにできるか | **D01上で稼働するVM-Aへの新規配備では成立しない。** 正常な事前配置でもWindowsのシステム領域にINF・カタログなどの管理情報が入る。D01のdevnodeや既存フィルターを保護する要件とは影響が異なる。 |

ここでのNO-GOは、公開契約と現行実装で必要な保証が成立しないという判定である。Windowsの内部実装に一切のロックが存在しないという結論は導いていない。[待機APIのRemarks][wait]／[デバイスプロパティの設定][set]／[Driver Store][store]／[カタログの登録先][catalog]

## 1　どの区間を保護する必要があるか

レビュー済み仕様REG-3の開始点は、パッケージ配置 **Pの最初の副作用より前**。排他が成立した状態でD01とT1を再同定する。その後、P、要求時起動サービスの準備 **S**、サービスにT1の完全なインスタンスIDを書き戻して確認する **K**、T1のdevnodeの既存UpperFiltersにP04を1件追加する **F** を経て、最終照合 **Vの最後の読み取りまで**同じ保護が必要になる。区間内では他の書き手による変更、devnodeの消失・再列挙、予定外の初回ロードを防ぎ、各段階で保護の継続を確かめる。VではT1の列とサービス・selectorに加え、D01・他devnode・クラスの保護対象の状態も照合する。

固定ソースでは、Windowsの非公開関数 `registration_prefix` がP/S/K/Fを扱う。F自身は同じdevnodeの値を読み戻す。全体のVと排他の世代検査が動くのは合成モデルで、実Windowsでこの区間を確保する入口はない。ドライバーは `DriverEntry` でselectorを一度読み、`AddDevice` で完全なIDを比較する。これで誤ったPDOへの装着を見送れても、既に別devnodeへ書いたフィルター名やパッケージの適用は取り消せない。要求時起動のサービスも、該当デバイスを構成する際にPnPからロードされ得る。[デバイスインスタンスID][instance]／[PnPドライバーのロード条件][load]

| 候補 | Microsoftが定める範囲と、残る競合 |
|---|---|
| PnPの待機・通知 | `CM_WaitNoPendingInstallEvents` の成功直後にも新規イベントが起こり得る。`CM_Register_Notification` は通知を届ける仕組みで、通知登録と列挙の間にもデバイスの状態は変わり得る。どちらにもP～Vを予約する権利はない。[待機][wait]／[通知][notify] |
| インスタンスID指定、設定直前・直後の再読 | `SetupDiOpenDeviceInfoW` は指定したインスタンスの情報要素を返す。`SetupDiSetDeviceRegistryPropertyW` はその要素のプロパティへ新しいバッファを設定する。旧値と一致した場合だけ更新する引数はない。正しいT1を選択できても、他者が更新した列との競合は残る。[インスタンスを開くAPI][open]／[設定API][set]／[UpperFiltersの定義][upper] |
| 独自mutex、Windows InstallerやSCMのロック | 独自mutexに従うのは同じオブジェクトを待つ参加者。`_MSIExecute` はMSIの所定の実行シーケンス中だけ使われる。現行Windowsの `LockServiceDatabase` は互換用ハンドルを返しても実際のロック効果がない。Driver Store・PnP・サービス・devnodeをまたぐ排他には使えない。[Mutex Objects][mutex]／[MSIの範囲][msi]／[SCMのRemarks][scm] |
| VM隔離、スナップショット、インストール制限 | VM隔離・R1は事故の範囲と復元先を限定する。ゲスト内の書き手は残る。デバイスID・インスタンスIDに基づくポリシーはインストール可否を制限する機能で、サービスやフィルター列を含めて区間を占有する仕組みではない。オフラインイメージへの配置も、起動後にはPnPが該当デバイスとの関連付けを行うため、稼働中に行うVまでの保証は引き継げない。[デバイスのインストール制限][policy]／[オフラインイメージと起動時PnP][offline] |

**読み戻しが成功しても検出できない競合がある。** たとえば元の列が「既存A」で、P04の最終比較後に別installerが「既存B」を加える。その直後にP04が準備済みの「既存A、P04」で全体を設定すると、読み戻しは予定値と一致し、既存Bだけが失われる。Sでも同名サービスの不在確認からサービス設定までに別の書き手が入り得る。事後の一致、空き時間の確保、一定時間イベントがなかったという観測では、この問題を解消できない。[プロパティ設定の引数][set]／[サービスセクションの設定][services]

## 2　署名・配備・T1限定登録の境界

**Pで証明できる範囲。** Microsoftは `SetupCopyOEMInf` による事前配置と、個別デバイスへのインストールを分けている。レビュー済みの候補が使う `SP_COPY_NOOVERWRITE` は既存INFがあれば `ERROR_FILE_EXISTS` を返すための指定で、他の書き手を締め出さない。また `CopyINF` があれば、Windows Vista以降はStoreへの取り込み時点で参照先のパッケージも再帰的に取り込まれる。したがって「配置APIを一度呼ぶ」だけでは1パッケージに限定できない。後日、対応するPnPパッケージが接続や再列挙時に選択される場合もある。[事前配置][pre]／[コピーのフラグ][copy]／[CopyINFの処理時点][copyinf]

**今回のINF。** 固定された案はAMD64用のprimitive driver形式で、`Class=System`、`DefaultInstall.NTamd64`、`DIRID 13`、1個のSYS、1件の `AddService`（flags 0、`StartType=3`）を指定する。通常のPnP機種照合に使うManufacturer／Models、追加のINFやクラスフィルターを指定する記述はない。これはパッケージ側でD01を機種照合する経路を減らす。PのStore配置、Sのサービス、Kのサービス設定はゲスト共通で、個体を宛先にする書き込みはFで行われる。`PnpLockdown=1` の保護対象はINFがコピーするファイルである。[primitiveの要件][primitive]／[DIRID 13][dirid]／[PnpLockdownの適用範囲][lockdown]

**処理をまとめると前提が変わる。** Windows 10 1903以降のprimitive driverについて、Microsoftはシステムへのインストールに `DiInstallDriver` を案内し、対応する装飾付きDefaultInstallを `InstallHInfSection` に渡すと条件に応じて処理を転送する。一方、現行の分割案はPの後にサービスが不在であることを確認し、Sで所定の `.Services` だけを処理する。このP/S分割で最終的な署名済みprimitiveがサポートされるライフサイクルと所期の効果順を満たすという根拠は未取得である。一括インストールへの単純な置き換えでは、Pで許可した効果を超える。[primitiveの推奨API・互換動作][primitive]／[サービスセクションの効果・再起動要求][services]

**T1以外への作用。** 通常のPnPパッケージや拡張INFの適用判定は、デバイスが報告するhardware ID／compatible IDに基づく。署名済みでも、両ディスクに共通するIDならD01を区別できない。`PnPUtil /install` と通常のPnPパッケージに対する `DiInstallDriver` は一致するデバイスを対象にし得る。`DiInstallDevice` にはpresentな1個体を渡せるが、署名済みパッケージ自体の後日の適用範囲まで絞る機能はなく、追加INFを適用可能な他デバイスにインストールし得るフラグもある。宣言的な `DDInstall.Filters/AddFilter` を検討する場合も、対象ディスクだけに一致する根拠と既存フィルターの順序を別設計として確認する必要がある。VM-AでD01とT1のhardware IDが実際に同一だとは仮定していない。[通常の照合][match]／[拡張INFの照合][extension]／[PnPUtil][pnputil]／[DiInstallDriver][di]／[DiInstallDevice][one]／[device-specific filter][filter]

**署名の意味。** CATの信頼できる署名、INFと全参照ファイルのカタログへの収録、署名対象バイトと配備する実物の一致を確認する。署名後のINF編集は署名を無効にする。署名は発行元・完全性の根拠であり、ディスクの適用範囲や動作を認定するものではない。固定版が同梱する配備案は未署名のINFだけで、CAT／SYSの実物は同梱されず、`deployment_review` も常に拒否する。対象Windowsの署名ポリシーに沿ったパッケージ受理と、後続のカーネルロード／HVCI適合も別判定になる。[Storeの信頼・完全性検査][store]／[カタログと改変][catalog]／[署名とメンバー検証][verify]／[カーネル署名ポリシー][sign]

## 3　NO-GOを見直すために必要な証拠

| 必要な証拠 | 不足時、または途中で失われた場合 |
|---|---|
| **区間全体の排他。** いつ取得し、誰のどの操作を止め、いつ解放するかを、対象Windowsでサポートされる仕様に対応づける。Store、サービス／selector、対象devnode・クラスの他の書き手、PnP再列挙と初回ロードを含む。保護下でT1とD01を再照合し、排他が失効したことを次の変更前に判別できること。 | 根拠がなければ**Pも開始しない**。途中で失効すれば次の変更を発行せず、確認済み段階と効果不明を分ける。 |
| **承認対象となる正確なパッケージ。** 最終INF、署名済みCAT、SYSのハッシュ・署名者・信頼・メンバーシップ、対象ビルドで有効な全セクションと参照先を固定する。検査した実体と配備する実体の差し替えを防ぐ。P単独が承認済み1パッケージの配置だけ、Sが当該サービス1件の準備だけで、ロード・再起動・既存サービスの上書きを伴わないことを示す。 | 付随INF、予期しない適合先、サービス衝突、先行ロード、再起動要求、受理条件不明、元ファイルと配備先の不一致は停止理由。署名の存在だけでは進めない。 |
| **T1以外の保護。** T1をストレージ固有ID・親位置・LUN・VMDK連鎖などで一意に同定し、特殊用途からの除外を再確認する。D01・他devnode・クラスにP04を参照する既存設定がなく、Fの宛先がT1だけで、既存の順序・表記・他の設定が保たれることをVまで確認する。 | 番号や容量だけの一致、競合の疑い、対象世代・比較元・用途の不明、対象外へのインストール／再構成があり得る状態はNO-GO。 |
| **失敗時の扱い。** 呼び出した段階、戻り値、独立した読み戻しを保持する。排他・対象・書き込み結果を確認できない場合は、調査可能な状態で止める。 | 自動再送、古いリストの書き戻し、既存フィルター削除は行わない。R1は運用上の復旧手段で、排他の証拠には使えない。 |

**現時点で実験許可だけを追加しても、判定は変わらない。** 次の机上判断に必要なのは、上表の区間全体を覆う仕様上の根拠と、正確な署名済み配布物のオフライン審査である。その排他機構を示せない場合は、T1に固有のWindows照合条件とOSの宣言的なフィルター登録で、現在の複数書き手による上書きを避けられるかを別設計として再審査する。現行の手順や条件をそのまま実行へ移せるという判定ではない。

## 照合した実装

基準コミット `eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c`、レビュー受入済みツリー `f1fb5c4f9f459edd2aeebfc490bef246f4a2f7d7`。固定ソースZIPのSHA-256は `c298424aca9d4785d3b642258ccbf195242b199a7938c563b8695d2b43935806`。以下の7ファイルについて、読み取ったソースが同ZIPのバイトと一致することを確認した。ファイル名と行は配布済み実装ZIPの `source/` を起点とする。[前回の独立レビュー](P04-source-build-review.md)／[レビュー済み実装一式](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/tree/153fac3473186241ccf3f354e59322e63b1f183f)／[登録仕様REG-3](P04-T1-cancel-spec.md)

- `crates/cache-probe-registration/src/lib.rs` 469行、`src/synthetic.rs` 610行（同crate）。実適用の拒否と合成V。
- `crates/cache-probe-registration-win/src/boundary/stages.rs` 122行、`src/boundary.rs` 242行（同crate）。P/S/K/FとF前後の読み戻し。
- `crates/cache-probe-registration-win/fixtures/p04-service-only.inf`、同crateの `src/package.rs` 38行・79行。パッケージの許可範囲と署名未取得。
- `experiments/stack-trace/probe/wdk/driver.c` 72行・98行。selectorの取得時点と装着前の照合。

[wait]: https://learn.microsoft.com/en-us/windows/win32/api/cfgmgr32/nf-cfgmgr32-cm_waitnopendinginstallevents
[notify]: https://learn.microsoft.com/en-us/windows/win32/api/cfgmgr32/nf-cfgmgr32-cm_register_notification
[mutex]: https://learn.microsoft.com/en-us/windows/win32/sync/mutex-objects
[msi]: https://learn.microsoft.com/en-us/windows/win32/msi/-msiexecute-mutex
[scm]: https://learn.microsoft.com/en-us/windows/win32/api/winsvc/nf-winsvc-lockservicedatabase
[open]: https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdiopendeviceinfow
[set]: https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupdisetdeviceregistrypropertyw
[upper]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/devpkey-device-upperfilters
[policy]: https://learn.microsoft.com/en-us/windows/client-management/client-tools/manage-device-installation-with-group-policy
[offline]: https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/add-and-remove-drivers-to-an-offline-windows-image?view=windows-11
[store]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/driver-store
[catalog]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/catalog-files
[pre]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/preinstalling-driver-packages
[copy]: https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupcopyoeminfw
[copyinf]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/inf-copyinf-directive
[primitive]: https://learn.microsoft.com/en-us/windows-hardware/drivers/develop/creating-a-primitive-driver
[dirid]: https://learn.microsoft.com/en-us/windows-hardware/drivers/develop/run-from-driver-store
[lockdown]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/inf-version-section
[services]: https://learn.microsoft.com/en-us/windows/win32/api/setupapi/nf-setupapi-setupinstallservicesfrominfsectionw
[load]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/specifying-driver-load-order
[match]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/overview-of-the-driver-selection-process
[extension]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/using-an-extension-inf-file
[pnputil]: https://learn.microsoft.com/en-us/windows-hardware/drivers/devtest/pnputil-command-syntax
[di]: https://learn.microsoft.com/en-us/windows/win32/api/newdev/nf-newdev-diinstalldriverw
[one]: https://learn.microsoft.com/en-us/windows/win32/api/newdev/nf-newdev-diinstalldevice
[filter]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/installing-a-filter-driver
[instance]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/device-instance-ids
[verify]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/verifying-the-release-signature
[sign]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/kernel-mode-code-signing-policy--windows-vista-and-later-

## 独立レビュー結果

**受入。重大な指摘、必須修正ともになし。実登録の判定はNO-GO。** 別コンテキストのAstraレビュアーが、Microsoftの原典、REG-3、レビュー済み実装、上記の調査本文を独立に照合した。正本ZIPと関係7ファイルのバイト一致を再確認し、待機後の競合、事前配置と個体へのインストールの区別、付随INF、primitiveの配備方式、署名の範囲、D01上に生じる通常のシステム書き込みを点検した。

このレビューが受け入れたのは文献とソースに基づく判断である。実Windowsにおける全区間の排他やP/S分離を新たに実証したものではない。今回の作業は既存資料の読み取り、調査報告、独立レビューまで。実装変更、署名、実機での登録・ロード・観測、VMやセキュリティ設定の操作、GitHubへの投稿は行っていない。

レビュー対象本文のSHA-256は `65f5833fa07cdc62d2926010e469b81ae321051d5eb542c3cea42d90d30b3b0c`。この節はレビュー完了後に追記した。
