# P04　T1限定の宣言的フィルター登録は現行手順に使えるか

2026年11月20日 JST。VM-Aのデータ用ディスクをT1、同じゲストの保護対象であるOSディスクをD01とする。[前回の排他・配備調査](P04-T1-registration-exclusion-and-package-scope.md)で残った代替案を、既存の匿名記録、固定ソース、Microsoftの一次資料で再審査した。

**判定はNO-GO。** 宣言的な `AddFilter` は、登録ツールが既存の `UpperFilters` リスト全体を読み、書き換える処理を省ける。一方、Microsoftの説明では、新規または更新した基本・拡張ドライバーパッケージを対象デバイスにインストールしてフィルターを追加すると、そのインストール中にデバイスが再始動し、新しい一覧が使われる。仮にT1だけに一致するIDが見つかっても、**登録の最終読み戻しVまではロード・PnP再始動を分離する**という現行条件を、このインストール経路で保証できない。ここでいう再始動は対象デバイスの再始動である。VM全体の再起動が必ず必要という意味ではない。[Microsoftのフィルター順序とデバイス再始動][ordering]／[AddFilterの効果][addfilter]／[現行仕様のREG-2～3](P04-T1-cancel-spec.md)

## 現行手順に代入できない理由

| 判定項目 | 一次資料と既存証跡から分かったこと |
|---|---|
| **競合の解消範囲** | `DDInstall.Filters` はフィルターをメタデータで登録するため、この方式へ置き換えれば、P04自身が他者の追記を旧リストで上書きする箇所はなくなる。一方、他のinstallerによる変更、PnP再列挙、サービス設定や初回ロードまでを区間全体で排除する契約は得られない。PnPの待機成功後も新しいイベントは発生し得る。[DDInstall.Filters][filters]／[PnPの待機API][wait] |
| **配置と適用** | Driver Storeへの事前配置だけでは、T1にフィルターを登録したことにならない。T1への適用では前述の再始動境界を越え得る。P04の既存ドライバーは初回 `DriverEntry` でselectorを読み、`AddDevice` で個体を照合する。先行ロードや対象外へのINF適用を、後からselectorを設定して回復できるとは扱えない。[Driver Store][store]／[PnPによるロード][load] |
| **読み戻しの意味** | 現行F/Vが照合するのはT1の旧 `SPDRP_UPPERFILTERS` に1件を加えた値である。宣言的方式はそのレジストリ値に依存しない。Microsoftが宣言的な一覧の照会先として示すのは `DEVPKEY_Device_CompoundUpperFilters` 等。別設計では「登録情報が一致」「実スタックの所定位置に装着」「実際のI/Oを観測」をそれぞれ判定する必要がある。[DDInstall.Filters][filters]／[一覧の照会][ordering] |
| **装着位置** | `FilterPosition=Upper` は `disk` に対する上側を指定する。基本INFがレベルを定義していれば既定レベルが使われ、同じレベル内の順序には依存できない。明示したレベルが基本INFになければ、そのフィルターはスタックに加わらない。レベルなしを従来の末尾追加と論理的に等価とする記述も、他の上位フィルターとの隣接を保証しない。保存済みの `partmgr → disk → stornvme` から、候補位置 `partmgr → P04 → disk` は確定できない。[FilterLevelとFilterPosition][addfilter]／[順序と既定レベル][ordering]／[既存スタックの記録](P04-VM-A-T1-runbook.md) |

## T1だけをINFで選べるか

**現時点の結果は「未確認」。** AFTER_R1では、完全なPnPインスタンスID、ストレージ固有ID、シリアル、親位置とLUNによりT1とD01をそれぞれ再同定できた。署名済み拡張INFのModels照合で使うのは、列挙元が報告する **hardware ID／compatible ID** である。インスタンスIDやVMDKの個体識別に成功しても、これら2種類のIDに有効な差があるとはいえない。MicrosoftのNVMe識別子の例にもベンダー・製品・リビジョンによる形式がある。VM-Aの実値をその例から推定していない。[拡張INFの選択][extension]／[インスタンスID][instance]／[ストレージの識別子][storageids]

