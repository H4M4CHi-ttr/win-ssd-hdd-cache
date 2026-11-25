# P04　T1限定登録・中止通知の試験条件と受入証跡

2026年11月19日 JST。基準main `eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c`。[仕様書](P04-T1-cancel-spec.md)のREG-1～4、ABORT-1～4とビルド判定に対応する。**下記は将来の試験。今回の結果はすべて NOT RUN。** 手順書のレビュー受入や既存CIの過去の合格件数を、新機能の試験成績に加算しない。

## 独立した期待値と試験装置

- 登録側には合成inventory、権限・排他・列挙世代、パッケージ／サービス／selector／devnode・クラスの状態を持つfakeを使う。保護ディスクA、対象T、第三のインスタンスU、クラスを別の固定オブジェクトにする。AとTには意図的に同一のhardware/compatible IDを与え、完全なinstance IDは異なる。実装の選択結果から期待するTを作らない。
- fakeは **試みた変更と、その結果ストアに残った変更**を別々に記録する。発行主体（登録ツール／競合者）、対象の実体、適用順を分ける。対象は返されたデバイス情報要素の実体で識別する。A・U・クラスへの登録／再構成要求は、一度も呼ばれていないことを全Rケースで照合する。書いて取り消して最終状態が一致しても失敗。PとSのゲスト共通の副作用は別欄に計上する。
- collectorは既存の実収集ループに、fake transport、仮想単調時計、試行ごとの通知源を接続する。別の監視担当が独立の時計、命令列、同期点、保存した実バイト列を観測する。各ケースの期待列・バイト・件数・状態を先に固定し、被試験builderの出力から正解を作らない。R13の各段階やH04の各命令は別の実行IDで記録する。表の行数は実行件数を表さない。
- 表中P/S/K/F/Vは仕様の配置・サービス・selector・T1リスト・読戻し。`B1, D2, S3, Q4` はBEGIN・DRAIN・STOP・STATUSと単調増加するcall ID。回収用のDRAINは通常の収集中のDRAINと区別する。表の「0」は呼出し記録または独立した台帳で確定できるときだけ使う。

## 登録　対象選択と変更範囲

