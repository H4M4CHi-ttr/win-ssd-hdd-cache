# 全Issue監査と是正 — 2026-11-02

対象: baseline 67c09a5526bd5ed812af6acab3223e2c9a608379 の49作業Issue (#2〜#50) と親 #1。rootと独立SOL/Astraが、本文・契約・計画・全依存・ラベル・受入条件を照合した。本文とsnapshotの単純な不一致や明示DAG cycleはなかった。一方、次の実装停止/契約欠落を是正した。作業記録は[AUD01 #51](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/51)。

| 問題 | 是正 | 安全条件 |
|---|---|---|
| owner-required課題が承認後もreadyになれない | backlog v2: owner_approval/evidence、done/accepted、cancelled/not-plannedを明示 | 既存承認はすべてpendingのまま。URLを承認そのものと扱わない |
| G1後のkernel作業がB03予算未完でも開始できた | G01へB03依存を追加 | 初期予算はhost証拠＋保守的仮定。実機結果を先取りしない |
| kernel custom control endpointの実装担当がない | [W10 #53](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/53) を新設、C04の依存へ | 認可/ABI/target再検証はdriver側でも強制 |
| G4の事前条件にG4担当Q04自身が含まれる | Q01〜Q03/D08/GOV01/C08完了後にQ04で判定 | 人間の専門review/owner公開承認は維持 |
| D07がportable oracleまでWindows所有権実証に依存 | portable D07と[Windows mapping D10 #52](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/52)を分離 | F08/G01はD10も必須。D10は実DMA実行証拠ではない |
| F09のoriginal Rust経路にも一律GOV01と直接D09依存 | 一律GOV01 gateと冗長な直接D09依存を除去し、original Rustのhost経路へ限定 | D09はF09→F08→D04→D09で依然必要。OCF vendoring/kernel経路は再設計・承認後のみ |
| D03が自身に依存する完成harnessを要求するように読める | 独立feasibility spikeのinterface/最小反例を明記 | spike自己modelを独立oracleと称さない |
| D01/D09 probeとW01製品driverの証拠境界が曖昧 | pre-G1 experimentsの限定scopeを明記 | install/VM/実機実行の別承認とG1を維持 |
| D02でcache-read lease/ID枯渇/短いcache I/Oの具体受入が不足 | 追加の規範と反例をD02へ要求 | epoch更新≠I/O停止、partial lineをvalidにしない |
| W04のraw SSD試験の準備来歴が暗黙 | C07または同等の承認済みmanual recordを要求 | identity/range/排他/直前検証を省略しない |
| Luna表示/report課題のAPI/出力/欠落nullが未凍結 | [host契約](contracts/host-formats-v1.md)に固定 | raw error/secret/path非反射、公開型の不正構築も拒否 |
| F01→F02→F10なのに二OS実行をF01前提とも読める | portable artifact受入とF10二OSgateを分離 | Windows未実行を合格にしない |
| T20担当がC07、GUIをG5別RFCとも記載 | C08へ訂正、U01〜U03は任意後段と明記 | CLI/lab安全条件は不変 |
| B01実装がC01のprivate strict JSON adapterを使用 | B01のC01依存をbacklog/plan/Issueへ明記 | レビュー済みadapterの共用を隠れた前提にしない |
| F02のschema/path/link/資源上限とerror細部が未固定 | [checker契約](contracts/planning-checker-v1.md)を追加し実装前に独立review | owner承認・受入判定をcheckerが代行しない |

## あえて維持した停止条件

D08をgovernance/配布実現性のG1前gateとして維持した。現時点でownerに代わってライセンス・署名・費用・lab対象・公開を決めていない。D01/D09の実Windows証拠、D04のkernel所有権、G02/G3、実機/HVCI/Verifier/HLK、人間の専門reviewをhostテストで代替しない。

## 実装とcloseの意味

root/ProがLuna分類の課題を実装しても難度ラベルは保持する。作者と別のreviewが必要。現在は10件のhost成果物が独立reviewとLinux/Windows Server試験で受入済み。[実装・PR・実行記録](evidence/2026-11-02-host-foundation.md)を参照。PR/merge、Issue本文・snapshotの同期はそれぞれ証拠を残す。main未統合の間はIssueをcloseせず、acceptedはPR上のartifact受入と明記する。Windows 11 driver/実機安全性や「破損ゼロ」を主張しない。

この監査で追加した作業はAUD01/D10/W10で、総作業数は52、親を含め53 Issues（PRは別）になった。最終検査結果・commit/PR・実装完了数は各Issueおよび[バックログ](backlog.md)を正本とする。
