# 技術仕様書: 読解力トレーニング CLI (yomitore)

**バージョン**: 0.1.10
**最終更新日**: 2026-01-19

## 改訂履歴

| Version | Date        | Change Log                                                       |
| ------- | ----------- | ---------------------------------------------------------------- |
| 0.1.10  | 2026-01-19  | Update prompt strategy (Prompt Repetition), evaluation logic fix |
| 0.1.9   | 2025-12-06  | API timeout, Define const, Improving error handling              |
| 0.1.0   | 1st Release | 1st Release                                                      |

## 1. 概要

本仕様書は、「要件定義書: 読解力トレーニング CLI」に基づき、アプリケーションの技術的実装に関する詳細を定義する。

## 2. アーキテクチャ

本アプリケーションは、以下の主要モジュールから構成される。

- **`main.rs`**: アプリケーションのエントリーポイント。メインループ、イベントハンドリング、アプリケーションフローを制御
- **`app.rs`**: アプリケーション状態を管理する構造体（App、ViewMode）を定義
- **`api_client.rs`**: Groq API との HTTP 通信を管理。タイムアウト設定、リクエスト/レスポンス処理
- **`ui.rs`**: ratatui を使用した TUI レンダリング。メニュー、トレーニング画面、レポート、ヘルプの描画
- **`tui.rs`**: ターミナル初期化・終了処理。ターミナルサイズチェック
- **`config.rs`**: 設定ファイルの読み書き。API キーの永続化
- **`stats.rs`**: トレーニング統計の管理。バッジシステム、日次/週次集計
- **`reports.rs`**: レポート画面のレンダリング。統計の可視化
- **`help.rs`**: ヘルプコンテンツの管理
- **`error.rs`**: アプリケーション固有のエラー型（thiserror 使用）

文字列生成と要約評価には生成 AI を利用してますが、<https://arxiv.org/abs/2512.14982> を参考にしてプロンプトを繰り返す手法 (Prompt Repetition) を採用しています。

### アーキテクチャ図

```text
┌───────────────────────────────────────────┐
│                  main.rs                  │
│  (Entry Point, Main Loop, Event Handling) │
└───────────────┬───────────────────────────┘
                │
    ┌───────────┼───────────┐
    │           │           │
    ▼           ▼           ▼
┌────────┐ ┌────────┐ ┌──────────┐
│ app.rs │ │ tui.rs │ │ error.rs │
└────┬───┘ └────┬───┘ └──────────┘
     │          │
     │          ▼
     │     ┌─────────┐
     │     │  ui.rs  │──────┬────────────┬────────┐
     │     └─────────┘      │            │        │
     │                      ▼            ▼        │
     │                 ┌──────────┐ ┌─────────┐   │
     │                 │reports.rs│ │ help.rs │   │
     │                 └──────────┘ └─────────┘   │
     │                                            ▼
     └────────┬───────────────────────────────────┐
              │                                   │
              ▼                                   ▼
      ┌──────────────┐                     ┌──────────────┐
      │api_client.rs │                     │  config.rs   │
      │  (Groq API)  │                     │  stats.rs    │
      └──────────────┘                     └──────────────┘
```

## 3. 機能別技術仕様

### 3.1. 認証機能 (main.rs, config.rs)

**実装関数**: `authenticate() -> Result<ApiClient, AppError>`

1. **設定ファイルからの読み込み**:
   - `config::load_api_key()` で TOML 形式の設定ファイルを読み込む
   - パス: `~/.config/yomitore/config.toml`
2. **環境変数からの読み込み**:
   - `env::var("GROQ_API_KEY")` で環境変数を確認
3. **認証検証**:
   - **エンドポイント**: `GET https://api.groq.com/openai/v1/models`
   - **タイムアウト**: 60 秒
   - **処理**: `ApiClient::validate_credentials()` で認証チェック
   - **成功**: API キーを設定ファイルに保存（Unix 系では 600 パーミッション）
   - **失敗**: `AppError::InvalidApiKey` を返す

### 3.2. 文章生成機能 (api_client.rs)

**実装関数**: `ApiClient::generate_text(prompt: &str) -> Result<String, AppError>`

- **エンドポイント**: `POST https://api.groq.com/openai/v1/chat/completions`
- **タイムアウト**: 60 秒（`API_TIMEOUT_SECS`定数）
- **モデル**: `openai/gpt-oss-120b`（`CHAT_MODEL`定数）
- **リクエストボディ**:
  ランダムにプロンプトを変更する。

  ```json
  {
    "model": "openai/gpt-oss-120b",
    "messages": [
      {
        "role": "user",
        "content": "日本の公的文書（省庁や自治体が発行する通知や報告書）の文体で、感情表現や口語表現を避け、形式的かつ客観的な文章をN文字程度で生成してください。"
      }
    ]
  }
  ```

  ```json
  {
    "model": "openai/gpt-oss-120b",
    "messages": [
      {
        "role": "user",
        "content": "日本の新聞記事の本文として、事実関係を中心に客観的かつ簡潔な文体で文章をN文字程度で生成してください。"
      }
    ]
  }
  ```

