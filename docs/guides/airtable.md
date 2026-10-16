# Airtable の接続手順

対象のデータソース種別: **Airtable**

## 概要

Airtable の **Personal Access Token**（個人用アクセストークン）を取得する手順を説明します。

## 事前に必要なもの

- Airtable アカウントと、対象ベースへのアクセス権

## 手順

1. [Personal access tokens](https://airtable.com/create/tokens) を開きます。
2. **Create new token** を押します。
   ![alt text](../assets/airtable/token-create.png)
3. 、名前（例: `EveryDB`）を入力します。
4. **Access** で、 **Add a base** から 取り込みたい**ベース**を追加します（ここで選んだベースだけが読めます）。
   ![alt text](../assets/airtable/base-access.png)
5. **Scopes** に `data.records:read` と `schema.bases:read` を追加します。
6. **Create token** を押します。
   ![alt text](../assets/airtable/token-scopes.png)
7. 表示された `pat...` で始まるトークンをコピーします
   （**この画面を閉じると再表示できないため、閉じてしまった場合は後ほど再発行となります**）。
   ![alt text](../assets/airtable/token-copy.png)
8. [Airtableのワークスペースページ](https://airtable.com/workspaces)から取り込みたいベースをブラウザで開き、URLをコピーして本ツールの自動入力欄に設定します。
   `https://airtable.com/appXXXXXXXX/tblYYYYYYYY/viwZZZZZZZZ`
   または、URLから以下のIDを控えて本ツールの入力欄に設定します。

- `appXXXXXXXX` … Base ID
- `tblYYYYYYYY` … テーブル ID（テーブル名でも可）
- `viwZZZZZZZZ` … ビュー ID（任意）
  ![alt text](../assets/airtable/base-url.png)

## EveryDB での安全な登録方法

秘密の Personal Access Token は接続設定に直接保存しません。

1. **設定** → **🔑 保存済みトークンの管理…** を開き、種別 **Airtable** のプロファイルを新規作成します。
2. 任意のラベルと、手順 7 でコピーした `pat...` トークンを入力して保存します。
3. Airtable 接続の追加・編集画面でトークン欄の **選択…** を押し、保存したプロファイルを選びます。
## EveryDB 側の入力項目

| 項目                   | 入力する値                | 備考                                         |
| :--------------------- | :------------------------ | :------------------------------------------- |
| 保存済みトークンプロファイル | 手順 7 の `pat...` を含む Airtable プロファイル | 接続側では **選択…** で参照します |
| Base ID                | `app...`                  |                                              |
| テーブル ID または名前 | `tbl...` またはテーブル名 |                                              |
| ビュー(任意)           | `viw...` またはビュー名   | 指定するとそのビューの絞り込み・並び順で取得 |

## よくあるエラーと対処

| 症状                 | 原因                                 | 対処                           |
| :------------------- | :----------------------------------- | :----------------------------- |
| `401 Unauthorized`   | トークンが誤り・失効                 | 手順 1〜5 でトークンを作り直す |
| `403 NOT_AUTHORIZED` | トークンの Access に対象ベースが無い | 手順 4 でベースを追加          |
| `404 NOT_FOUND`      | Base ID / テーブル名の誤り           | 手順 6 の URL から取り直す     |

## 参考リンク

- [Airtable Web API](https://airtable.com/developers/web/api/introduction)
- [Personal access tokens](https://airtable.com/create/tokens)
