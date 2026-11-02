# ADR-004: I/O捕捉位置を実stackの証拠なしに採択しない

状態: **調査済み・lab証拠待ち**。D01 #3は未完了。更新: 2026-11-02 JST。

## Context

原本HDDの変更を一つでも見落とすと、clean read cacheでも古いNTFS metadataを返し、原本を間接的に破損させ得る。「下位filter」という名前だけでは捕捉位置は決まらない。

## Options

| 候補 | 利点 | 未解決の境界 |
|---|---|---|
| filesystem / volume filter | file/volumeの通常I/Oに近い | physical-disk handle、別volume、adapter-directed commandを網羅できるとは限らない |
| disk-class upper filter | 通常read/write IRPのbyte rangeを扱いやすい | partmgr・他filter・raw/control・bypassとの実際の位置関係が必要 |
| disk-class lower filter | classからportへ渡る要求を見る | legacy SRBとextended SRBの両形式、CDB分割・特殊commandを理解する必要がある |
| miniport/専用virtual disk | より下位または自分のdevice境界を所有 | 汎用既存HDDのdrop-in cacheとは異なる製品scope・保守負債 |

## Decision

**現段階では製品の捕捉位置を採択しない。** device-specific disk-class upper候補とlower/SRB候補を、同じcoverage matrixで比較する。volume filterだけでphysical raw mutationを網羅したと認定しない。C/WDK/Rustの選択はD09、所有権はD04で別に決める。

class-wide UpperFilters/LowerFiltersは対象外OS diskにも装着され得る。cache default-offはboot safetyの証拠ではない。per-device登録も、実際の順序が想定どおりになることを確認する。同level内の順序に依存しない。

## Evidence

Microsoftの[storage filter概説](https://learn.microsoft.com/en-us/windows-hardware/drivers/storage/storage-filter-drivers)はclassの上下を区別し、[dispatch契約](https://learn.microsoft.com/en-us/windows-hardware/drivers/storage/storage-filter-driver-s-dispatch-routines)はclass/port間のdual SRB対応を要求する。[filter ordering](https://learn.microsoft.com/en-us/windows-hardware/drivers/develop/device-filter-driver-ordering)は同levelの順序を保証しない。資料確認は2026-11-02。これらは対象機での捕捉実証ではない。

[coverage matrix](../contracts/io-coverage.md)と[lab handoff](../../experiments/stack-trace/README.md)に、事実・提案・未観測を分離した。今回、driverを作成・install/loadせず、実stack traceも取得していない。

## Consequences / Rejected alternatives

- D04/D05/D08/D10/G1などD01受入を前提にする製品作業は引き続きblocked。
- 未知potential mutationをcache activeのまま素通しする案は不採択。単なる後追いinvalidateは競合readを救えない。
- 「最も下に付ければ全て見える」「管理者しかraw writeしないから考えなくてよい」「default-offならglobal registrationも無害」は証拠にならない。
- 不明経路がある構成はactivation拒否。未知操作を拒否する場合の互換性・原本errorの扱いも公開する。

## Revisit trigger / Reviewer

immutable probe source/build、承認された使い捨てtarget、全caseの相関trace、欠落counter、復旧記録が揃ったら独立Astraレビューで採択を判断する。Windowsストレージ専門家による出荷前レビューをAIで代替しない。関連: [D01 #3](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/3)、INV-02/07/08/12。