| ID | 入力・故障注入 | 必須の期待結果 |
|---|---|---|
| R01 | AとTで同型、同じ親。専有できる合成環境。TのみRAW。Tの既存列は`[KeepA, keepB]` | dry-runでは変更0。fake適用ではP→S→K→Fを各1回、VでTだけ`[KeepA, keepB, Probe]`。A・U・クラス・lower・機能サービスは期待スナップショットと同一。結果は登録読戻し一致まで |
| R02 | 入力がAを指す／APIがT用照合後にAを返す／TのラベルにAの実体／AをTと同じ容量・Tの旧番号にして指定（別fixture） | 保護ID・OS用途または同定不能で拒否。Aの変更要求0。最初の判定で分かる場合はPを含め全変更0。後段に注入した場合は確認済みprefixまで、Fへの呼出し0 |
| R03 | Tが0件、2件、disconnected、同じシリアルの2台、ID群が別々の行に分かれる、ホスト連鎖が旧T1。逆に番号だけが変わり全独立証拠が揃う | 0件・重複・混成・誤ファイルでは推測せず拒否。番号だけの正常変化ではTを選択し、Aに作用しない。ディスク自身の位置パスが空でも、親位置・LUN・他の独立IDを満たす正例を区別 |
| R04 | 空／途中NUL／末尾NULなし／奇数バイト／型違い／511コード単位＋終端と、その1単位超過／大文字小文字・接頭辞だけ一致 | selectorの型・長さの単体試験では511単位＋終端を許容し、超過を拒否。実際のWindows IDの最大長や登録成功を意味しない。ID照合は規約上の大文字小文字差を許容。空IDでSetupDiOpenDeviceInfoWを呼ばない。切詰め、部分一致、ルート選択、誤った値／型のselector書込みは0 |
| R05 | TへEFI/OS/pagefile/dump/専用dump/休止／別特殊用途をそれぞれ割り当てる。用途照会失敗、build・VM識別・R1・VMDK連鎖不明も個別に注入 | すべてPより前に拒否。A内のpagefile配置変化は差分として保存し、Tに不使用である証拠が揃う場合との違いを確認。不明をFalseにしない |
| R06 | TのUpperFiltersが不在、正しい空列、複数既存名、型違い・長さ異常・終端後に別文字・未取得の各状態 | 最初の2ケースは元状態を区別して記録し、許可された場合だけ1名追加。既存名は大文字小文字・順序を保持。不正／取得不能は変更せず拒否。照会エラーを「不在」とみなす実装は失敗 |
| R07 | 予定名が既存列にある、大小文字だけ違う重複、同名別バイナリ、既ロード、予定パッケージでD01にも適合するinstall、付随INF／class操作／サービス即時起動 | 未所有の既存状態は採用しない。計画外効果はPの前に拒否。意図的に広い登録を許すテスト用変異（mutation）がR02またはR07で必ず落ちる |
| R08 | 独自mutexだけ、別installer・PnP除外が未立証。敵対的fixtureでは最終読取り後に第三者が`Other`を追加 | 本物の効果先を選んだモードはPの前に拒否。比較だけで進む誤実装なら、読戻しが予定列でも`Other`消失を独立ストアが検出して不合格。偽の排他証明は本番型／引数に渡せない |
| R09 | K確認後、F直前にTの世代変更、取り外し、保護ディスクとの入れ替わり、排他失効、予期しない初回ロードを個別注入 | prefixは最大P/S/K。F以降は0。D01を開き直して続行しない。ロード後にselectorだけ書き換える処理、広い再列挙・PnP再起動での救済は0 |
| R10 | Vの読戻しでTの列順・型・追加数・対象が違う、D01／クラスの想定外差分、AまたはTの実stackが不明 | 一致に昇格しない。対象外の変更が外部由来かを推測せず記録し、復元判断へ。fakeでVが一致しても、Windows装着・CDO存在・ロード・安全性の各フラグはfalseまたは未測定 |
| R11 | 成功済み試行を同じ記録で再提出／完了記録なしで値だけ同じ／P04名の重複・所有者不明／R1から戻して別世代で同じ番号を再利用 | 前者だけ読み取りで既確認を返せる。ほかは新世代・残存・不明として適用拒否。全ケースで再変更0。前のR1やIDを新規試行へ持ち越さない |
| R12 | raw instance ID・シリアル・ユーザーパスを識別可能な合成マーカーで混入、Windowsエラーにそれらを含める | 私的な照合記録と公開可能なdiagnosticを分離。後者やCI出力には生値・ハンドル・絶対パスが出ない。対象が未取得なら未取得の理由を保持 |

### 登録の途中失敗は4段階すべてで再現する

R13はP、S、K、Fの各段階に下記の列を掛け合わせる。独立した初期状態から始め、既に確認済みのprefix、当該段階の呼出し回数と実効果、次段の呼出し0を比較する。Vの読戻し・完了台帳保存の失敗はR14で扱う。一つの例で全段階を通った扱いにはしない。

| ケース | 注入点 | 必須の判定 |
|---|---|---|
| R13a | 当該段階の変更直前に意図の保存が失敗。生きている呼出し側が未呼出しを確認 | 当該APIは0回。前段の効果は保持。Pなら適用前拒否、S/K/Fなら確認済みprefixを伴う停止 |
| R13b | 意図は保存済み。APIに入る直前のプロセス消失と、実際に効果が生じた直後・戻り値保存前の消失の2系列 | 再起動時の台帳は同じでもfakeの最終実体は異なる。双方を成功にも未変更にも決め打ちしない。独立した再列挙と読戻し、結果不明ならhold。後段・自動再送0 |
| R13c | APIはエラーを返す。効果が無い場合と、効果を生じてから同じエラーを返す場合 | 戻り値だけでは両者を区別しない。再照合で確定できた効果だけを記録。確認不能なら効果不明。古い列への書戻し・自動削除・再適用0 |
| R13d | API成功直後の読戻しが拒否／型違い／対象世代違い／別内容／確認記録の保存失敗 | 成功を採用しない。次段0、呼出済み段階を保持。F後でも「登録読戻し一致」は返さない |
| R14 | Vの途中または全読戻し後・完了記録確定前のクラッシュ。再起動時に完了記録も失われた版を含む | 私的な根拠を再構成できる範囲だけ報告。欠損時に「初回」「続きから」に切り替えない。ドライバーロードやR1復元をテスト対象が自動実行することは0 |

## 中止通知　所有者、競合、回収

