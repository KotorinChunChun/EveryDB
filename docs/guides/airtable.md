# Airtable の接続手順

対象のデータソース種別: **Airtable**

## 概要

Airtable の **Personal Access Token**（個人用アクセストークン）を取得する手順を説明します。

## 事前に必要なもの

- Airtable アカウントと、対象ベースへのアクセス権

## 手順

1. 入力欄の横の **🔑 取得…** を押して [Personal access tokens](https://airtable.com/create/tokens) を開きます。
2. **Create new token** を押します。
   ![alt text](image.png)
3. 、名前（例: `EveryDB`）を入力します。
4. **Access** で、 **Add a base** から 取り込みたい**ベース**を追加します（ここで選んだベースだけが読めます）。
   ![alt text](image-1.png)
5. **Scopes** に `data.records:read` と `schema.bases:read` を追加します。
6. **Create token** を押します。
   ![alt text](image-2.png)
7. 表示された `pat...` で始まるトークンをコピーします
   （**この画面を閉じると再表示できないため、閉じてしまった場合は後ほど再発行となります**）。
   ![alt text](image-3.png)
8. [Airtableのワークスペースページ](https://airtable.com/workspaces)から取り込みたいベースをブラウザで開き、URLをコピーして本ツールの自動入力欄に設定します。
   `https://airtable.com/appXXXXXXXX/tblYYYYYYYY/viwZZZZZZZZ`
   または、URLから以下のIDを控えて本ツールの入力欄に設定します。

- `appXXXXXXXX` … Base ID
- `tblYYYYYYYY` … テーブル ID（テーブル名でも可）
- `viwZZZZZZZZ` … ビュー ID（任意）
  ![alt text](image-4.png)

## EveryDB 側の入力項目

| 項目                   | 入力する値                | 備考                                         |
| :--------------------- | :------------------------ | :------------------------------------------- |
| Personal Access Token  | 手順 5 の `pat...`        | Windows DPAPI により保護されます             |
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