- **レスポンス処理**:
  - `choices[0].message.content` から生成文を抽出
  - null の場合は空文字列を返す
  - エラー時は `AppError::ApiError` を返す

### 3.3. 要約入力機能 (main.rs, ui.rs)

**使用クレート**: `ratatui`, `crossterm`, `rat-text`

- **イベントポーリング**: `event::poll(Duration::from_millis(EVENT_POLL_INTERVAL_MS))`
  - `EVENT_POLL_INTERVAL_MS = 100` (定数化)
- **モード管理**: `App::is_editing` フラグで入力モードを管理
- **テキストエリア**: `rat-text::TextAreaState` を使用
  - ワードラップ対応（`TextWrap::Word(10)`）
  - カーソル位置管理
  - マルチライン入力

**キーバインディング**:

- `i` or `Enter`: 入力モード開始
- `Esc`: 通常モードに戻る
- `Ctrl+S`: 要約送信（`KeyModifiers::CONTROL`）
- その他: rat-text が処理

### 3.4. 要約評価機能 (api_client.rs)

**実装関数**: `ApiClient::evaluate_summary(original: &str, summary: &str) -> Result<String, AppError>`

- **エンドポイント**: `POST https://api.groq.com/openai/v1/chat/completions`
- **タイムアウト**: 60 秒
- **プロンプト**:

  ```text
  以下の「原文」と「要約文」を比較し、要約として適切か評価してください。

  # 評価ルール
  - 出力は必ず以下のフォーマットのみ使用すること
  - 数値は 1〜5 の整数のみ
  - 余計な文章や注釈は禁止
  - Markdown 記法は禁止

  # 出力フォーマット(厳守)
  - 適切な要約か: はい／いいえ
  - 重要情報の抽出: [1-5]
  - 簡潔性: [1-5]
  - 正確性: [1-5]
  - 改善点1: ...
  - 改善点2: ...
  - 改善点3: ...
  - 総合評価: 合格/不合格

  # 採点基準
  - 5: 非常に優れている
  - 3: 可もなく不可もなく
  - 1: 明確な問題がある

  # 原文
  {original_text}

  # 要約文
  {summary_text}
  ```

  ※ 実装上は上記の内容全体を `format!(...).repeat(2)` を使用して2回繰り返し、LLMへ送信する。

- **合否判定**: 評価結果のテキスト内に「総合評価: 合格」という文字列が含まれているかチェック（`evaluation.contains("総合評価: 合格")`）

### 3.5. UI レンダリング (ui.rs, tui.rs)

**ターミナル初期化**:

```rust
// tui.rs
const MIN_WIDTH: u16 = 100;
const MIN_HEIGHT: u16 = 30;

pub fn init() -> io::Result<Tui> {
    // サイズチェック
    let (width, height) = size()?;
    if width < MIN_WIDTH || height < MIN_HEIGHT {
        return Err(...);
    }
    // AlternateScreen有効化、Rawモード有効化
}
```

**レイアウト構成**:

- ヘッダー: 1 行（固定）
- コンテンツ: 残り領域（可変）
  - 左 50%: 原文表示（スクロール可能）
  - 右 50%: 要約入力（TextArea）
- ステータスバー: 3 行（固定）

**オーバーレイ**:

```rust
// ui.rs定数
const OVERLAY_SIZE_PERCENT: u16 = 75;
const MIN_OVERLAY_WIDTH: u16 = 40;
const MIN_OVERLAY_HEIGHT: u16 = 10;
```

**動的画面サイズ取得**:

```rust
// ui.rs: render()
app.terminal_width = frame.area().width;
app.terminal_height = frame.area().height;
```

### 3.6. 統計管理 (stats.rs)

**定数定義**:

```rust
const BADGE_INTERVAL: usize = 5;             // バッジ獲得間隔
const MAX_CONSECUTIVE_STREAK: usize = 50;    // 最大連続正解
const MAX_CUMULATIVE_MILESTONE: usize = 100; // 最大累積正解
```

**データ構造**:

```rust
pub struct TrainingStats {
    pub results: Vec<TrainingResult>,
    pub badges: Vec<Badge>,
    pub current_streak: usize,
}

pub struct TrainingResult {
    pub timestamp: DateTime<Local>,
    pub passed: bool,
}

pub enum BadgeType {
    ConsecutiveStreak(usize),
    CumulativeMilestone(usize),
}
```

**バッジ授与ロジック**:

- `add_result(passed: bool)` で結果を追加
- 連続正解時: `current_streak`をインクリメント、5 の倍数でバッジ授与
- 不正解時: `current_streak`をリセット
- 累積正解: 全結果から正解数をカウント、5 の倍数でバッジ授与

**データ永続化**:

- パス: `~/.config/yomitore/stats.json`
- 形式: JSON（serde_json 使用）
- 保存: `save() -> Result<(), Box<dyn std::error::Error>>`
- 読み込み: `load() -> Result<Self, Box<dyn std::error::Error>>`
  - 存在しない場合は新規作成
  - 読み込み後、`recalculate_streak()`と`rebuild_badges_from_history()`を実行

### 3.7. テスト戦略 (api_client.rs, app.rs)

**目的**: 実際の GroqCloud API を利用せずに、評価結果の判定までを自動テストする。

**方針**:

- `ApiClient` をトレイト化し、本番実装とテスト実装を差し替え可能にする
- 評価結果のパースと合否判定は純粋関数として切り出し、ユニットテスト対象とする
- テストはモジュール名で絞り込み実行できる構成にする

**テスト用固定レスポンス**:

```text
- 適切な要約か: はい／いいえ
- 重要情報の抽出: 4
- 簡潔性: 4
- 正確性: 4
- 改善点1: なし
- 改善点2: なし
- 改善点3: なし
- 総合評価: 合格
```

```text
- 適切な要約か: いいえ
- 重要情報の抽出: 2
- 簡潔性: 2
- 正確性: 2
- 改善点1: 情報不足
- 改善点2: 要約が長すぎる
- 改善点3: 原文の主旨を外れている
- 総合評価: 不合格
```

```text
not a valid format
```

**最低限のテストケース**:

- 合格 1 件
- 不合格 1 件
- 壊れた形式 1 件

## 4. データ構造

### 4.1. API 通信 (Serde)

```rust
// api_client.rs

// リクエスト
#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<ChatMessage<'a>>,
}

#[derive(Serialize)]
struct ChatMessage<'a> {
    role: &'a str,
    content: &'a str,
}

// レスポンス
#[derive(Deserialize, Debug)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Deserialize, Debug)]
struct Choice {
    message: ChatResponseMessage,
}

#[derive(Deserialize, Debug)]
struct ChatResponseMessage {
    content: Option<String>,  // nullの可能性あり
}
```

### 4.2. アプリケーション状態

```rust
// app.rs

#[derive(PartialEq, Clone, Copy)]
pub enum ViewMode {
    Menu,      // メニュー画面
    Normal,    // トレーニング画面
    Report,    // レポート画面
    Help,      // ヘルプ画面
}

pub struct App {
    pub api_client: Option<ApiClient>,
    pub is_editing: bool,
    pub original_text: String,
    pub original_text_scroll: u16,
    pub evaluation_text: String,
    pub evaluation_passed: bool,
    pub status_message: String,
    pub should_quit: bool,
    pub text_area_state: TextAreaState,
    pub is_evaluating: bool,
    pub show_evaluation_overlay: bool,
    pub evaluation_overlay_scroll: u16,
    pub view_mode: ViewMode,
    pub stats: TrainingStats,
    pub character_count: u16,
    pub selected_menu_item: usize,
    pub help_scroll: u16,
    pub terminal_width: u16,
    pub terminal_height: u16,
}
```

### 4.3. 設定データ

```rust
// config.rs

#[derive(Serialize, Deserialize, Default)]
struct Config {
    api_key: Option<String>,
}
```

保存形式（TOML）:

```toml
api_key = "your_api_key_here"
```

## 5. エラーハンドリング

