# Box（ファイル一覧）の接続手順

対象の EveryDB 種別: **Box (ファイル一覧)**

## 概要

Box の**カスタムアプリ**を作り、**Client Credentials Grant（CCG）**でサービスアカウントとして
認証し、フォルダ配下のファイル一覧を取得します。アプリは Box 管理者による**承認**が必要です。

## 事前に必要なもの

- Box の開発者アカウントと、企業（Enterprise）の管理者による承認
- Enterprise ID（Box 管理コンソールの「アカウントと請求」等で確認できます）

## 手順

1. 入力欄の横の **🔑 取得…** を押して [Box Developer Console](https://app.box.com/developers/console) を開きます。
2. **アプリの新規作成** → **カスタムアプリ** を選び、認証方式に
   **サーバー認証（クライアント資格情報許可）** を選んで作成します。
3. 作成したアプリの **構成** タブで次を設定します。
   - **アプリケーションスコープ**: 「Box に格納されているすべてのファイルとフォルダの読み取り」を有効化
   - **アプリアクセスレベル**: 「アプリ + Enterprise アクセス」
4. 同じ画面の **OAuth 2.0 資格情報** から **クライアント ID** と **クライアント シークレット** をコピーします。
5. **承認** タブで **アプリの承認を申請** を押し、Box 管理者に承認してもらいます
   （管理者は管理コンソールの「アプリ」→「カスタムアプリマネージャー」で承認します）。
   **承認前は認証に失敗します。**
6. EveryDB に手順 4 の値と Enterprise ID を入力して保存します。

## EveryDB 側の入力項目

| 項目 | 入力する値 | 備考 |
| :--- | :--- | :--- |
| クライアントID | 手順 4 の Client ID | |
| クライアントシークレット | 手順 4 の Client Secret | マスターパスワードで保護されます |
| Enterprise ID | 企業アカウントの ID（数字） | |
| 開始フォルダID(任意) | 走査を開始するフォルダ ID | 空欄なら `0`（ルート） |

フォルダ ID は、Box でフォルダを開いたときの URL `https://app.box.com/folder/123456789` の数字部分です。

## よくあるエラーと対処

| 症状 | 原因 | 対処 |
| :--- | :--- | :--- |
| `Box トークン取得失敗 400` | Enterprise ID の誤り、CCG 未設定 | 手順 2〜3 を確認 |
| `Box トークン取得失敗 401` | Client ID / Secret の誤り | 手順 4 を貼り直す |
| `403 access_denied_insufficient_permissions` | アプリが未承認・スコープ不足 | 手順 3・5 を実施 |
| 0 件 | サービスアカウントにフォルダが共有されていない | 対象フォルダをアプリのサービスアカウントに共有 |

## 参考リンク

- [Box Developer Console](https://app.box.com/developers/console)
- [Client Credentials Grant](https://developer.box.com/guides/authentication/client-credentials/)
