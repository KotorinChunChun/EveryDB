# Microsoft 365 / 個人 OneDrive（Graph API）接続方法の選択

対象の EveryDB 種別: **EntraID ユーザー / グループ / デバイス一覧、Intune デバイス一覧、
Microsoft 365 ライセンス一覧、Teams チーム一覧、SharePoint リスト / ドキュメントライブラリ、
Outlook 予定表、OneDrive（組織）、Teams タスク & Planner、OneDrive（個人）**

## 目的

Microsoft Graph の保存済みトークンを作成する前に、まずアカウントの種類を選びます。
会社・学校の Microsoft 365 は **Microsoft Entra ID（組織）**、Outlook.com / Hotmail / Live などの
個人向け OneDrive は **Microsoft アカウント（個人）**を使用します。両者の資格情報と
OneDrive データソースは相互利用できません。

## アカウントとトークン種別

| 利用対象 | EveryDB のトークン種別 | データソース | 設定手順 |
| :--- | :--- | :--- | :--- |
| 会社・学校の Microsoft 365 | Microsoft Entra ID（組織） | OneDrive（組織）を含む Microsoft Graph 各種 | 下記の app-only または組織向け委任 OAuth |
| 個人 Microsoft アカウント | Microsoft アカウント（個人） | OneDrive（個人）のみ | [個人向け OneDrive の設定手順](microsoft365-personal.md) |

既存の **Microsoft 365 (Graph)** プロファイルと OneDrive 接続は、内部 ID や保存内容を変えず
組織用として引き続き利用できます。既存データの移行操作は不要です。

## 組織向けの2つの認証方式

| 比較項目 | アプリケーション認証（app-only） | ユーザー委任認証（OAuth） |
| :--- | :--- | :--- |
| 認証する主体 | Entra ID に登録したアプリ | Microsoft にサインインしたユーザー |
| 認証の手順 | テナント ID、クライアント ID、クライアントシークレットを登録 | テナント ID、クライアント ID を登録し、Microsoft の認可画面で同意 |
| ブラウザでのログイン | 不要 | 初回認可時と再認可時に必要 |
| クライアントシークレット | 必要。値と有効期限の安全な管理が必要 | 不要 |
| 管理者の同意 | 必要 | 組織ポリシーや選択した権限によって必要 |
| 取得できる範囲 | アプリに許可されたテナント範囲 | サインインユーザー本人がアクセスできる範囲 |
| EveryDB の対応範囲 | すべての組織向け Microsoft Graph データソース | SharePoint リスト / ライブラリ、本人の Outlook 予定表、本人の OneDrive（組織） |
| 向いている用途 | 無人の定期実行、組織全体の管理データ | 本人の権限範囲、クライアントシークレットを保存したくない場合 |

委任 OAuth で SharePoint の **サイト・チームから選択**を使う場合、サイトは本人の権限で検索し、
Teams は本人が直接参加しているチームだけを取得します。Teams チーム一覧データソース自体は app-only 専用です。

## 組織向け認証方式を選ぶ

### アプリケーション認証を選ぶ場合

次のいずれかに当てはまる場合はこちらを選びます。

- Entra ID、Intune、ライセンス、Teams チーム一覧、Planner を取り込みたい
- ユーザーがログインしていない状態でも定期リロードしたい
- 組織全体のデータを、管理者が許可した範囲で取得したい
- Outlook 予定表または OneDrive（組織）で、本人以外の対象ユーザーを UPN で指定したい

[アプリケーション認証の設定手順を開く](microsoft365-app-only.md)

### ユーザー委任認証を選ぶ場合

次の条件に合う場合はこちらを選びます。

- SharePoint リストまたはドキュメントライブラリを取り込みたい
- サインインした本人の Outlook 予定表または OneDrive（組織）を取り込みたい
- 本人が閲覧できる範囲だけを対象にしたい
- クライアントシークレットを作成・保存したくない

[委任 OAuth の設定手順を開く](microsoft365-delegated.md)

### 個人 Microsoft アカウントを選ぶ場合

個人契約の OneDrive を取り込む場合は、組織向け認証方式を選ばず、
[OneDrive（個人）の接続手順](microsoft365-personal.md)へ進みます。

## EveryDB での組織向け設定方法

1. **設定** → **🔑 保存済みトークンの管理…** を開きます。
2. 種別で **Microsoft Entra ID（組織）** を選びます。
3. **認証方式**で利用する方式を選びます。
4. 委任 OAuth の場合は、利用する機能を選んで Microsoft の認可を実行します。
5. **手順書**を押すと、現在選択している認証方式の個別ページが開きます。
6. 個別ページに従って Entra ID と EveryDB を設定し、プロファイルを保存します。

委任 OAuth の機能を追加・削除すると、保存済みの認可は解除されます。
必要な権限を Entra ID に追加してから **Microsoft にログインして認可…** を再実行してください。

接続設定の資格情報選択では、対象データソースと認証方式・機能が合わないプロファイルも
理由付きで表示されますが選択できません。既に接続から参照されているプロファイルを非対応の方式へ
変更する操作も拒否されます。用途ごとにプロファイルを分けると管理しやすくなります。

## 委任 OAuth の機能と権限

すべての委任プロファイルで `User.Read` と `offline_access` を要求します。
それに加え、EveryDB で選択した機能に応じて次の最小権限を要求します。

| EveryDB で選ぶ機能 | 追加される委任されたアクセス許可 | 対応データソース |
| :--- | :--- | :--- |
| SharePoint サイト・リスト / Teams選択 | `Sites.Read.All`, `Team.ReadBasic.All` | SharePoint リスト / ライブラリ、サイト・Teams 選択 |
| 本人の Outlook 予定表 | `Calendars.ReadBasic` | Outlook 予定表（サインインした本人） |
| 本人の OneDrive | `Files.Read` | OneDrive（組織、サインインした本人） |

既存の委任 OAuth プロファイルに機能情報が無い場合は、後方互換のため
**SharePoint サイト・リスト / Teams選択**用として扱われます。認証方式情報が無い既存 Graph プロファイルは
app-only として扱われます。

## 注意事項

- **アプリケーションの許可**と**委任されたアクセス許可**は別物です。同名の権限でも追加先を間違えると認証できません。
- 組織向け委任 OAuth の Outlook / OneDrive（組織）はサインインした本人だけが対象です。UPN 入力は使いません。
- OneDrive（個人）は Microsoft アカウント（個人）専用です。組織用資格情報との相互利用はできません。
- 委任 OAuth の Teams 選択は `/me/joinedTeams` を利用します。直接参加していないチームや、共有チャネルのホスト Team は表示されないことがあります。
- クライアントシークレット、リフレッシュトークン、アクセストークンを手順書やスクリーンショットへ記録しないでください。
- 組織の条件付きアクセスや管理者同意ポリシーにより、追加の承認が必要な場合があります。

## 関連ページ

- [アプリケーション認証の設定手順](microsoft365-app-only.md)
- [組織向け委任 OAuth の設定手順](microsoft365-delegated.md)
- [OneDrive（個人）の設定手順](microsoft365-personal.md)
- [Microsoft Graph のアクセス許可リファレンス](https://learn.microsoft.com/graph/permissions-reference)
