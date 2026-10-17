# KotorinToysとの連携

EveryDBは独立したGitリポジトリとして管理し、KotorinToysから開発版を起動できます。KotorinToysの配布用インストーラーやGitHub公開一覧には含めていません。

EveryDB単体のインストーラーは [EveryDB Releases](https://github.com/KotorinChunChun/EveryDB/releases) で配布します。これはKotorinToys本体が管理するインストーラーや本体の再リリースとは別です。

提供元は [EveryDB](https://github.com/KotorinChunChun/EveryDB) です。2026-10-17に公開前の履歴整理を行いました。それ以前のコピーをマージして戻さず、新規cloneまたは本体からのsubmodule取得を行ってください。

開発用秘密情報の保存先は `C:\AI\env\EveryDB\` です。[外部保管と読込み](development/secrets.md)に従って準備してください。

## 開発版のビルド

このビルドはフロントエンドを作成し、KotorinToys専用のRust出力先へ管理連携版を生成します。通常の `src-tauri/target/debug` と利用者用データは共有しません。

```powershell
cd C:\develop\KotorinToys\tools\EveryDB
pwsh -File .\dev\scripts\build-debug-EveryDB-kotorintoys.ps1
```

出力先は `src-tauri/target/kotorintoys/debug/everydb.exe` です。ビルド後はKotorinToysの一覧から起動できます。起動設定ではアプリをトレイに待機させ、本体からの表示要求を受けて画面を開きます。

## 管理連携

CurrentUserOnly相当のACLを持つユーザー専用名前付きパイプを使い、同一対話セッションと対象PIDを確認します。対応コマンドは `status`、`show`、`stop` です。停止は強制終了ではなくTauriの終了処理を呼び、バックグラウンド更新と認証処理の終了処理を実行します。

KotorinToysの管理定義は親側の `catalog/tools.json` からこのリポジトリの `tool.json` を参照します。`config/tools.json` は親のローカル開発用一覧にも同じ実行情報を持ちます。GitHubへのpushや公開一覧への反映は、別途親リポジトリを公開するときに行います。

EveryDBの通常データ保存先 `%APPDATA%\com.everydb.app\cache.db` とバックアップは、配置やビルド出力先の変更では移動しません。

統合テストではDebugビルドだけが環境変数 `EVERYDB_TEST_DATA_DIR` をデータベース保存先として使います。WebView2の試験プロファイルには `WEBVIEW2_USER_DATA_FOLDER` を指定し、通常利用のデータと分けます。この上書きはReleaseビルドには含まれません。
