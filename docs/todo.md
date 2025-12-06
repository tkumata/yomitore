# yomitore 改善タスク

## 概要

コードレビューで指摘された項目を修正します。

## タスクリスト

### 1. Clippy 警告の修正

- [x] cargo clippy --fix を実行
- [x] src/ui.rs:64 のネストされた if 文を修正

### 2. API タイムアウトの設定を追加

- [x] reqwest::Client にタイムアウトを設定
- [x] 適切なタイムアウト値を定数として定義 (60 秒)

### 3. エラーハンドリングの改善

- [x] 統計保存失敗時にユーザーへ通知
- [x] app.status_message にエラーメッセージを表示

### 4. ターミナルサイズ要求の見直し

- [x] MIN_WIDTH を 150 → 100 に変更
- [x] MIN_HEIGHT を 40 → 30 に変更
- [x] より小さい端末でも動作するように調整

### 5. ハードコーディングされた値を定数化

- [x] calculate_max_scroll の visible_height を動的に取得
- [x] API モデル名を定数として管理 (既に実装済み)
- [x] その他のマジックナンバーを定数化
  - EVENT_POLL_INTERVAL_MS (イベントポーリング間隔)
  - OVERLAY_SIZE_PERCENT (オーバーレイサイズ)
  - MIN_OVERLAY_WIDTH, MIN_OVERLAY_HEIGHT (最小オーバーレイサイズ)
  - BADGE_INTERVAL, MAX_CONSECUTIVE_STREAK, MAX_CUMULATIVE_MILESTONE (バッジ関連)
  - MAX_BADGES_DISPLAY (レポート表示バッジ数)

## 進捗状況

- 開始日時: 2025-12-06
- 完了日時: 2025-12-06
- ステータス: ✅ 完了

## ドキュメント更新

- ✅ docs/HELP.md - 最新機能とトラブルシューティング追加
- ✅ docs/requirements_definition.md - v0.1.8 の要件に更新
- ✅ docs/specification.md - 技術仕様を詳細化、定数管理を追加

## 実施した変更の詳細

### 修正されたファイル

1. `src/ui.rs` - Clippy 警告修正、定数化
2. `src/api_client.rs` - API タイムアウト設定追加
3. `src/main.rs` - エラーハンドリング改善、定数化
4. `src/tui.rs` - ターミナルサイズ要求の緩和
5. `src/app.rs` - 画面サイズ追跡機能追加
6. `src/stats.rs` - バッジ関連の定数化
7. `src/reports.rs` - 表示バッジ数の定数化

### テスト結果

- ✅ ビルド成功
- ✅ Clippy 警告なし
- ✅ 全ユニットテスト通過 (3/3)

## 注意事項

- 各タスク完了後、このファイルを更新すること
- テストを実行して動作確認を行うこと
