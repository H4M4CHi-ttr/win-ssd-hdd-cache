# 貢献ガイド

現在は設計段階です。ライセンス/外部contribution条件の採択前は、第三者ソースのvendoring、外部contributionの受付、一般向け公開を開始しません（GOV01）。通常のCargo/npmパッケージ依存はlicense・hashを確認し、lockfileを更新して利用できます。この利用許可は本repo自体のOSSライセンス採択とは別です。

1. [設計](docs/README.md)と[担当ルール](docs/agent-routing.md)を読む。
2. [Issue一覧](docs/backlog.md)からstatus:readyを選び、依存と必要reviewを確認する。
3. 一つの受入単位をbranch/PRにまとめる。Rustを主とし、必要なGUIはTauri、Windows固有の言語例外はADRにする。
4. テスト先行で実装し、再現コマンド・実行環境・未実行試験をPRへ記録する。
5. 作者と別のreviewerが確認。安全契約に触れる場合はreview:astra以上、出荷前は人間の専門レビューも必要。
6. merge後にIssueへ証拠を残し、依存課題のready条件を再評価する。

F01のportable workspaceで `cargo test --locked`、`cargo fmt --all -- --check`、`cargo clippy --all-targets --locked -- -D warnings` を使います。Windows/Linux両hostでの実行証拠はF10で記録し、Linuxだけの結果をWindows合格とは扱いません。

kernel/実機試験は開発専用環境でのみ行います。Issueに実装を依頼されただけで、利用者の普段のHDDを初期化する権限は生じません。

