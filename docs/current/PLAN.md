# PLAN

## 目的

監査で確認した過剰な抽象化、未使用コード、不要な依存関係を削除し、既存動作を維持したままコードベースを簡素化する。

## 成功基準

- `ApiClient` を直接保持し、API 通信を伴わない評価解析テストを維持する
- ヘルプを `include_str!` で埋め込み、`build.rs` を削除する
- アプリケーション状態を `App` に直接保持する
- 未使用ファイル、完了済み TODO、未使用依存を削除する
- 認証キーの取得優先順位と既存の保存データ形式を維持する
- Stop フックが Rust 関連差分だけを対象に、`make check`、`make build`、レビュー要求の順序を守る
- `make check`、`make build`、コードレビューが成功する

## Phase 1: ドキュメント同期

- REQUIREMENTS、SPECIFICATIONS、DESIGN、ADR、Current Task を簡素化後の契約に更新する
- 過去の ADR は履歴として維持する

## Phase 2: Rust コードと依存関係の簡素化

- `ApiClientLike`、`ApiFuture`、`TestApiClient` を削除する
- `AppFlags` 階層を削除し、重複しない状態だけを `App` に直接保持する
- ヘルプを `include_str!` で埋め込む
- テスト専用面積計算ラッパーと `TrainingStats::new` を削除する
- 重複する環境変数再認証分岐を削除する
- `build.rs`、`test_badges.rs`、`docs/TODO.md`、`tempfile` を削除する

## Phase 3: 検証フックの簡素化

- 検証済み差分を単一フィンガープリントで管理する
- 未検証差分では `make check`、次の Stop で `make build` を実行する
- build 成功後にコードレビューを要求し、同一差分では再実行しない
- Codex と Copilot の既存出力契約を維持する

## Phase 4: 検証とレビュー

- `bash -n .agent-hooks/verify_pipeline.sh`
- `make check`
- `make build`
- 現在差分のコードレビュー
