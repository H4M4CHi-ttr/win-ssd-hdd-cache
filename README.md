# win-ssd-hdd-cache

Windows向けに、既存のHDDを正本に保ち、SSDを破棄可能な読み取りキャッシュとして利用するOSSを設計・開発するプロジェクトです。

**現在は設計段階です。実用ドライバー、インストーラー、対応を保証するWindowsビルドはまだありません。実データを接続して試用できる状態ではありません。**

## 方針

- 最初はデータ用HDDと別SSDに限定した **clean read cache / Write-Invalidate**。書き込みの正本は常にHDDです。
- SSDキャッシュの消失時も、原本の読み書きの意味を変えない設計を目指します。ただし「read cacheだから無条件に安全」とは扱いません。
- 初期版ではwrite-back、起動ディスク、ページファイル、再起動を跨ぐキャッシュ再利用は扱いません。
- WindowsのI/O捕捉位置とOCF採否は、調査・実証・レビューを経て決めます。既存Linux実装をそのままWindowsに持ち込める前提にはしません。
- **主要実装はRust、GUIが必要な部分はTauri**。Windowsの核心部分だけ、必要に応じC/WDK等を局所採用します。
- 小さな実装課題と、高度な設計・独立レビューが必要な課題を分離します。

## 開発状況

[ロードマップと課題一覧](https://github.com/H4M4CHi-ttr/win-ssd-hdd-cache/issues/1) を起点にしてください。[正式設計と関連文書](docs/README.md)・[貢献ルール](CONTRIBUTING.md)を参照してください。

This project is in the **design phase**: a discardable, clean SSD read cache for selected Windows data HDDs. No production driver or verified hardware support is available yet.

## ライセンスと公開

オープンソースとしての公開を目指しますが、ライセンスの採択と公開範囲の変更は、所有者の明示的な決定をリリース前ゲートとします。第三者コードはそのライセンスに従い、未決の段階で取り込みません。現時点でライセンスが付与済みの完成OSSであるとは表示しません。
