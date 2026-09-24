# 技術仕様書: 読解力トレーニング CLI (yomitore)

**バージョン**: 0.1.14
**最終更新日**: 2026-07-17

## 改訂履歴

| Version | Date        | Change Log                                                       |
| ------- | ----------- | ---------------------------------------------------------------- |
| 0.1.14  | 2026-07-17  | Simplify internal structure and remove unused components         |
| 0.1.10  | 2026-01-19  | Update prompt strategy (Prompt Repetition), evaluation logic fix |
| 0.1.9   | 2025-12-06  | API timeout, Define const, Improving error handling              |
| 0.1.0   | 1st Release | 1st Release                                                      |

## 1. 概要

本仕様書は、「要件定義書: 読解力トレーニング CLI」に基づき、アプリケーションの技術的実装に関する詳細を定義する。

## 2. アーキテクチャ

本アプリケーションは、以下の主要モジュールから構成される。

- **`main.rs`**: アプリケーションのエントリーポイント。メインループ、イベントハンドリング、アプリケーションフローを制御
- **`app.rs`**: アプリケーション状態を管理する構造体（App、ViewMode）を定義
- **`api_client.rs`**: Groq と Jev の HTTP 通信を管理。タイムアウト設定、リクエスト/レスポンス処理
- **`ui.rs`**: ratatui を使用した TUI レンダリング。メニュー、設定、トレーニング画面、レポート、ヘルプの描画
- **`tui.rs`**: ターミナル初期化・終了処理。ターミナルサイズチェック
- **`config.rs`**: API キーを環境変数または設定ファイルから読み込み、設定ファイルへ保存する
- **`stats.rs`**: トレーニング統計の管理。バッジシステム、日次/週次集計
- **`reports.rs`**: レポート画面のレンダリング。統計の可視化
- **`help.rs`**: ヘルプコンテンツの管理
- **`error.rs`**: アプリケーション固有のエラー型（thiserror 使用）

文章生成には Groq の生成 AI を利用し、要約の判定には TypeSafe AI の Jev を利用する。改善点3件の文章は従来どおり Groq で生成する。文章生成プロンプトには <https://arxiv.org/abs/2512.14982> を参考にした繰り返し手法を使用する。

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

1. **API キーの読み込み**:
   - Groq キーは `GROQ_API_KEY` を優先し、未設定時は TOML 形式の設定ファイルを読み込む
   - Jev キーは同じ設定ファイルの別項目から読み込む
   - パス:
     - Linux: `~/.config/yomitore/config.toml`
     - macOS: `~/Library/Application Support/yomitore/config.toml`
     - Windows: `%APPDATA%/yomitore/config.toml`
2. **認証検証**:
   - **エンドポイント**: `GET https://api.groq.com/openai/v1/models`
   - **タイムアウト**: 60 秒
   - トレーニング開始時に Groq キーを検証する。失敗時はメニューに理由を表示し、設定画面へ進める
   - Jev キーがない場合は評価を開始せず、設定画面での入力を案内する。無効なキーは Jev の評価要求で検出する

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
- **モード管理**: `TextAreaState::focus` で入力モードを管理
- **テキストエリア**: `rat-text::TextAreaState` を使用
  - ワードラップ対応（`TextWrap::Word(10)`）
  - カーソル位置管理
  - マルチライン入力

**キーバインディング**:

- `i` or `Enter`: 入力モード開始
- `Esc`: 通常モードに戻る
- `Ctrl+S`: 要約送信（`KeyModifiers::CONTROL`）
- その他: rat-text が処理

### 3.4. 要約評価機能 (api_client.rs, evaluation.rs)

- **入力**: 原文とユーザーの要約文。両方を Jev と Groq に送る
- **Jev エンドポイント**: `POST https://api.typesafe.ai/v1/systemone`、Bearer 認証、`model: "jev-latest"`
- **Jev 質問**: 1回の要求に `appropriate`（Noul）、`importance`・`conciseness`・`accuracy`（各5段階の Score）、`overall`（合格/不合格の Choice）を含める。各 Score は観点ごとに具体的な5つの状態を記述する
- **Groq エンドポイント**: `POST https://api.groq.com/openai/v1/chat/completions`。原文と要約文を比較し、改善点1〜3だけを従来のテキスト形式で生成する
- **変換**: Noul は0.5以上を「はい」とする。Score の0〜4の小数値を四捨五入して1を加え、1〜5点として表示・保存する。Choice の合格/不合格を合否に使う
- **総合得点**: 丸め前の3つの Score から `round(100 × (importance + conciseness + accuracy) / 12)` で0〜100の整数点を算出する。合否の閾値には使わない
- **結果**: Jev の5判定、Groq の改善点3件、総合得点を固定順で表示する。全項目が揃った場合だけ統計に保存する
- **失敗**: API エラー、回答の欠落、型または値の不正、改善点の解析失敗は評価失敗として扱い、統計に保存しない
- **タイムアウト**: 両 API とも60秒