| ID | 入力・競合 | 必須の期待結果 |
|---|---|---|
| C01 | 旧help・audit・project・collectの固定入力、feature無効、非Windows、N=0/1/30000/30001、先頭0、重複・未知引数、予約環境値だけの起動 | 既存の出力・終了値・default-offを保存。新モードの正規形だけが制御経路の検査へ進む。無効環境ではRNG、CDO、ライブFFIの呼出し0 |
| C02 | fake／Windows合成processで、起動引数と通知用handleの欠落、ゼロ、幅超過、不正桁、擬似値、古いhandle、非Event型、過剰権限、照会不明、stdoutが非pipe | CDOを開く前に拒否。無効なオブジェクトをwait／所有しない。正規の子ではEvent型・`SYNCHRONIZE`のみを確認。ランチャーの明示リスト外のsentinel・親のsignal権・CDOは子に継承されない |
| C03 | 試行AとBを分け、AだけにSetEventを2回。送信者以外からの更新権なし、送信者が通知前に消失する版 | Aのラッチ1回、Bの通知0。read-only子からのSetEventは失敗。送信者消失を自動的な通知／ACKとして記録しない。正規ランチャーの経路を証明できない場合、実機利用を許可しない |
| C04 | open前とopen後・B1前の2か所で通知。通知経路故障も同じ境界に注入 | 通知はBEGIN/STOPとも0、archiveなし、新モードの終了値5。open前はCDO open 0、open後は通常close 1回。OS故障はexit 3、入力不正はexit 2。通信成立を偽装しない |
| C05 | B1がoutstandingの間に通知。独立barrierを解放し、B1が有効ACK／失敗／binding不正の3通り | 有効ACKなら `B1,S2`。不明ならB1のみ、再BEGIN・STOP・成功archiveは0件。開始効果不明を残す。構造は正しいが履歴異常のBEGINはbuilderのuncertain規則に従い、正常開始として継続しない |
| C06 | 最後の未通知poll直後に通知され、その境界のD2が先に投入される。D2の正常返答でownerが受理。通知重複と期限到達を重ねる | この競合の許容列は `B1,D2,S3`。受理後の新規通常DRAINは0件、outstanding最大1。D2で返った実記録は1回だけ保持。通知がpoll前ならD2を出さず `B1,S2` |
| C07 | D2が先にpopした後に応答喪失。あるいは応答のsession/target/callが不一致。中止通知も同時に入れる | 受理した正当なフレーム以外を保存しない。`B1,D2,S3`、fresh cleanup STOPは最大1回、D2再送やD4は0。gap／TRANSPORT_ERRORまたはBAD_RESPONSEを保持。後で有効終端が届いても収集完全にはしない |
| C08 | 正常なSTOPが失敗、次のcleanup STOPが成功または失敗／既に不確かな交換のcleanup STOPにも失敗／正常STOP後の回収DRAINに失敗 | 通常STOP失敗なら新IDで最大1件だけcleanup STOP。すでにcleanupなら追加なし。例 `B1,D2,S3,D4失敗,S5` は許容しD6不可。通知の回数で予算が増えない |
| C09 | D2の検証済み応答ごとにproducer fault mask非0、既知drop、既知skip、必要なknown bit消失、Running→Stoppingを個別注入。整合しない履歴は別ケース | 戻った事実は保持し、次の通常DRAINを発行しない。STOPからは既存の信頼できる回収だけ可。履歴破綻・binding不正はuncertain側が優先。欠落値を0に変換しない。元の `normal_draining_allowed` だけで継続する旧動作なら不合格 |
| C10 | 通知＋producer fault＋通常期限が同時。通知確認API自体がBEGIN後に失敗。STOP中または回収DRAIN中に通知が重なる | 同時の正当なproducer faultと外部中止なら主因はproducer、両方を保持。応答不明が同時なら主因は応答不明。同じ返答の輸送／検証エラーを隠さない。中止要因を重ねても通常STOPは重複しない。通知失敗をtransportの実測lossと混同しない |
| C11 | 既に受けた要求の完了がSTOP後に来る。active=0/queued>0、active>0/queued=0、Sealedかつ双方0、frame/record上限、5秒直前／到達を用意 | 終端ACKの前やqueued>0で終了を捏造しない。STOP後の回収だけが有界で続く。上限／期限到達を記録し未完了contextを強制退役しない。記録予算0の経路で `DRAIN(max=0)` は0件 |
| C12 | 中止しても有効終端・0件のarchive、または正常なsubmit/completion 1組が残るケース。stdout短書込み・途中失敗・flush失敗・退避失敗も個別注入 | native v1バイトとfooterの既存マスクを維持。0件はEMPTY。予定窓完遂・terminal・reconciled・プロセス終了・保存は別の値。valid archive出力はexit 4、出力失敗は3。パイプ失敗時に成功footerを追記しない。最終write成功・flush失敗で見掛け上完全なfooterがあっても、保存成功は未確定。終端後に初めて届いた通知で新規open・STOP・再出力しない。補助記録やASCII診断をbinary stdoutに混ぜない |