- **肯定条件だけのModels案。** VM-AのOS・アーキテクチャで有効なすべてのModels行を対象に、T1には1件以上の一致、D01と対象範囲内の他ディスクには一致0件を求める。両ディスクの全ID集合が互いに完全に異なる必要はない。採用した行のうち1つでもD01に一致すれば不採用。接続済み2台での一時点の判定に加え、同じパッケージがStoreに残る間の再列挙・ディスク差し替えなど、保証する範囲を明示する。[Modelsの照合項目][models]／[拡張INFの適用条件][extension]
- **除外や識別子を取り違えない。** `ExcludeID` が扱うのも、該当するDDInstallに直接記載したhardware ID／compatible IDである。両台の該当ID集合が同じなら、それを使ってD01だけを除外できない。`ExtensionId` は同じ拡張の版を選ぶ識別子、CHIDはコンピューター単位の識別子で、どちらも同一VM内の2台を区別する根拠にならない。[DDInstallのExcludeID][exclude]／[ExtensionId][extension]／[CHIDの単位][chid]
- **実測と合成試験。** 引き継ぎ手順第3節と共有されたAFTER_R1要約には、実機で採用できるModels条件とそのT1・D01別の一致件数は記録されていない。試験表と実装fixtureでは、保護ディスクと対象ディスクに同一のhardware／compatible IDを意図的に与えている。これは誤選択の試験条件である。実機の両台が同一IDを報告したという測定結果には使っていない。[手順書](P04-VM-A-T1-runbook.md)／[試験表](P04-T1-cancel-test-matrix.md)

## 次の判断に必要な最小の材料

| 判断 | 生の識別子を含めず提示できる証拠 | 現在 |
|---|---|---|
| 対象だけに一致するか | 同じ採取時点にT1とD01を独立に同定した記録、両者のhardware／compatible ID照会の完全性、有効なINF条件を全件評価した結果（T1一致あり、D01一致なし、ほかの対象外への一致件数と未取得件数）、その判定の有効範囲 | INF照合結果は未確認。未取得を0件には数えない |
| 候補位置を説明できるか | 適用する基本INFの版・レベル定義、既存フィルターの登録元と順序、P04が `disk` 直上かつ `partmgr` の下に入る根拠 | 現スタックの記録はあるが、新配置の根拠は未確認 |
| 現行条件を維持できるか | 登録からVまで再始動・ロードを許さない条件と、Windowsによる実際の適用時点が両立する根拠。境界を変える場合は、対象・登録情報・起動・装着・回復それぞれの合否を定めた別契約 | 新規フィルターの通常の適用ではデバイス再始動を伴う。現行契約のまま実登録に進めない |

固定ソースZIP（ツリー `f1fb5c4f9f459edd2aeebfc490bef246f4a2f7d7`）にはINFが1件、CAT・SYSは各0件だった。INFは `DefaultInstall.NTamd64` を持つprimitive形式で、Models節も `AddFilter` もない。ソース中のパッケージ審査と実適用入口は閉じている。拡張INFへの移行は当該INFの署名だけで済む変更ではなく、配布物・契約の別審査が必要になる。新しい署名済み一式が得られても、今回特定した再始動・装着位置の条件は残る。[レビュー済み実装](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/tree/153fac3473186241ccf3f354e59322e63b1f183f)／[primitive INFの要件][primitive]／[カタログ署名の範囲][catalog]

[ordering]: https://learn.microsoft.com/en-us/windows-hardware/drivers/develop/device-filter-driver-ordering
[addfilter]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/inf-addfilter-directive
[filters]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/inf-ddinstall-filters-section
[wait]: https://learn.microsoft.com/en-us/windows/win32/api/cfgmgr32/nf-cfgmgr32-cm_waitnopendinginstallevents
[store]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/driver-store
[load]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/specifying-driver-load-order
[extension]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/using-an-extension-inf-file
[instance]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/device-instance-ids
[storageids]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/identifiers-for-scsi-devices
[models]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/inf-models-section
[exclude]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/inf-ddinstall-section
[chid]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/computer-hardware-ids
[primitive]: https://learn.microsoft.com/en-us/windows-hardware/drivers/develop/creating-a-primitive-driver
[catalog]: https://learn.microsoft.com/en-us/windows-hardware/drivers/install/catalog-files

## 独立レビュー結果

**調査本文を受入。重大な訂正事項なし。実登録はNO-GO。** 別コンテキストのAstraが本文のSHA-256、レビュー済み仕様、手順書と合成試験の記録、Microsoftの一次資料を直接照合した。インストール時のデバイス再始動、従来の値と宣言的一覧の違い、装着位置、実測と合成のID照合の区別、および現行パッケージの適用拒否を確認した。

対象は文献と固定成果物に基づく判断である。VM-Aでの実機IDの一致件数、P04の装着位置、デバイス再始動の実挙動は未確認。今回、実装変更、署名、機器・レジストリ照会、実機での登録・ロード・観測、VM設定変更、GitHubへの投稿は行っていない。

レビュー対象本文のSHA-256は `d91562a55aa05031d19fca3d57c141fe047b219f64fdf322d5ba2125612e7b29`。この節は受入後に追記した。