### 5.1. エラー型定義 (error.rs)

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("API request failed: {0}")]
    ApiError(#[from] reqwest::Error),

    #[error("Failed to parse API response: {0}")]
    ResponseParseError(#[from] serde_json::Error),

    #[error("Invalid API Key.")]
    InvalidApiKey,

    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("API response contained no choices.")]
    NoChoicesInResponse,
}
```

### 5.2. エラーハンドリング戦略

1. **main 関数**: `Result<(), AppError>`を返し、最上位でエラーをキャッチ
2. **API 通信**: タイムアウト時は`reqwest::Error`が発生し、`AppError::ApiError`に変換
3. **統計保存失敗**: ユーザーに通知し、処理を継続

   ```rust
   if let Err(e) = app.stats.save() {
       app.status_message = format!("Warning: Failed to save stats: {}", e);
       eprintln!("Failed to save stats: {}", e);
   }
   ```

4. **ターミナル復元**: panic やエラー時も`tui::restore()`を確実に実行

## 6. 定数管理

### 6.1. API 関連定数 (api_client.rs)

```rust
const API_BASE_URL: &str = "https://api.groq.com/openai/v1";
const CHAT_COMPLETIONS_ENDPOINT: &str = "/chat/completions";
const MODELS_ENDPOINT: &str = "/models";
const CHAT_MODEL: &str = "openai/gpt-oss-120b";
const API_TIMEOUT_SECS: u64 = 60;
```

### 6.2. UI 関連定数 (main.rs, ui.rs)

```rust
// main.rs
const EVENT_POLL_INTERVAL_MS: u64 = 100;
const OVERLAY_SIZE_PERCENT: u16 = 75;

// ui.rs
const OVERLAY_SIZE_PERCENT: u16 = 75;
const MIN_OVERLAY_WIDTH: u16 = 40;
const MIN_OVERLAY_HEIGHT: u16 = 10;

// tui.rs
const MIN_WIDTH: u16 = 100;
const MIN_HEIGHT: u16 = 30;
```

### 6.3. 統計関連定数 (stats.rs, reports.rs)

```rust
// stats.rs
const BADGE_INTERVAL: usize = 5;
const MAX_CONSECUTIVE_STREAK: usize = 50;
const MAX_CUMULATIVE_MILESTONE: usize = 100;

// reports.rs
const DAYS_IN_MONTH: usize = 30;
const WEEKS_TO_SHOW: usize = 4;
const MAX_BADGES_DISPLAY: usize = 20;
```

### 6.4. メニュー関連定数 (app.rs)

```rust
pub const MENU_OPTIONS: [u16; 4] = [400, 720, 1440, 2880];
```

## 7. 依存クレート (Cargo.toml)

```toml
[package]
name = "yomitore"
version = "0.1.9"
edition = "2024"

[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.13", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
dirs = "6.0"
toml = "0.9"
ratatui = { version = "0.30", features = ["crossterm", "unstable-rendered-line-info"] }
crossterm = { version = "0.29", features = ["event-stream"] }
rat-text = "3.0"
chrono = { version = "0.4", features = ["serde"] }
```

## 8. テスト

### 8.1. ユニットテスト (stats.rs)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_badge_awarding_consecutive() { ... }

    #[test]
    fn test_streak_reset_on_incorrect() { ... }

    #[test]
    fn test_rebuild_badges_from_history() { ... }
}
```

### 8.2. テスト実行

```bash
cargo test
cargo clippy --all-targets --all-features
```

## 9. パフォーマンス最適化

### 9.1. 実装済み最適化

1. **API タイムアウト設定**: 60 秒で明示的にタイムアウト
2. **イベントポーリング**: 100ms ごとに効率的にポーリング
3. **動的画面サイズ取得**: レンダリング時に実際のサイズを使用
4. **定数化**: マジックナンバーを削減、コンパイル時最適化
5. **ライフタイム活用**: API リクエスト構造体で`&'a str`を使用し、コピーを削減

### 9.2. メモリ管理

- 不要な`String`コピーを避け、参照を活用
- `Vec`の事前容量確保は行わず、動的に拡張（使用量が予測困難なため）
- 統計データは JSON 形式で効率的にシリアライズ

## 10. セキュリティ考慮事項

1. **API キー保護**:
   - ファイルパーミッション 600（Unix 系）
2. **入力検証**:
   - API 応答の`content`フィールドが null の場合を考慮
   - ファイル I/O 時の適切なエラーハンドリング
3. **データ永続化**:
   - 設定ファイルは暗号化せず、OS のファイルシステム保護に依存
   - 統計データは平文 JSON（機密情報を含まない）

## 11. 将来の拡張性

### 11.1. 設定可能にできる項目

- API モデル選択
- タイムアウト時間
- ターミナルサイズ要件
- バッジ獲得間隔
- 色テーマ

### 11.2. 機能拡張の可能性

- 複数の AI モデル対応
- カスタムプロンプトテンプレート
- エクスポート機能（CSV, PDF）
- マルチユーザー対応
- クラウド同期

## 評価結果ダイアログ余白追加 仕様

- 画面全体の領域から上下左右2セル分を内側に縮めた領域を「利用可能領域」とする
- ダイアログは利用可能領域の中央に配置する
- ダイアログのサイズは利用可能領域に対する現行の割合計算を使う
- 最小サイズを超えるような拡大は行わず、利用可能領域を上限として収める