## H　同期I/Oが戻らない試験

同じループを実行する隔離fake子プロセスと、その外で動くsupervisorを用いる。両者が同じ停止したイベントループや仮想時計に依存しないようにする。テスト用transportは特定の呼出しの入口で `Entered(段階, call)` を外に通知し、明示的な `Release` まで戻らない。supervisorはEntered確認→通知発行→**別の**仮想時計を進める→外部観測の固定期待値を照合する。テストcleanup時にだけReleaseし、workerの正常な後始末を確認する。実VMや本物のCDOは開かない。固定期間内の不帰還を示す試験である。

| ID | 保留する箇所 | 外側の期限時に必要な観測／解放後の期待 |
|---|---|---|
| H01 | CDO open | ownerの次のpollは未到達、BEGIN/STOP=0。成功で解放したら受理済み通知に従ってBEGINせずclose。開けなければopenエラー。未発行と観測不能を区別 |
| H02 | 同期B1 | outstandingはB1だけ。返答・STOP・成功footerなし。テストが正常ACKを解放したときだけ `S2`、失敗ならC05。ハンドルのforce-close、2本目のCDOは0 |
| H03 | 同期D2（fakeはpop済みにもできる） | `B1,D2` のまま、通知発行済みでもowner受理・STOPは未確認。15,000 msで「停止未確認」。解放したD2が有効なら保存して同じownerがS3、不明ならcleanup S3最大1。D2の再投入なし |
| H04 | 同期STOP、または正常STOP後の回収DRAIN／STATUS | 未返答のSTOPにACKを付けない。既にSTOP ACKがあればその事実と未受信の終端を分離する。同時STOP、別ハンドルからのSTATUS、5,000 ms経過だけの終了は0。解放後はC08/C11 |
| H05 | archiveのwrite／flush | workerが終端まで受けていても、外で確定できるのは受信済みbytes・既知の通知だけ。プロセス完了・EOF・保存済みを捏造しない。解放後は実際のwrite／flush結果を記録。R1内だけの保存を退避成功としない |
| H06 | supervisorのみが起動から14,999→15,000 msへ移る。別ケースでworkerが本当に完了・受信側がEOFとハッシュを確認。さらにhost監視経路自体を喪失させる | 期限前の未知をタイムアウトにしない。期限時の未終了はstickyな未確認記録。後着の完了を別時刻で追記し、当時の判定を書き換えない。監視不能を完了にしない。ホストからの強制操作・R1復元呼出しはいずれも0件 |

実wall clockのsleep、返り続けるfakeの `active>0`、仮想時計だけを進める試験はH03の代用にならない。監視期限到達が期待結果のケースでも、テスト装置自身の上限超過／worker解放失敗は**テスト失敗**とする。停止を確認したという出力でハング試験を合格にしない。[現行の期限試験][oldtest]

## B　ビルドの合否と証拠の保管

