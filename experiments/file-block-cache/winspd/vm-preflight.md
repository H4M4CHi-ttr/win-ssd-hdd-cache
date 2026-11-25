# VM-AでWindowsのディスク列挙を調べる前の記録

当日の事前調査記録。後続のVM試験の結果は[Core導入・W0復元](lab/WinSpd-VM-A-Core-W0-userready-result-2026-11-25.md)を参照。

2026-11-25 JST。次の検証候補は、隔離したVM-Aで、WinSpdが用意する新しい読み取り専用の仮想ディスクをWindowsがどう列挙するかである。現段階の許可は通常ユーザーのテストプロセス、固定版DLL、ローカルパイプ、新規の通常ファイル2本まで。今回はホスト上のパッケージと既存VMwareメタデータの読み取りに留めた。

## 配布物を実行せずに分かったこと

対象は公式 v1.0B1（2020-12-22）の `winspd-1.0.20357.msi`。SHA-256は `F1157EEF805DCBEC78A477F2B4EE5ABC0049C8A9329444E5D18CAB01D3604265`。MSIデータベースは `OpenDatabase(..., 0)` で読み、その固定配布物のINF、CAT、SYS、DLLをローカルの一時フォルダーで静的に照合した。[公式リリース](https://github.com/winfsp/winspd/releases/tag/v1.0B1)・[固定版のインストーラー定義](https://github.com/winfsp/winspd/blob/55c53bc454afbbba38bd1692f52beb77ed59142f/build/VStudio/installer/Product.wxs)

| 対象 | 実ファイルまたはMSIテーブルでの結果 |
| --- | --- |
| x64 SYS | SHA-256 `E2E5E2E02452F2A4D09E9EC3C5375D2D8512D075523A3A6BF88E246B343184C9`。PEの証明書テーブルと入れ子のCMSを展開すると、ベンダーの署名のほか `Microsoft Windows Hardware Compatibility Publisher` の葉証明書を確認。Windowsの一般Authenticode検証を署名インデックス別に実行し、Microsoft署名のインデックス1は成功した |
| x64 CAT | SHA-256 `BCAFF78F0801E64E11B39F2BF790E24B25C88251D7BDB9BC40C245B9A3D0CF93`。列挙したCMS署名者はどちらも `NAVIMATICS LLC`。一般検証はインデックス0で成功し、インデックス1を明示した呼び出しは `0x800B0100`。すべての署名で成功したとは扱わない |
| カタログに含まれるファイル | `CryptCATAdminCalcHashFromFileHandle2` で求めたSHA-1のカタログ参照タグが、INF、x64 SYS、x64 DLL、WOW64用x86 DLLの4件すべてで一致。上表の配布物識別用SHA-256とは用途が異なる |
| x64 INF | クラスは `SCSIAdapter`、モデルは `root\winspd`。x64 SYS、x64およびx86 DLLをコピーし、要求時起動のカーネルサービス `WinSpd` をアダプターに関連付ける。調べたINFにDiskDriveクラスのUpperFilters／LowerFilters登録はない |
| 通常のCoreインストール | per-machine（`ALLUSERS=1`）。`F.User` にminiportだけでなく、自動起動する `WinSpd.Launcher` サービス、インストール時のサービス開始、x64シェル拡張の自己登録が含まれる。Developerは別機能。x64の初回導入・repair時にはデバイス追加のカスタムアクションが予定される |

署名のインデックス検査はホストの `WINTRUST_ACTION_GENERIC_VERIFY_V2` を用いたキャッシュ限定・オンライン失効確認なしの結果。SYSにMicrosoft署名があることから、特定のWindowsビルドでのHLK合格、VM-AのCode Integrityポリシーによるロード許可、HVCI互換性までは判断できない。Microsoftの現行Driver Policyには監査と強制の状態があり、対象システムではWHCP署名や所定の許可リストが判断に関わる。試すためのポリシー緩和は予定しない。[Windows Driver Policy](https://support.microsoft.com/en-us/windows/hardware/drivers/the-windows-driver-policy)・[Microsoftの署名方式](https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/driver-signing-offerings)・[メモリ整合性との互換性](https://learn.microsoft.com/en-us/windows-hardware/drivers/bringup/device-guard-and-credential-guard)

同梱のデバイス追加ヘルパーは、必要に応じてrootデバイスを登録し、ハードウェアIDを指定してドライバーを更新する。既存の同じIDのデバイスがあれば影響範囲が変わる。MSIの一括導入、個別のINF配備、デバイス作成、最初のロードは別の作用を持つ。いずれも今回実行していない。上流のサンプル `rawdisk` はファイルを読み書き用に開き、空のファイルではパーティション表の作成を試みる。このサンプルや、既存ボリュームを対象にする `stgtest` は次の読み取り専用照会に使わない。[デバイス追加の実装](https://github.com/winfsp/winspd/blob/55c53bc454afbbba38bd1692f52beb77ed59142f/src/devsetup/devsetup.c#L226-L259)・[同じIDへの更新範囲](https://learn.microsoft.com/en-us/windows/win32/api/newdev/nf-newdev-updatedriverforplugandplaydevicesw)・[サンプルのファイル初期化](https://github.com/winfsp/winspd/blob/55c53bc454afbbba38bd1692f52beb77ed59142f/tst/rawdisk/rawdisk.c#L269-L331)

[匿名化したMSIテーブル・署名照会・ホスト側メタデータの検査結果](offline-preflight.json)

## ホスト側で再照合した範囲

ローカル台帳4件のうち、所有者が指定したWindows 11表記は1件。VMwareの保存済みメタデータにはスナップショットが2件あり、R1の親は従来のR0、現在位置の参照はR1。R1には既存OSディスクとT1の2接続位置が記録され、参照先ファイルの存在を確認した。確認時のホスト上のVMX実行プロセスは0件、VM-Aのディレクトリのサスペンドファイルは0件で、設定に再開用ファイルの指定もなかった。今回はゲストの起動、各VMDKの親子連鎖全体の検証、復元テストは行っていない。

## 次に所有者の指定が必要な範囲

次はVM-Aを通常起動して、既存のペアリング先でWinSpd向けの事前情報を**読むところまで**。以前のVM-A起動と限定UACはT1準備・P04の対象照合のためで、今回のWinSpd用起動・照会を実施したことにはならない。

1. ホストで本人確認済みVM-Aと実行先を照合し、R0・R1を保持する。通常起動以外の電源操作、構成編集、スナップショット作成・復元はこの段階には含めない。
2. ゲストの現在のbuild／アーキテクチャ、Secure Boot、VBS・メモリ整合性の設定と実行状態、利用できる範囲でWindows Driver Policyの状態を照会する。追加権限が必要なら事前に許されたゲスト内の該当読み取り照会に限る。取得拒否や未対応項目は不明として残す。
3. 既存のWinSpdデバイス・Driver Storeパッケージ、カーネルサービス、Launcher、既存インストールの有無と版を読み取る。稼働中の同一ID、利用元不明の既存デバイス、repair・upgradeの可能性を記録する。返すのは件数・版・状態とエラーまでとし、instance IDや私的なパスは出さない。
   照会には、必要に応じて `Win32_DeviceGuard` の `SecurityServicesConfigured` と `SecurityServicesRunning`、Secure Bootの読み取り、`CiTool -lp -json` のローカル解析、SCMとPnPの読み取り専用一覧を使う。Driver Policyの評価／強制の識別にはMicrosoftが示す該当ポリシーのID・認可・有効状態の組合せを用いる。不明を無効扱いしない。アプリの有無はUninstallレジストリを読み、MSI修復を誘発し得る `Win32_Product` は使わない。[Driver Policyの状態確認](https://support.microsoft.com/en-us/windows/hardware/drivers/the-windows-driver-policy)・[CiToolの照会](https://learn.microsoft.com/en-us/windows/security/application-security/application-control/app-control-for-business/operations/citool-commands)

4. 結果を基に、専用VM・復元手段・現在のセキュリティ設定を維持したまま行える導入試験があるかを判断する。既存WinSpdがある場合は、新規導入と同一視しない。判定が難しい場合もインストールによる確認には転じない。

OSに公開する将来の候補は、専用アダプターが新しく提示する読み取り専用の実験ディスクである。D01にも元のT1にも登録・装着しない。現在の試験実行器はローカルパイプ専用で、実デバイスへ切り替える経路を用意していない。将来の組み込み・導入・ロード・公開・復元は、上の照会後に対象と影響を示して別途判断する。P04の実登録、D01、F09の受入状態はこの試験では変更しない。
