# データソース追加手順

新しいデータソースは、少なくとも次を同じ変更で確認します。

1. `shared/source-types.json` へ表示名、カテゴリ、親種別、credentialスコープ、手順書を追加します。
2. `src/source-types/forms.ts` のフォーム定義へ入力項目を追加します。
3. `src-tauri/src/connectors.rs` にconnectorとfactory分岐を追加します。
4. 秘密フィールドがある場合は `secret.rs` と `credentials.rs` の保護・スコープ対応を追加します。
5. 親グルーピング、browse、テーブルを開く導線、ガイドを確認します。
6. `npm run docs:check`、フォーム/registryテスト、可能ならモックテストを追加します。
7. [データソース一覧](../data-sources.md) と対応する `docs/guides/` を更新します。

認証情報、実在するデータ、APIキーをテストfixture・コミット・スクリーンショットに含めないでください。