| ID | 将来行う検証 | 合格条件／取り違え防止 |
|---|---|---|
| B01 | pure Rustの登録判定／段階制御、collectorの同じ収集ループ、独立oracleをLinuxとWindowsでテスト。Windows API境界は合成関数テーブルで差し替える | テスト専用に分岐した別アルゴリズムを検証しない。全R/C/Hとパラメータ展開のfixture ID、対象・コマンド・終了値・名前付き件数・期待と実測を保存。既存default-off CLI、archive/audit/project、P04 recorderの関連回帰を維持 |
| B02 | Windows AMD64で、将来の登録境界・通知し得るランチャー・通知を受けるcollectorを、実際のヘッダー／Win32 bindings・SDKライブラリで**リンク**。既存の `cargo check/build -p cache-probe-collector --features experimental-live --locked` 相当のcheckとbuildを区別 | リンク対象のソース／feature／リンクマップ・import・ハッシュを記録。fake-only、構文検査、`cargo check` だけは不合格。本物の効果を持つバイナリは**起動しない**。コンパイル条件で実装が落ちたり、リンク時に必要なFFIが除かれたりしていないことをソースとオブジェクト／リンク結果の双方で確認する。実環境の排他を証明する機構のない構成ではlive apply入口は閉じたまま |
| B03 | Windows hostedのユーザーモードIPC試験は、合成専用workerに限って実イベント・継承allowlist・binary pipe・待機戻り値を検査。productionと同じ限定FFIを呼び、デバイスtransport・登録APIはfakeに固定 | 読取専用eventへの変更拒否、別試行への通知0、正当なSetEvent・受理、権限過大／誤型拒否、ハンドルの再継承0・通常close各1回を実OS境界で検査する。ダミーworkerのプロセス起動は可。本番CDO open、実デバイスの列挙／設定、Driver Store／サービス／レジストリ変更は0。実IPC試験と実storage試験を別欄に記録 |
| B04 | パッケージ案がある場合は全アーキテクチャ該当のINF展開・依存、許可先、カタログ対象を静的検査。CIの権限・イベントフィルターとアップロード対象も確認 | 新規パスをpush/PR双方の対象に含め、既存のbuild-only禁止事項を維持。INF解析だけで排他・順位・将来の署名受理をPASSにしない。新パッケージ未確定ならpackage検査は未実施／配備保留と明記 |
| B05 | 変更箇所のfmt／Clippy、パッケージのdebug／release、workspaceの既存ゲート、既存P04 proofとの互換性を確認。WDK境界を変更する必要が出た場合は別にその実WDKリンク証拠を要求 | 安定したcase IDの必須集合と実行一覧を突合する。0件、ignored/skip、期待名の欠落、重複での水増し、Windows側のリンク欠落は不合格。過去の45/46件やCIバッジを新しい試験の証拠に転用しない |
| B06 | 証跡の最終照合 | Gitの実checkout・tree、対象ソース、テスト期待値、ツール／SDK版、feature/target、コマンド・exit、個別試験一覧・バイナリのハッシュを固定。入力と出力のハッシュ／結果を別担当が照合。公開する想定の証跡は合成かつテキスト／JSONだけ。driverInstalledOrLoaded、registeredOrAttached、observedLiveRequests、realDeviceAccess、runtimeQualified、windowsClientQualifiedはすべてfalseのまま |

将来のビルド手順は、既存の `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --locked` とP04のrelease対象テストを利用する。Windows AMD64では、collectorについて `cargo check -p cache-probe-collector --features experimental-live --locked` と `cargo build -p cache-probe-collector --features experimental-live --locked` の両方を要求する。新しい登録ターゲットと合成IPC workerの名前・featureが確定したら、対応するリンクと合成試験の**実際のコマンド**を検査契約に追加する。上記の既存コマンドだけでは新機能の合格にならない。

**将来の判定欄。** 登録モデルはR01～R14（R13の全展開を含む）とB01/B05/B06。登録のWindows境界はこれにB02の登録部分とB04の具体的なパッケージ検査を加える。パッケージ未確定ならモデルに限って判定し、登録一式は保留。中止通知はC01～C12、H01～H06、B01/B02/B03/B05/B06の該当項目を要求する。1件でも必須の未実施、期待不一致、workerのハング後始末失敗があれば、その欄をPASSにしない。実Windowsへの書込み排他、T1以外への実副作用がないこと、装着位置・実効ACL・署名、同期I/Oの実停止時刻、ゲストハングからの復元は独立した実機ゲートに残す。[既存P04ワークフロー][workflow]・[本物のWin32リンクと実行禁止][link]・[名前付き非ゼロ検査][nonzero]

[oldtest]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/crates/cache-probe-collector/src/capture.rs#L329-L355
[workflow]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/.github/workflows/probe-build-proof.yml
[link]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/experiments/stack-trace/probe/tools/build.ps1#L89-L101
[nonzero]: https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/blob/eb2ee0573ed5cc82b84bb7ad3477b4cb764b5d7c/experiments/stack-trace/probe/tools/result-proof.mjs#L6-L38
