# EveryDB 開発資料目録

このディレクトリは、開発者本人の計画・判断・作業記録を管理します。利用者と公開開発者向けの説明は `README.md` と `docs/` を正本にします。

## 現在の資料

- [共通開発ルール適用記録](testing/20260907-rule-application-imp.md): 2026-09-07 の配置・ルール統合作業と検証結果
- [将来案](roadmap/PLAN_IDEAS.md): 今後検討する機能
- [不採用・保留案](roadmap/reject.md): 採用しなかった案と理由

版別要求・計画・進捗・報告は、進行中・未来・完了済みを問わず `history/vX.Y.Z/` に置きます。v0.1.1の共通インストーラー対応と公開は完了しました。

## 版別資料

- [v0.0.1](history/v0.0.1/)
- [v0.0.2](history/v0.0.2/)
- [v0.0.3](history/v0.0.3/)
- [v0.0.4](history/v0.0.4/)
- [v0.0.5](history/v0.0.5/)
- [v0.0.6](history/v0.0.6/)
- [v0.1.0](history/v0.1.0/)
- [v0.1.1](history/v0.1.1/)
- [v0.1.2](history/v0.1.2/)（候補：[フロントエンドのリファクタリング](history/v0.1.2/v0.1.2-refact-imp.md)）

過去資料を確認するときは対象版だけを読み、後続版の完了記録とユーザー確認も照合してください。

## 運用スクリプト

| スクリプト | 用途 |
| :--- | :--- |
| `scripts/run-debug-EveryDB-all.ps1` | Debug 構成をビルドして通常起動 |
| `scripts/run-debug-EveryDB-all-sample.ps1` | ローカルサンプルを生成・登録して Debug 起動 |
| `scripts/run-release-EveryDB-all.ps1` | バンドルを作らず Release 構成をビルドして起動 |
| `scripts/run-exe-EveryDB-all.ps1` | 既存の Release 実行ファイルをビルドせず起動 |
| `scripts/build-release-EveryDB-kotorintoys.ps1` | 共通配布用のネイティブReleaseをビルド（導入・公開なし） |
| `scripts/build-debug-EveryDB-kotorintoys.ps1` | KotorinToys管理連携版を専用Debug出力先へビルド |
| `scripts/generate-EveryDB-access-sample.ps1` | Access 用ローカル試験データを生成する補助処理 |
| `scripts/import-EveryDB-development-environment.ps1` | C:\AI\env\EveryDB\development.env を値の評価・表示なしで読み込む |
| `scripts/test-development-environment.ps1` | 環境ファイルの形式・原子性・値を実行しない境界を検証 |
| `scripts/test-script-naming.ps1` | 運用スクリプトの命名を自動検査 |

配布物の生成・公開は [配布手順](RELEASE_RULE.md) に従います。

## 非公開・一時領域

- 開発用秘密情報は [外部保管の手順](../docs/development/secrets.md) に従い `C:\AI\env\EveryDB\` へ保存します。
- `private/`: その他の非公開資料。Git、公開成果物、通常の調査・静的検査から除外します。
- `temp/`: プロジェクト外の一時制作物。Git、公開成果物、目録、通常の調査・静的検査から除外します。

両ディレクトリの個別資料は目録へ載せず、ユーザーが明示した場合だけ読みます。
