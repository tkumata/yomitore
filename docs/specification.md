# 技術仕様書: 読解力トレーニング CLI (yomitore)

## 1. 概要

本仕様書は、「要件定義書: 読解力トレーニング CLI」に基づき、アプリケーションの技術的実装に関する詳細を定義する。

## 2. アーキテクチャ

本アプリケーションは、以下の主要モジュールから構成される。

- **`main.rs`**: アプリケーションのエントリーポイント。メインループを制御し、各モジュールを協調させる。
- **`api_client.rs`**: CloudGroq API との HTTP 通信を責務とする。API キーを保持し、リクエストの送信とレスポンスの受信を行う。
- **`ui.rs`**: ターミナルへの表示とユーザーからの入力を管理する。`crossterm` クレートをラップし、UI ロジックを集約する。
- **`app_state.rs`**: アプリケーション全体の状態（API クライアント、生成された原文など）を保持する構造体を定義する。
- **`error.rs`**: アプリケーション固有のエラー型を定義する。

## 3. 機能別技術仕様

### 3.1. 認証機能

- **API キー入力**: `rpassword::prompt_password("Enter your CloudGroq API Key: ")` を使用し、入力内容がターミナルに表示されないようにする。
- **認証検証**:
  - **エンドポイント**: `GET https://api.groq.com/openai/v1/models`
  - **処理**: 入力された API キーを`Authorization: Bearer <API_KEY>`ヘッダーに設定し、上記エンドポイントへリクエストを送信する。
  - **成功判定**: HTTP ステータスコード `200 OK` が返却された場合を認証成功とする。
  - **失敗判定**: その他のステータスコードやリクエストエラーが発生した場合は認証失敗とし、エラー内容に応じてメッセージを表示後、再入力を促す。

### 3.2. 文章生成機能

- **エンドポイント**: `POST https://api.groq.com/openai/v1/chat/completions`
- **リクエストボディ (JSON)**:
  ```json
  {
    "model": "openai/gpt-oss-120b",
    "messages": [
      {
        "role": "user",
        "content": "日本語の公的文書のようなお堅い文章を200文字程度で生成してください。"
      }
    ]
  }
  ```
- **レスポンス処理**: レスポンスボディの`choices[0].message.content`から生成された文章を抽出する。

### 3.3. 要約入力機能

- **使用クレート**: `crossterm`
- **入力検知**: `crossterm::event::read()` を用いてキーイベントを同期的に読み取る。
- **入力完了判定**:
  - イベントループ内でキー入力を監視する。
  - `Event::Key` 型のイベントをハンドリングする。
  - `KeyEvent` が `code: KeyCode::Enter` かつ `modifiers: KeyModifiers::SHIFT` の条件を満たした場合、入力完了と判断する。
  - 上記以外のキー入力はバッファに追加し、ターミナルにエコーバックする。

### 3.4. 要約評価機能

- **エンドポイント**: `POST https://api.groq.com/openai/v1/chat/completions`
- **リクエストボディ (JSON)**:
  ```json
  {
    "model": "openai/gpt-oss-120b",
    "messages": [
      {
        "role": "user",
        "content": "以下の『原文』を『要約文』は適切に要約できていますか？ 「はい」か「いいえ」で端的に答えた上で、簡単な解説を加えてください。\n\n# 原文\n{原文テキスト}\n\n# 要約文\n{ユーザー入力テキスト}"
      }
    ]
  }
  ```
- **レスポンス処理**: `choices[0].message.content` から評価結果の文字列を抽出し、そのまま表示する。

## 4. データ構造 (Serde)

API との通信に使用する主要な JSON 構造体は、`serde` を用いて以下のように定義する。

```rust
// (request)
#[derive(serde::Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
}

#[derive(serde::Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

// (response)
#[derive(serde::Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(serde::Deserialize)]
struct Choice {
    message: ChatMessage,
}

#[derive(serde::Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}
```

_注: API の仕様変更により、フィールドが変更・追加される可能性があるため、実装時に公式ドキュメントを再確認すること。_

## 5. エラーハンドリング

- `thiserror` クレートを用いて、以下の内容を含むカスタムエラー型 `AppError` を定義する。
  - `ApiError(reqwest::Error)`: ネットワーク関連のエラー
  - `InvalidApiKey`: API キーが無効である場合のエラー
  - `ResponseParseError(serde_json::Error)`: API レスポンスのパース失敗エラー
- 全ての関数は `Result<T, AppError>` を返すようにし、`main`関数でエラーを集約してハンドリングする。

## 6. 依存クレート (Cargo.toml)

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
crossterm = "0.27"
rpassword = "7"
dotenvy = "0.15"
thiserror = "1"
```
