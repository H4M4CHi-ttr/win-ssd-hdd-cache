# VM-AのWinSpd事前調査（2026-11-25 JST）

**読み取り専用の事前調査は完了。** ゲストのWindows Driver Policyは評価モード。今回の照会範囲で既存のWinSpdとの競合を示す痕跡は0件だった。固定版ドライバーの実際の導入・ロード可否は未試験である。

## 実行先と復元点

09:52にホストで所有者確認済みのVM-Aを台帳・私的照合記録と照合し、Workstationの稼働一覧で一致を確認した。すでに起動していたため、新たな起動・再起動は不要だった。09:54にペアリング済みゲストでVMware上のWindowsを確認し、今回の使い捨て照合値に一致するMACは重複除外で1種類だった。

R0・R1の識別記録が一致し、R1の親はR0、現在位置マーカーはR1だった。既存OSディスクとT1の2接続、およびR1に記録された2つのディスクファイルの存在を確認した。追加NVMeコントローラーは空だった。終了時もホストのスナップショット記録とストレージ接続定義は調査前と一致した。完全なVMDK親子連鎖・実復元は今回再検証していない。VM-Aは起動したまま引き渡す。

## ゲストの照会結果

| 項目 | ゲストで得た根拠・判定 |
| --- | --- |
| Windows | Windows 11 Pro 26H2、build 26300.9974、x64 |
| Secure Boot | Windowsが報告する状態値 `UEFISecureBootEnabled=0`。無効 |
| VBS | `Win32_DeviceGuard.VirtualizationBasedSecurityStatus=0`。無効 |
| メモリ整合性（HVCI） | `SecurityServicesConfigured=[0]`、`SecurityServicesRunning=[0]`。設定なし・非稼働 |
| Windows Driver Policy | UAC承認後に `CiTool -lp -json` の実行・解析に成功。全14件のうち評価用は1件（`IsEnforced=true`、`IsAuthorized=true`）、強制用は0件。Microsoftの判定手順では評価モード |
| サービス | `WinSpd` と `WinSpd.Launcher` はどちらもSCMが1060（未登録）を返し、対応する登録キーもなし |
| PnPデバイス | 指定Hardware IDの完全一致は接続中0件・非接続0件。列挙件数はそれぞれ208件・15件 |
| Driver Store | 第三者パッケージ12件の元INF名を確認し、`winspd.inf` は0件。初回はエラー5、ログイン中の非昇格ユーザーで一覧取得に成功 |
| アプリ登録 | HKLMと現在ユーザーのHKCUの各32・64ビットビューでWinSpd本体0件 |

Secure Boot、VBS、HVCIとWindows Driver Policyは別の判定である。この構成で将来ロードに成功しても、Secure Boot・HVCIを有効にした環境での互換性の証拠にはならない。[MicrosoftのDriver Policyの状態確認](https://support.microsoft.com/en-us/windows/hardware/drivers/the-windows-driver-policy)・[VBSとメモリ整合性の値](https://learn.microsoft.com/en-us/windows/security/hardware-security/enable-virtualization-based-protection-of-code-integrity)

## UACと再照会の経過

通常ユーザーでの照会は `0x80070005`（アクセス拒否）。初回と次の再提示は、昇格プロセスの起動時にWindowsが `1223`（キャンセル）を返し、ポリシー一覧を取得できなかった。所有者がゲストで「はい」を押した回に、ペアリング済みの同じVM-Aから実行・解析成功の報告を受けた。上表にはこの成功回の値を採用した。

## 次の判断

事前調査で依頼された読み取り項目はすべて取得できた。追加のUACや結果の転記は不要。VM-Aで導入試験に進む場合は、R0・R1を保持したままの復旧方法、Coreのシステム全体への変更と初回ロードの可能性を先に示し、別途承認を得る。今回はインストール、ロード、ディスク公開、セキュリティ設定変更、復元点の作成・復元を実施していない。
