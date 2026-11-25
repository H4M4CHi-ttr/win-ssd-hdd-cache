# VM-AでWinSpd Coreを1回導入し、W0に戻す手順案

2026-11-25 JST。**未実行・所有者の承認待ち。** 目的は、現在のVM-Aのセキュリティ設定を保ったまま、固定のWinSpd 2021 BetaのCoreが新規導入でき、Windowsが仮想アダプターとカーネルドライバーを開始するかを確認すること。終了後は成功・失敗とも、ホスト側からこの試行の直前に作る停止状態の復元点W0へ戻す。

## 着手前に確認した事実

| 対象 | 今回の証拠 |
| --- | --- |
| 対象VM | 既存の私的照合記録、ホスト台帳、Workstationの対象コンソールが一致。ゲストでもペアリング済み実行先とVMwareのネットワーク識別情報を非公開で再照合した |
| 復元点 | 10:29、既存2件。R1の親はR0、現在位置の記録はR1。両状態ファイルの存在と、R1の状態ファイル時刻の既存記録との一致を確認。W0という名前は未使用 |
| OSディスク | 通常の従属ディスク、仮想容量200 GiB。現在の記述子は3層。親CIDの2リンク、R1・R0を通る枝、既知のベースと全記載extentの存在を確認 |
| T1 | 通常の従属ディスク、仮想容量8 GiB。現在の記述子は2層。親CIDの1リンク、R1の裏付けディスクと既知のベースを確認。OSとT1の記述子・extentの経路は別。未接続の旧候補は参照されていない |
| ホスト容量 | VM・スナップショットの作業場所と両ディスクの現在／ベースの保存先は同一ホストボリューム。利用可能な空きは**合計249 GiB**（整数切り捨て）。親子連鎖と構成メタデータは照会の前後で安定していた |
| ゲスト | Windows 11 Pro 26H2 x64、26300.9974。Secure Boot・VBSは無効、HVCIは未設定・非稼働。Windows Driver Policyは評価モード。既設確認ではWinSpdとLauncherのサービス、関連PnPデバイス、第三者パッケージ中の元INF、アプリ登録はいずれも0件 |
| ゲストの容量・稼働前提 | 10:37、OSと一時フォルダーは同一ボリュームで利用可能な空きは各7 GiB（共通、整数切り捨て）。PnPで接続中2台、既知の親・LUNでOS側1台／データ側1台と照合。Get-Disk／Get-PartitionはCIM AccessDeniedのため、現在のGPT・区画数／RAWは再確認できていない。Windows Update・CBSの再起動待ちキーなし、Installer InProgressは32／64ビットともなし、msiexecプロセス0件 |
| 既存登録と保留操作の補足 | 10:43、固定ProductCodeのProductStateは−1（UNKNOWN）、UpgradeCodeの関連製品0件。WinSpd.Eject、WinSpd.DiskFile、2つのInProcServer32と、WinSpd／Servicesキー（32／64ビュー）に該当登録なし。PendingFileRenameOperationsは3組でWinSpd固有と識別したもの0組、残り3組はその他・帰属不明。対象を先回りして削除せず、通常再起動を要するかはこの値だけで確定しない |

