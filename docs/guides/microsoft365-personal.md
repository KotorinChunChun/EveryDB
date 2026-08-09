# OneDrive（個人 Microsoft アカウント）の接続手順

対象の EveryDB 種別: **OneDrive（個人）**

> この手順は Outlook.com / Hotmail / Live などの **個人 Microsoft アカウント**専用です。
> 会社・学校の Microsoft 365 は [Microsoft Graph 接続方法の選択](microsoft365.md) から
> 組織向けの手順を選んでください。

## 概要

EveryDB は利用者が登録したパブリッククライアントを使い、Authorization Code + PKCE で
個人 Microsoft アカウントへサインインします。認可先は個人アカウント専用の `consumers` に固定され、
サインイン本人の OneDrive を `/me/drive` から読み取り専用で取得します。

- EveryDB のトークン種別: **Microsoft アカウント（個人）**
- EveryDB のデータソース: **OneDrive（個人）**
- 必要な入力: **クライアント ID**のみ
- 使用する委任権限: `Files.Read`, `User.Read`, `offline_access`
- テナント ID、クライアントシークレット、対象ユーザー UPN は使用しません。

## 1. 個人アカウント対応アプリを登録する

1. [Microsoft Entra 管理センターのアプリ登録](https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade) を開きます。
2. **新規登録**を押し、名前（例: `EveryDB Personal OneDrive`）を入力します。
3. **サポートされているアカウントの種類**で
   **個人用 Microsoft アカウントのみ**（Personal Microsoft accounts only）を選びます。
4. **リダイレクト URI**のプラットフォームで
   **パブリック クライアント/ネイティブ（モバイル & デスクトップ）**を選び、
   `http://localhost:58914/graph-callback` を正確に登録します。
5. **登録**を押し、概要画面の **アプリケーション (クライアント) ID** を控えます。

既存アプリを使う場合は、マニフェストの `signInAudience` が
`PersonalMicrosoftAccount` または個人 Microsoft アカウントを含む設定であることを確認してください。
EveryDB は `consumers` authority を使うため、組織アカウント専用アプリでは認可できません。

## 2. 委任されたアクセス許可を設定する

1. 登録したアプリの **API のアクセス許可**を開きます。
2. **アクセス許可の追加** → **Microsoft Graph** → **委任されたアクセス許可**を開きます。
3. 次の権限を追加します。

   | 権限 | 用途 |
   | :--- | :--- |
   | `Files.Read` | サインイン本人の OneDrive ファイルを読み取る |
   | `User.Read` | サインインユーザーを確認する |
   | `offline_access` | リフレッシュトークンを取得する |

EveryDB は書き込み権限やアプリケーションの許可を要求しません。

## 3. パブリック クライアント フローを有効にする

1. アプリ登録の **認証**（Authentication）を開きます。
2. **詳細設定**の **パブリック クライアント フローを許可する**を **はい**にします。
3. **保存**を押します。

項目が見つからない場合はマニフェストで `allowPublicClient` を `true` に設定します。
クライアントシークレットは作成しません。

## 4. EveryDB で認可する

1. **設定** → **🔑 保存済みトークンの管理…** を開きます。
2. 種別 **Microsoft アカウント（個人）** のプロファイルを新規作成します。
3. 任意のラベルと、手順1で控えた **クライアント ID** を入力します。
4. **🔗 Microsoft にログインして認可…** を押します。
5. 取り込む OneDrive を所有する個人 Microsoft アカウントでサインインし、表示された許可へ同意します。
6. **✅ 認可済み**を確認して保存します。

リフレッシュトークンは Windows DPAPI で暗号化して保存されます。認可を取り消した場合や
トークンが失効した場合は、この手順をもう一度実行してください。

## 5. OneDrive（個人）を追加する

1. 接続設定のデータソース一覧で **＋追加**を押し、**OneDrive（個人）**を選びます。
2. 認証情報の **選択…**から、手順4で保存した **Microsoft アカウント（個人）**プロファイルを選びます。
3. 保存してリロードし、サインイン本人のファイルが表示されることを確認します。

組織用の **Microsoft Entra ID（組織）**プロファイルは選択できません。逆に、個人用プロファイルを
**OneDrive（組織）**や他の組織向け Graph データソースへ使用することもできません。

## よくあるエラーと対処

| 症状 | 原因 | 対処 |
| :--- | :--- | :--- |
| `redirect_uri_mismatch` | Redirect URI の未登録・入力ミス | `http://localhost:58914/graph-callback` と完全一致させる |
| 個人アカウントでサインインできない | アプリの対応アカウントが組織専用 | 対応アカウントを「個人用 Microsoft アカウントのみ」にする |
| `AADSTS700016` | クライアント ID が誤っている | アプリ概要のアプリケーション (クライアント) ID を確認する |
| OneDrive（個人）を選択できない | 組織用プロファイルを選んでいる | Microsoft アカウント（個人）プロファイルを作成・選択する |
| 認可後の取得に失敗する | `Files.Read` 不足、またはトークン失効 | 権限を確認し、Microsoft の認可をやり直す |

## ライブ確認チェックリスト

- アプリが個人 Microsoft アカウントをサポートしている。
- Redirect URI とパブリッククライアントフローが設定されている。
- `Files.Read`, `User.Read`, `offline_access` が委任されたアクセス許可として登録されている。
- Microsoft アカウント（個人）の認可・保存が成功する。
- OneDrive（個人）の初回取得と再リロードが成功する。

## 参考リンク

- [Microsoft Entra ID にアプリを登録する](https://learn.microsoft.com/entra/identity-platform/quickstart-register-app)
- [サポートするアカウントの種類](https://learn.microsoft.com/entra/identity-platform/v2-supported-account-types)
- [Microsoft Graph のアクセス許可リファレンス](https://learn.microsoft.com/graph/permissions-reference)
- [組織向け Microsoft Graph 接続方法の選択](microsoft365.md)