### 3.5. バディ育成機能 (stats.rs, reports.rs)

**実装メソッド**:

- `Buddy::add_exp()`: 経験値加算。5 exp でレベルアップ。
- `TrainingStats::check_buddy_penalty()`: ペナルティ判定。最終トレーニングから3日経過でレベルダウン。
- `get_buddy_ascii(level: u32) -> &'static str`: 500ms 間隔で切り替わるアニメーションフレームを返す。

**ロジック**:

- 合格時: `exp += 1`
- レベルアップ: `exp >= 5` (Level 2の場合は `exp >= 10`) → `level += 1`, `exp = 0`
- ペナルティ: `now - last_training_date >= 3 days` → `level -= 1` (if level > 1), `exp = 0`

### 3.6. UI レンダリング (ui.rs, tui.rs)

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

### 3.6.1. メニュー描画詳細 (ui.rs)

- 文字数選択ブロックは `MENU_OPTIONS` を 1 行ずつ描画する
- 選択状態は文字列幅の増減ではなくスタイルで表現する
- 選択肢の間に空行を挟まないことで、ブロック内の余白を最小化する

### 3.7. 統計管理 (stats.rs)

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
    pub buddy: Buddy,
    pub last_training_date: Option<DateTime<Local>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Buddy {
    pub level: u32,
    pub exp: u32,
}

impl Default for Buddy {
    fn default() -> Self {
        Self { level: 1, exp: 0 }
    }
}

pub struct TrainingResult {
    pub timestamp: DateTime<Local>,
    pub passed: bool,
    pub evaluation: Option<EvaluationScores>,
}

pub enum BadgeType {
    ConsecutiveStreak(usize),
    CumulativeMilestone(usize),
}

pub struct EvaluationScores {
    pub appropriate: bool,
    pub importance: u8,
    pub conciseness: u8,
    pub accuracy: u8,
    pub improvement1: String,
    pub improvement2: String,
    pub improvement3: String,
    pub overall_passed: bool,
}
```

**バッジ授与ロジック**:

- `add_result_with_evaluation(passed: bool, evaluation: Option<EvaluationScores>)` で結果を追加
- 連続正解時: `current_streak`をインクリメント、5 の倍数でバッジ授与
- 不正解時: `current_streak`をリセット
- 累積正解: 全結果から正解数をカウント、5 の倍数でバッジ授与

**評価スコア集計**:

- 直近180日の `EvaluationScores` を集計して平均・中央値・件数を表示する

**月次ヒートマップ**:

- 入力データは `TrainingStats::get_daily_stats(180)` の戻り値を使用する
- 対象期間は今日を含む直近180日とする
- 横軸は週とし、左から古い週、右へ行くほど新しい週とする
- 週列は日曜始まりとして扱い、対象期間内に含まれる週を表示する
- 各セルは、週列と曜日行の交点にある1日を表す
- 縦軸は曜日とし、画面上から土、金、木、水、火、月、日、つまり下から日、月、火、水、木、金、土の順に表示する
- 各セルは Unicode block 文字で描画する
- 横軸ヘッダーは表示しない
- セル同士の間隔は最小限に詰める
- 対象期間外、またはその週に存在しない曜日のセルは `·` で表示する
- 対象期間内の日は `■` で表示する
- 未実施日は既存の未実施色、全不正解・混在・良・優・秀は既存の正解率ベースの色を使用する
- `##`、`--` などの ASCII 代替セルは使用しない
- セルの色判定は `get_heatmap_cell_style(total, correct)` 相当の責務に閉じ、統計集計ロジックへ持ち込まない
- 表示領域が狭い場合は凡例を省略しても、週列、曜日ラベル、ヒートマップ本体の対応を維持する

