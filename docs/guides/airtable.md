# Airtable の接続手順

対象のデータソース種別: **Airtable**

## 事前に必要なもの

- Airtable アカウントと対象ベースへのアクセス権
- 読み取り対象の Base ID、テーブル ID または名前、必要ならビュー ID

## Personal Access Token の作成

1. [Personal access tokens](https://airtable.com/create/tokens) を開きます。
2. **Create new token** を選び、名前（例: `EveryDB`）を入力します。
3. **Access** → **Add a base** で読み取り対象のベースだけを追加します。
4. **Scopes** に `data.records:read` と `schema.bases:read` を追加します。
5. **Create token** を選び、表示されたトークンを安全な場所へ保存します。

トークンは秘密情報です。画面共有、スクリーンショット、Git、問い合わせ本文へ含めないでください。作成画面を閉じて再表示できなくなった場合は、Airtableで再発行します。不要になったトークンはAirtable側で失効させます。

## 接続先の指定

[ワークスペース](https://airtable.com/workspaces)から対象のベースを開き、URLをEveryDBの自動入力欄へ貼り付けます。次は実在する接続先を示さない構造例です。

```text
https://airtable.com/appEXAMPLE/tblEXAMPLE/viwEXAMPLE
```

| URLの部分 | EveryDBの入力項目 |
| :--- | :--- |
| `appEXAMPLE` | Base ID |
| `tblEXAMPLE` | テーブル ID または名前 |
| `viwEXAMPLE` | ビュー ID（任意） |

## EveryDB への登録

1. **設定** → **保存済みトークンの管理**で、種別 **Airtable** のプロファイルを作成します。
2. 任意のラベルとトークンを入力して保存します。
3. Airtable接続の追加・編集画面で **選択** を押し、保存したプロファイルを選びます。
4. Base IDとテーブルを設定し、必要ならビューを指定して保存します。

開発・ライブ試験で使用する秘密値の置き場所は [開発用秘密情報](../development/secrets.md) を参照してください。利用者がアプリへ登録する認証情報の保存先・DPAPI保護は [セキュリティ](../security.md) のとおりです。

## よくあるエラーと対処

| 症状 | 確認する内容 |
| :--- | :--- |
| `401 Unauthorized` | トークンの誤りや失効。必要ならAirtable側で再発行します。 |
| `403 NOT_AUTHORIZED` | トークンのAccessに対象ベースが含まれるか確認します。 |
| `404 NOT_FOUND` | Base ID、テーブルID・名前、ビューIDを対象URLと照合します。 |

## 参考リンク

- [Airtable Web API](https://airtable.com/developers/web/api/introduction)
- [Personal access tokens](https://airtable.com/create/tokens)
