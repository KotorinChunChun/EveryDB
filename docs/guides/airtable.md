# Airtable の接続手順

対象の EveryDB 種別: **Airtable**

## 概要

Airtable の **Personal Access Token**（個人用アクセストークン）でベース内のテーブルを読み取ります。
トークンにはアクセスできるベースを明示的に指定します。

## 事前に必要なもの

- Airtable アカウントと、対象ベースへのアクセス権

## 手順

1. 入力欄の横の **🔑 取得…** を押して [Personal access tokens](https://airtable.com/create/tokens) を開きます。
2. **Create new token** を押し、名前（例: `EveryDB`）を入力します。
3. **Scopes** に `data.records:read` と `schema.bases:read` を追加します。
4. **Access** で、取り込みたい**ベース**を追加します（ここで選んだベースだけが読めます）。
5. **Create token** を押し、表示された `pat...` で始まるトークンをコピーします
   （**この画面を閉じると再表示できません**）。
6. 取り込みたいベースをブラウザで開き、URL から ID を控えます。
   `https://airtable.com/appXXXXXXXX/tblYYYYYYYY/viwZZZZZZZZ`
   - `appXXXXXXXX` … Base ID
   - `tblYYYYYYYY` … テーブル ID（テーブル名でも可）
   - `viwZZZZZZZZ` … ビュー ID（任意）

## EveryDB 側の入力項目

| 項目 | 入力する値 | 備考 |
| :--- | :--- | :--- |
| Personal Access Token | 手順 5 の `pat...` | マスターパスワードで保護されます |
| Base ID | `app...` | |
| テーブル ID または名前 | `tbl...` またはテーブル名 | |
| ビュー(任意) | `viw...` またはビュー名 | 指定するとそのビューの絞り込み・並び順で取得 |

## よくあるエラーと対処

| 症状 | 原因 | 対処 |
| :--- | :--- | :--- |
| `401 Unauthorized` | トークンが誤り・失効 | 手順 1〜5 でトークンを作り直す |
| `403 NOT_AUTHORIZED` | トークンの Access に対象ベースが無い | 手順 4 でベースを追加 |
| `404 NOT_FOUND` | Base ID / テーブル名の誤り | 手順 6 の URL から取り直す |

## 参考リンク

- [Airtable Web API](https://airtable.com/developers/web/api/introduction)
- [Personal access tokens](https://airtable.com/create/tokens)