249 GiBは測定時の空きであり、将来の差分成長やホストの他用途に対する予約量ではない。停止状態で新規復元点を取る直前とインストール前に再測定する。変化が大きい場合は処理を止め、容量の確保や既存スナップショットの削除を自動で行わない。ゲストの7 GiBとホストの249 GiBは別の空きで、ファイル表の合計はインストール時の最大消費量を表さない。VMDKの確認は記述子とextentのメタデータに限定した。W0自体の復元はまだ検証していない。[Workstationのスナップショットファイル保存先](https://knowledge.broadcom.com/external/article?legacyId=2007069)・[VM-Aで確認したポリシーと既設状況](WinSpd-VM-A-preflight-2026-11-25.md)

## 導入するものと変更範囲

対象は公式 `v1.0B1` の `winspd-1.0.20357.msi`（2,195,456バイト、SHA-256 `F1157EEF805DCBEC78A477F2B4EE5ABC0049C8A9329444E5D18CAB01D3604265`）。ホストで署名 `Valid`。MSIを読み取り専用で開き、機能の内部名と実行順を確認した。Coreの選択は `ADDLOCAL=F.Main,F.User`。Developerはレベル1000で選択しない。選択機能に属するFile表26行のサイズ合計は936,233バイト。MSIキャッシュ・Driver Store・ログ・更新等を含むゲスト実使用量は実行前には分からない。[公式配布物](https://github.com/winfsp/winspd/releases/tag/v1.0B1)・[MSIの監査記録](../offline-preflight.json)

ゲスト全体への変更を伴う。OSディスクには通常のファイル、Driver Store、レジストリ等が書き込まれる。x64シェル拡張の自己登録、StartNameが空欄（既定LocalSystem）の自動起動サービス `WinSpd.Launcher` の登録と開始、`root\winspd` に対応するSCSIAdapterの登録・更新、要求時起動の `WinSpd` カーネルサービスの初回ロードがあり得る。MSI実行順は自己登録5600、サービス登録5800、デバイス追加5804、サービス開始5900。導入の許可にはこの初回ロードの可能性も含める。P04の登録、元のT1へのフィルター装着、raw書き込みやWinSpdディスクの公開は含めない。W0後にデータを残す必要のある作業は入れない。[固定版のMSI定義](https://github.com/winfsp/winspd/blob/55c53bc454afbbba38bd1692f52beb77ed59142f/build/VStudio/installer/Product.wxs)・[サービス登録の既定アカウント](https://learn.microsoft.com/en-us/windows/win32/msi/serviceinstall-table)・[デバイス追加時の処理](https://github.com/winfsp/winspd/blob/55c53bc454afbbba38bd1692f52beb77ed59142f/src/devsetup/devsetup.c#L226-L259)

## 承認後の実施順

1. **ホストで本人照合し、ゲストを正常終了する。** 並行して保存が必要なユーザー作業がないことを前提とする。対象の台帳、枝、2台の接続、当日の空きと名前W0の未使用を再確認する。初回の正常終了はW0より前に行われる。既存の保留操作やWindows Updateを実験目的で処理・消去しない。更新を明示的に選ぶ必要がある場合や正常に停止できない場合はそこで保留する。
2. **停止状態でW0を新規取得する。** ホストのWorkstationで名称W0、用途を示す説明を設定し、この回のVM・W0の識別記録をホスト側に私的に保持する。R1の子としてW0が1件追加され、両方の従属ディスクを含み、R0・R1の個体と既存のディスク枝が保持されることを確認する。ホストからW0を選択できることを照合してから通常起動する。暗号化認証が必要なときだけWorkstationの私的入力を使う。
3. **導入直前の基準値をホストに保存する。** 新しい起動で、ゲストとVM-Aの対応、build、セキュリティ状態、2台のPnP対応、既存のWinSpd関連MSI・SCM・PnP・Driver Store・シェル拡張・Launcher設定を再確認する。W0の各裏付けディスクからアクティブな差分が派生したこともホストで確認する。ディスク番号や名称だけで同一性を決めない。更新中、別のMSI実行中、既存WinSpdの修復・アップグレードになる場合、対象不一致や復元経路の不確実性があればMSIを起動しない。再起動待ちのマーカー、MSIの製品／関連製品、Launcherやシェル登録は上表と比較する。3組の保留中ファイル名変更の存在だけで活動中のインストールとは判定しない。通常起動で保留操作が処理されたら変化を記録し、buildや評価／強制モードが変わった場合は今回の評価環境と混同せずMSI起動前に再判断する。
4. **ゲストの専用フォルダーに固定MSIを用意し、1回だけ実行する。** 既存ファイルに上書きせず、ゲストでサイズ、SHA-256、発行元署名を照合する。既存の接続で公式HTTPS配布物を取得するか、許可された既存の転送経路でホストの同一ファイルを渡す。共有フォルダーやネットワークの設定変更はしない。普通のWinSpd導入に必要なゲスト内UACだけを使用する。次は将来の引数の指定で、今回 `msiexec` は実行していない。

   ```text
   /i  <検証済みのゲストローカルMSI>  ADDLOCAL=F.Main,F.User  /passive  /norestart  /L*V  <新規のゲストローカルログ>
   ```

   実行ファイルはゲストの正規の `msiexec.exe`。上記7引数を別々のトークンで渡す。ホストでは日本語と空白を含むダミーパスを使い、インストーラーを起動しない受信器で7個が完全一致することを確認した。`REINSTALL`、`REMOVE`、`ADDLOCAL=ALL` は指定しない。所有者・ログイン先を取り違えた場合、UACのキャンセル、実行中に接続が切れて成否不明になった場合も二重起動しない。まず元の試行のプロセス、終了値、ログと状態を読み戻す。[ADDLOCALの仕様](https://learn.microsoft.com/en-us/windows/win32/msi/addlocal)・[Windows Installerの引数](https://learn.microsoft.com/en-us/windows/win32/msi/standard-installer-command-line-options)
5. **終了値と実際の開始を別々に判定する。** `0` はMSI成功、`3010` はMSI成功・要再起動。必要な証拠を先に取り、承認に含まれた場合のみ、この導入に起因する要求に対して通常再起動を1回行う。上流のカスタムアクションはデバイス追加の3010をいったん成功に変換し、後段の `CheckReboot`／`ScheduleReboot` で再起動要求を扱う。`1641` は再起動が既に開始されたことを示すため、予定外として記録し追加再起動やMSI再投入はしない。キャンセル・失敗・別MSI実行中（例1602・1603・1618）は先へ進まず原因と状態を記録する。[MSI終了値](https://learn.microsoft.com/en-us/windows/win32/msi/error-codes)・[カスタムアクション](https://github.com/winfsp/winspd/blob/55c53bc454afbbba38bd1692f52beb77ed59142f/build/VStudio/installer/CustomActions/CustomActions.cpp#L59-L105)
6. **結果を保存し、ホストからW0に戻す。** 導入成否にかかわらず、W0以降の変更を破棄する承認を前提に、まず匿名化した結果をホストに退避する。正常終了を確認し、WorkstationのSnapshot Managerでこの回のW0を識別して復元する。停止時のW0から通常起動し、再び同じVMで下表の復旧判定を行う。R0・R1・W0は保持する。復元不能、正常停止不能、あるいはこの試行以外の作業が入った場合はそこで停止する。強制電源断、古いR1/R0への代替復元、アンインストール・repairによる帳尻合わせは承認外。

## 判定と報告

| 項目 | 何をもって確認するか |
| --- | --- |
| MSI導入 | この試行の終了値、対象製品コード／版の登録、Coreがlocal・Developerがabsentの実状態を確認。個体識別情報は私的に照合し、ログ全体はチャットへ出さない |
| 仮想アダプターと初回ロード | 新規の該当Hardware IDが1台。現在接続・クラスSCSIAdapter、使用中のINF・サービスが固定版WinSpdに一致し、devnodeがstartedかつ問題コード0、カーネルサービスがRUNNING。導入したSYSファイルのハッシュも固定配布物と照合する。ファイルがある、サービスが登録された、候補ドライバー一覧に出た、だけではロード成功としない。メモリ中の機械語の再測定までは主張しない |
| 副作用 | Launcherの自動起動設定・実状態、Coreのシェル登録（DriveのWinSpd.Eject、WinSpd.DiskFileとMount/EjectのInProcServer32）を読み取る。シェル拡張をクリックして動かす試験は行わない。WinSpdによる新規DiskDrive・volumeは0件を期待し、既存2台の個体・接続・特殊用途に異常がないかを照合する。意図しないデバイスや起動異常があれば中止。新しいstorage unit作成、mount、format、`rawdisk` や `stgtest` の実デバイスモードは使わない |
| 失敗の切り分け | 当該時間帯・対象に絞り、MSI、SetupAPI.dev.log、Kernel-PnP、Code Integrityの関連情報を照会する。3076（監査）と3077（実ブロック）を対象イメージ・該当policyと突合し、ログがないだけで互換とはしない。取得拒否は理由とともに未確認とする |
| W0復旧 | ホストでW0・R0・R1の識別、両VMDKの枝と親CIDを再照合。ゲストで正常起動、導入直前と同じWinSpd製品・サービス・PnP・パッケージ・シェル／Launcher設定の状態に戻り、実験用MSI・ログ等の新規一時物が消え、OS/T1が従来の役割であることを確認。通常起動が更新する全OSファイルのバイト一致は要求しない |

報告では「W0取得」「MSI導入」「ドライバー開始」「W0復旧」を分け、各項目を成功・失敗・未確認のいずれかで返す。Driver Policyが評価モードでSecure Boot・HVCIが無効なVM-Aで得た結果は、その環境に限る。[デバイスの実際の使用INF](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/devpkey-device-driverinfpath)・[デバイス開始状態](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/devpkey-device-devnodestatus)・[Windows Driver Policy](https://support.microsoft.com/en-us/windows/hardware/drivers/the-windows-driver-policy)

## 所有者にお願いする承認

**VM-Aだけで「正常終了・W0作成 → 固定版WinSpd Coreの新規導入1回と初回ロードの確認 → 必要時の通常再起動1回 → 結果を退避し、成功・失敗ともW0へ復元・通常起動」までを一括で承認する。W0以後のゲスト変更は失われる。** これは今後の承認文案であり、今回はW0も導入も未実施。ホストへの導入、セキュリティ設定変更、製品ディスク公開、P04、実機障害試験、GitHub投稿は含めない。
