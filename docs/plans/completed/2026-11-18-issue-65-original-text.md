# Issue #65 問題文の文字欠けを修正する

## Status

- Phase: completed
- Approval: approved

## Goal

問題文の文字が欠ける箇所を特定し、全文を読めるようにする。Issue では文頭と文末の欠落が多いと報告されている。報告時の端末サイズは 150 列 × 49 行だった。

## Scope

- Groq の応答から `App::original_text` への格納までを調査する。
- TUI の原文表示、折り返し、スクロール範囲を調査する。
- 原因が確認できた処理を修正し、再現条件を回帰テストに追加する。

## Required Behavior

- 完全な問題文を受け取った場合、文頭から文末までの文字を保持する。
- 問題文が表示領域を超える場合、利用者は既存のスクロール操作で全文を読める。
- 問題文の生成以外でも修正対象の処理を共有する場合、既存の呼び出し元への影響を確認する。

## Tasks

### Task 1 調査と再現

- **Objective** — 問題文の文字が欠ける処理と条件を特定する。
- **Scope** — `src/api_client.rs` の応答抽出、`src/main.rs` と `src/app.rs` の受け渡し、`src/ui.rs` と `src/events.rs` の表示とスクロールを調べる。
- **Acceptance Criteria** — 欠けた文字を確認できる入力と操作を記録し、API 応答、格納値、TUI 表示のどこで欠落が生じたかを示す。
- **Verification** — まず 150 列 × 49 行で TUI を確認する。文頭と文末を識別できる日本語の問題文を使う。表示領域に収まる場合と、折り返しやスクロールが必要な場合を比較する。固定入力で TUI 側の欠落を再現できない場合は、Groq の応答本文と終了理由を調べる。
- **Result** — 150 列 × 49 行で再現した。原文枠の内幅は 73 列。`a`、`日` 35 字、`末` を並べた行では、Ratatui が `末` を原文枠の右端にまたがって描画した。格納した問題文には文頭と文末があり、描画では右枠線が消えた。報告時の問題文との一致は未確認。

### Task 2 修正と回帰検証

- **Objective** — 特定した原因を修正する。
- **Scope** — 原因のある処理と回帰テストを変更する。
- **Acceptance Criteria** — Task 1 の再現条件で文字欠けが解消する。文頭と文末を含む全文を既存の操作で確認できる。
- **Verification** — 修正前に失敗し、修正後に成功する回帰テストを実行する。`make check` と `make build` を実行する。
- **Result** — 表示幅で文字を折り返し、折り返し後の行数でスクロールの上限を決めた。150 列 × 49 行の回帰テストで右枠線の維持と、下スクロールで `末` が読めることを確認した。`make check` は 45 テストを含めて成功した。`make build` は成功した。

## Open Questions

- 報告時の問題文と Groq の応答は未確認。回帰テストには固定の問題文を使用した。

## References

- [Issue #65 問題文の文字が欠ける](https://github.com/tkumata/yomitore/issues/65)
- [Ratatui #2679](https://github.com/ratatui/ratatui/issues/2679)
- [技術仕様書 §3.2 文章生成機能](../../current/SPECIFICATIONS.md#32-文章生成機能-api_clientrs)
- [技術仕様書 §3.6 UI レンダリング](../../current/SPECIFICATIONS.md#36-ui-レンダリング-uirs-tuirs)
