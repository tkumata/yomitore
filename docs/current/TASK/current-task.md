# Current Task: コードベース簡素化

## 状態

- Phase: 4
- Status: done
- Created: 2026-07-17 09:28

## 目的

監査で確認した過剰な抽象化、未使用コード、不要な依存関係を削除し、既存動作を維持する。

## フェーズ

1. ドキュメント同期
2. Rust コードと依存関係の簡素化
3. 検証フックの簡素化
4. 検証とコードレビュー

## 完了条件

- 監査で指定された10項目が反映されている
- 保存データ形式とユーザー向け動作が変わらない
- `bash -n .agent-hooks/verify_pipeline.sh` が成功する: done
- `make check` が成功する: done
- `make build` が成功する: done
- コードレビューが成功する: done
