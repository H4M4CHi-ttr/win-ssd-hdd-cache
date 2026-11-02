# ADR-013: clean-cacheの可視性と物理I/O所有権を分離する

状態: D02独立review待ち。2026-11-02。関連: [D02 #4](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/4)、INV-01〜09。

## Context / Options

単なるinvalidate-before-writeでは、古いreadからのfillがwrite後に公開され得る。epochだけを変更しても、旧I/Oが同じ物理slotを後から上書きする危険は消えない。

| 案 | 判断 |
|---|---|
| validity flagだけ、cancel/epochで即解放 | 不採択。stale-fill、two-writer、late-I/O反例がある |
| 全I/Oを一つのlockで物理完了まで直列化 | 不採択。非同期I/O待ちをlock内に置けず、hangが全原本I/Oを止める |
| checked token + writer count + physical lease | 採用候補。短いmetadata transitionと、完了まで保持する別ownershipを分離 |

## Decision

[trace v1契約](../contracts/trace-v1.md)をportable abstractionとする。原本mutationの前に失効し、fillはsource readを開始する前のtokenを保持する。各lineのwriter **数** が0で、epoch/generation/slot incarnation/coverage/knownnessがすべて一致する場合だけ公開する。

cache-readはowned bufferへ取得し、最終validationとcaller result commitmentを一つの抽象transitionにする。commit後に到着したwriterとの重なりは許し、通知時点の原本との一致を無条件に要求しない。複数byte/sectorのatomicityは新設しない。

metadata失効とphysical I/Oの停止は別。cache readのshared lease、cache writeのexclusive leaseはactual completionまで保持する。parent timeout/cancel/completeでは解除しない。slot再bindはlease=0かつincarnationのchecked incrementが必要。完了済みfillのpublish前に再bindされた場合も古いincarnationで拒否する。

同じbindingへのcache writeでも、admission時にvisibility失効とincarnation更新を行い、そのchild IDを現在のcontent provenanceにする。後発writeがslotを上書きした後、先行fillのmetadataだけを公開してはいけない。publish失敗で以前のvalid bitを復活させない。独立レビューで発見したこの二つのfillの穴を、別の反例で検証する。

counterはwrapしない。generation枯渇ならそのlineをsession中uncacheable、epoch枯渇ならcache admissionを停止、slot incarnation枯渇ならそのslotをretireする。原本変更の失効を省略してcacheを続けない。再有効化や新sessionの具体的なidentity/rundown条件はD04/D05/D10が追加する。

## Evidence

[有限checker](../../experiments/coherence/README.md)は2byteセル、2writer、2fill、1hit/slotを上限に、明示順序の全interleavingを探索する（二つのfillの追加caseは固定順序）。14個のcase/fault組で反例を要求する。これはcacheの製品実装、全状態の数学的証明、Windows IRP/DMA試験ではない。

## Consequences / Rejected alternatives

- fill source、cache physical I/O、caller-visible resultを別buffer/責務にする。Pin/Arcの存在をrundown証明としない。
- unknown originはI/O errorでもzeroでもない。knownでないlineを公開しない。
- flush/FUAを通常write completionと同一視しない。durable historyと独立oracleはD07、実装はF06で別受入。
- F05は入力を厳密に型付けするだけ。unsafe scheduleをparser errorに隠してmutant検出と称さない。
- ordinary resetで未完了I/Oを消すことは禁止。power_lossはwhole-session終了の抽象モデルだけ。

## Revisit trigger / Reviewer

独立Astra reviewでcontract/checkerを確認してからD02をacceptedにする。D03 engine feasibility、D07のdurability契約、D04の実callback mappingで矛盾が出たらcontractを変更し、既存fixtureと後続Issueを同じ変更で更新する。未知のWindows保証を推測で埋めない。
