# EveryDB 開発者ガイド

EveryDB を自分でビルド・検証・改造する開発者向けの入口です。利用者向けの操作方法は [使い方](user-guide.md) を参照してください。

## 開発環境と品質確認

- [ビルド方法](development/building.md): 必要なツール、セットアップ、Debug／Release ビルド
- [KotorinToysとの連携](kotorintoys-integration.md): 独立Git、管理連携版のビルド、本体からの起動方法
- [テスト方法](development/testing.md): オフライン品質ゲート、ライブテスト、手動確認
- [アーキテクチャ](development/architecture.md): フロントエンド、Tauri、SQLite、コネクタの構成
- [データソース追加手順](development/adding-a-data-source.md): 新しい接続先を追加するときの変更箇所と検証

通常の変更では、リポジトリ直下から次の品質ゲートを実行します。

```powershell
npm ci
npm run check:all
```

アプリを起動する場合は、用途に応じて `dev/scripts/` の入口を使います。

```powershell
pwsh -File .\dev\scripts\run-debug-EveryDB-all.ps1
pwsh -File .\dev\scripts\run-debug-EveryDB-all-sample.ps1
pwsh -File .\dev\scripts\run-release-EveryDB-all.ps1
pwsh -File .\dev\scripts\run-exe-EveryDB-all.ps1
pwsh -File .\dev\scripts\build-debug-EveryDB-kotorintoys.ps1
```

配布物生成と外部公開は別工程です。リリース担当者は [配布手順](../dev/RELEASE_RULE.md) を確認してください。

## 資料とデータの扱い

- 公開可能な再現手順と仕様は `docs/` に置きます。
- 開発計画、判断、検証記録は `dev/` に置きます。
- 秘密情報や個人情報は `dev/private/`、プロジェクト外の一時制作物は `dev/temp/` に置き、Git へ追加しません。
- `samples/` はローカル生成の試験データ置き場で Git 対象外です。製品同梱リソースは `src-tauri/resources/`、共通試験データは `tests/fixtures/` または既存の固有テスト配置へ置きます。
