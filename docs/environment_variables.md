# 環境変数 (Environment Variables)

Kasaneでは、設定を環境変数で変更することができます。また、`.env` ファイルをプロジェクトルートに配置することで、環境変数を自動的に読み込みます。

以下は、利用可能なすべての環境変数のリストとその説明です。

## アプリケーション設定

| 変数名 | デフォルト値 | 説明 |
| :--- | :--- | :--- |
| `FILE` | `default.kasane` | データベースファイルのパスを指定します。コマンドライン引数 `--database-path` が指定されていない場合、この環境変数が使用されます。 |
| `PORT` | `5173` | サーバーがリッスンするポート番号を指定します。コマンドライン引数 `--port` が指定されていない場合、この環境変数が使用されます。(※ `.env.example` では `3000` と記載されています) |
| `LOG_MODE` | `kasane=info,tower_http=info` | ログの出力レベルを指定します。`tracing_subscriber::EnvFilter` の書式に従います。 |

## 認証・権限設定 (Authentication & Authorization)

KasaneはJWT（JSON Web Token）ベースの認証と、ロールベースアクセス制御（RBAC）をサポートしています。
以下の環境変数を設定することで、初回起動時のルートアカウントやJWTのセキュリティをカスタマイズできます。

| 変数名 | デフォルト値 | 説明 |
| :--- | :--- | :--- |
| `KASANE_ROOT_PASSWORD` | `password` | データベース初期化時に自動作成されるデフォルトの管理者（root）のパスワードです。 |
| `KASANE_JWT_SECRET` | `kasane-super-secret-key-change-me` | JWTトークンの署名・検証に使用されるシークレットキーです。**本番環境では必ず安全でユニークな文字列に変更してください。** |
