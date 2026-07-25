# PostgreSQL の接続手順

対象の EveryDB 種別: **PostgreSQL**

## 概要

PostgreSQL サーバーへ直接接続し、指定したテーブルの内容を取り込みます。
**読み取り専用のユーザー**を用意することを強く推奨します。

## 事前に必要なもの

- サーバーのホスト名 / ポート / データベース名
- 接続ユーザーとパスワード（対象テーブルへの `SELECT` 権限）
- ファイアウォールで EveryDB を動かす PC からの接続が許可されていること

## 手順

1. サーバー管理者に、読み取り専用ユーザーの作成を依頼します（例）。

```sql
CREATE ROLE everydb_ro LOGIN PASSWORD '（強いパスワード）';
GRANT CONNECT ON DATABASE app TO everydb_ro;
GRANT USAGE ON SCHEMA public TO everydb_ro;
GRANT SELECT ON ALL TABLES IN SCHEMA public TO everydb_ro;
```

2. リモート接続の場合は `postgresql.conf` の `listen_addresses` と `pg_hba.conf` の
   接続許可設定を確認します。
3. EveryDB に接続情報を入力し、**保存してリロード**を押します。
4. 同じサーバーの別テーブルを追加したいときは、接続先を右クリック →「接続設定」から
   テーブルを追加するのではなく、データソース一覧の **＋追加** で
   同じ接続情報を入力して追加してください（PostgreSQL はテーブル一覧の自動取得に未対応です）。

## EveryDB 側の入力項目

| 項目 | 入力する値 | 備考 |
| :--- | :--- | :--- |
| ホスト | `localhost` / `xxx.rds.amazonaws.com` 等 | |
| ポート | 既定 `5432` | 空欄可 |
| データベース名 | 接続先 DB 名 | |
| スキーマ(任意) | 既定 `public` | |
| テーブル名 | 取り込むテーブル | |
| ユーザー / パスワード | 読み取り専用ユーザー | パスワードは保護されます |
| SSLモード(任意) | `prefer` / `require` / `verify-full` / `disable` | インターネット越しなら `require` 以上 |
| 自己署名証明書を許可 | 社内サーバーで自己署名を使う場合のみ ON | |

## よくあるエラーと対処

| 症状 | 原因 | 対処 |
| :--- | :--- | :--- |
| `connection refused` | ホスト/ポート誤り、待ち受けていない | 手順 2 を確認 |
| `password authentication failed` | 認証情報の誤り | ユーザー名・パスワードを確認 |
| `no pg_hba.conf entry for host` | 接続元 IP が許可されていない | `pg_hba.conf` に接続元を追加 |
| `permission denied for table` | `SELECT` 権限が無い | 手順 1 の GRANT を実施 |
| 証明書エラー | 自己署名証明書 | 「自己署名証明書を許可」を ON（社内限定） |

## 参考リンク

- [PostgreSQL: クライアント認証](https://www.postgresql.jp/document/current/html/client-authentication.html)
