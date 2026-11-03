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

portable host基盤・整合性契約・strict trace parser・独立oracle付きreplay・13本の固定回帰trace・Rust engine feasibility・Rust/WDK build-only proof・offline capture auditまで、**20課題を独立受入してmainへ統合**しました。最新のP01はWindows Server248件・Linux251件のhost CIを通過しています。これは製品driverの完成ではなく、driver load・実機安全性・性能の検証は未実施です。[F06 replay](docs/evidence/F06.md)、[F07 coherence](docs/evidence/F07.md)、[F11 lifetime/error](docs/evidence/F11.md)、[P01 capture audit](docs/evidence/P01.md)、[統合記録](docs/evidence/2026-11-03-portable-integration.md)、[開発手順](docs/development.md)、[全課題](docs/backlog.md)を参照してください。

[ロードマップ #1](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/1)・[設計文書](docs/README.md)・[貢献ルール](CONTRIBUTING.md)。This is portable-host research and implementation, not a production driver.

## 次の工程と主要Issue

- [P01 #79: 観測ログのoffline完全性検査](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/79) — PR #81で実装・受入済み。欠落・相関・loss counterを検査しますが、実captureの真実性やdriver安全性は認定しません。
- F07/F11は受入済み、F08/F09は未受入。Windows所有権mappingなどの前提を省略せず、D04/D10のgateを維持します。
- [D01 #3: I/O捕捉位置](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/3) — 調査文書とlab手順は統合済み。位置は未採択。capture-only probeの限定寿命契約・source/buildと、別承認の使い捨てWindows labでの実traceが必要です。D09 build-only entryは観測probeではありません。
- [全Issueと依存関係](docs/backlog.md) / [モデル担当とreview規則](docs/agent-routing.md)。

現在、既存バックログのreadyは0件です。次の情報取得は[Windows inventory用Codex引き継ぎ](docs/handoff/2026-11-03-windows-next-step.md)。対象Windowsのread-only構成確認から始め、probe限定契約・source/buildと別承認の観測へ進みます。「完成driverの実機調整だけが残った」とは扱いません。

GUIはTauriを用いますが、CLI/serviceと安全なdata pathの契約を先に固めます。

## ライセンスと公開

リポジトリは所有者の判断でpublicになりました。通常のGitHub Actionsによる検証も許可されています。ただし、ソースの閲覧可能性とOSSライセンスの採択・外部contributionの受付・製品の公開/配布は別です。ライセンスや配布条件はGOV01の所有者決定を待ちます。通常のCargo/npmパッケージ依存はlicense・hashを確認して利用できます。第三者ソースのvendoringや外部contributionの受付はGOV01の方針採択後とし、通知を保持します。現時点でライセンスが付与済みの完成OSSであるとは表示しません。