**データ永続化**:

- パス:
  - Linux: `~/.config/yomitore/stats.json`
  - macOS: `~/Library/Application Support/yomitore/stats.json`
  - Windows: `%APPDATA%/yomitore/stats.json`
- 形式: JSON（serde_json 使用）
- 保存: `save() -> Result<(), Box<dyn std::error::Error>>`
- 読み込み: `load() -> Result<Self, Box<dyn std::error::Error>>`
  - 存在しない場合は新規作成
  - 読み込み後、`recalculate_streak()`と`rebuild_badges_from_history()`を実行

### 3.8. テスト戦略 (evaluation.rs)

**目的**: 実際の Groq・Jev API を利用せずに、評価結果の変換、解析、得点、合否判定を自動テストする。

**方針**:

- `ApiClient` は単一の具象型として使用する
- Jev 応答の検証・得点計算と、Groq の改善点のパースを API 通信から分離して検証する
- テストはモジュール名で絞り込み実行できる構成にする
- Jev 応答の5項目は必須とし、型と値域を検証する。Score の丸めと総合得点の0・100点を境界値として検証する
- Groq の改善点は3件必須とし、欠落・重複・壊れた形式をエラーとする
- 評価結果の表示は固定順とする
- パース失敗時は「評価結果の形式が不正です」と表示する
- レポートは直近180日の平均・中央値・件数を表示する
- 月次ヒートマップは週横軸、曜日縦軸、Unicode block セルで表示する
- Groq 応答の余分な行は無視し、先頭の箇条書き記号が異なっていても改善点を解釈する
- 片方の API が失敗した場合は統計を保存しない。設定保存、Groq 環境変数優先、旧 `stats.json` 読み込みを検証する

**テスト用固定レスポンス**:

```text
- 改善点1: なし
- 改善点2: なし
- 改善点3: なし
```

```text
- 改善点1: 情報不足
- 改善点2: 要約が長すぎる
- 改善点3: 原文の主旨を外れている
```

```text
not a valid format
```

**最低限のテストケース**:

- Jev の合格・不合格、各 Score の境界値
- Groq の改善点3件と壊れた形式
- 設定画面の保存・キャンセル、API 失敗時の統計保存抑止

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
    Settings,  // API キー設定画面
    Normal,    // トレーニング画面
    Report,    // レポート画面
    Help,      // ヘルプ画面
}

pub struct App {
    pub api_client: Option<ApiClient>,
    pub original_text: String,
    pub original_text_scroll: u16,
    pub evaluation_text: String,
    pub evaluation_passed: bool,
    pub status_message: String,
    pub should_quit: bool,
    pub text_area_state: TextAreaState,
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

`config.rs` は `GROQ_API_KEY` と設定ファイルの `api_key` から有効な Groq キーを選び、設定ファイルの `jev_api_key` から Jev キーを読む。保存時は既存の TOML 項目を保持する。

保存形式（TOML）:

```toml
api_key = "your_api_key_here"
jev_api_key = "your_jev_api_key_here"
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
const REPORT_DAYS: usize = 180;
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

### 8.1. ユニットテスト

各モジュールに `#[cfg(test)]` ブロックを設け、純粋関数を中心にテストを実装している。

#### 8.1.1. 統計管理 (`stats.rs`)

- バッジ授与ロジック（連続正解、累積正解）
- 履歴に基づくストリークの再計算
- 日次/週次統計の集計ロジック
- 中央値、平均計算のエッジケース

#### 8.1.2. APIクライアント (`api_client.rs`)

- Jev の5回答の型・値域・欠落を検証
- Score の整数表示への丸めと、元値から算出する総合得点の境界を検証
- Groq の改善点3件の解析（箇条書き記号の揺らぎ、欠落・重複）と結果表示を検証

#### 8.1.3. 設定管理 (`config.rs`)

- 両キーの TOML 読み書き、Groq 環境変数の優先、既存値の保持、Unix 系の保存権限を検証

#### 8.1.4. エラー定義 (`error.rs`)

- 各エラー型のカスタム表示メッセージの検証

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
   - Unix 系で設定画面から保存するとき、設定ファイルを 600 パーミッションにする
   - API キーを画面、エラー表示、ログに出さない
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
