# win-ssd-hdd-cache

Windows向けに、既存のHDDを正本に保ち、SSDを破棄可能な読み取りキャッシュとして利用するOSSを設計・開発するプロジェクトです。

**現在は設計とportable Rust基盤の実装段階です。実用ドライバー、インストーラー、対応を保証するWindowsビルドはまだありません。実データを接続して試用できる状態ではありません。**

## 方針

- 最初はデータ用HDDと別SSDに限定した **clean read cache / Write-Invalidate**。書き込みの正本は常にHDDです。
- SSDキャッシュの消失時も、原本の読み書きの意味を変えない設計を目指します。ただし「read cacheだから無条件に安全」とは扱いません。
- 初期版ではwrite-back、起動ディスク、ページファイル、再起動を跨ぐキャッシュ再利用は扱いません。
- 後続のhost qualificationには独自の最小Rust coreを採択しました。OCFのWindows移植は今回見送り。製品のI/O捕捉位置・Windows境界・engine適合性は、引き続き実証と独立レビューで決めます。
- **主要実装はRust、GUIが必要な部分はTauri**。Windowsの核心部分だけ、必要に応じC/WDK等を局所採用します。
- 小さな実装課題と、高度な設計・独立レビューが必要な課題を分離します。

## 開発状況

portable host基盤・整合性契約・strict trace parser・独立oracle付きreplay・13本の固定回帰trace・Rust engine feasibility・offline capture auditに加え、観測専用driverとRust collectorのsource/buildまで、**23課題を独立受入してmainへ統合**しました。最新P04のhost CIはWindows342件・Linux344件が成功。Cの同一driver sourceを使う69件のhost試験、実WDK compile/link、Windows collectorの有効feature付きcompile/linkも通過しています。これは製品cache driverの完成ではなく、driver load・実機安全性・性能の検証は未実施です。[F06 replay](docs/evidence/F06.md)、[F07 coherence](docs/evidence/F07.md)、[F11 lifetime/error](docs/evidence/F11.md)、[P01 capture audit](docs/evidence/P01.md)、[P02 source/build](docs/evidence/P02.md)、[P03 v2](docs/evidence/P03.md)、[P04 source/buildの証拠](docs/evidence/P04.md)、[開発手順](docs/development.md)、[全課題](docs/backlog.md)を参照してください。

[ロードマップ #1](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/1)・[設計文書](docs/README.md)・[貢献ルール](CONTRIBUTING.md)。This is portable-host research and implementation, not a production driver.

## 次の工程と主要Issue

- [P04 #89: 観測driverとRust collectorのsource/build](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/89) — [候補sourceとhost試験](experiments/stack-trace/probe/README.md)を実装し、独立whole-candidate review・実WDK/Windows CIを経てPR #91でmain統合済み。実deviceへの登録・ロード・観測は行っていません。
- [P03 #86: 不明Informationを保持するcapture v2](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/86) — PR #88で実装・受入済み。未取得値をゼロやevent lossに置換せず、v1の互換性を保ちます。
- [P02 #83: bounded producerとcompile-only snapshot](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/83) — PR #85でsource/build受入済み。固定64contexts/256recordsのRust coreとWDK scalar object。driver登録・装着・ロード・live transportは含みません。
- [P01 #79: 観測ログのoffline完全性検査](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/79) — PR #81で実装・受入済み。欠落・相関・loss counterを検査しますが、実captureの真実性やdriver安全性は認定しません。
- F07/F11は受入済み、F08/F09は未受入。Windows所有権mappingなどの前提を省略せず、D04/D10のgateを維持します。
- [D01 #3: I/O捕捉位置](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/3) — 調査文書とlab手順は統合済み。位置は未採択。P04の限定契約・source/buildは受入済みですが、指定Windowsのinventory、対象適合性・登録/回復手順のreview、別承認の使い捨てlabでの実traceが必要です。
- [全Issueと依存関係](docs/backlog.md) / [モデル担当とreview規則](docs/agent-routing.md)。

現在のready課題は0件です。残る33件は実Windowsの事実・設計gate等の前提待ち、1件は所有者判断待ちです。次の技術的な入力は[Windows inventory用Codex引き継ぎ](docs/handoff/2026-11-03-windows-next-step.md)に沿う読み取り専用調査です。C17/WDKは実験用observerの限定境界、collection/auditはRustです。実装済み部品と別承認の実機観測を区別し、「完成driverの実機調整だけが残った」とは扱いません。

GUIはTauriを用いますが、CLI/serviceと安全なdata pathの契約を先に固めます。

## ライセンスと公開

リポジトリは所有者の判断でpublicになりました。通常のGitHub Actionsによる検証も許可されています。ただし、ソースの閲覧可能性とOSSライセンスの採択・外部contributionの受付・製品の公開/配布は別です。ライセンスや配布条件はGOV01の所有者決定を待ちます。通常のCargo/npmパッケージ依存はlicense・hashを確認して利用できます。第三者ソースのvendoringや外部contributionの受付はGOV01の方針採択後とし、通知を保持します。現時点でライセンスが付与済みの完成OSSであるとは表示しません。

