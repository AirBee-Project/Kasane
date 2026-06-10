# 権限管理と認証システム (Permissions & Authentication)

Kasaneはセキュアなデータ管理を実現するため、JWT (JSON Web Token) を利用したBearer認証と、MySQLのようなデータベース単位でのロールベースアクセス制御 (RBAC) を提供しています。

## 1. 認証 (Authentication)

Kasaneでは `argon2` を用いた堅牢なパスワードハッシュ機能を採用しています。

### トークンの取得
ユーザーは `/auth/login` エンドポイントに対し、クレデンシャル (ユーザー名とパスワード) を送信することで認証を行います。
認証が成功すると、アクセストークン (JWT) が発行されます。

```http
POST /auth/login
Content-Type: application/json

{
  "username": "your_username",
  "password": "your_password"
}
```

### APIへのアクセス
発行されたトークンは、その後のAPIリクエストにおいて `Authorization` ヘッダーに `Bearer <token>` の形式で付与する必要があります。

```http
GET /databases
Authorization: Bearer eyJhb...
```

## 2. ロールと権限 (Roles & Privileges)

システムは「データベース単位」での権限を管理しており、各ユーザーは対象データベースに対して以下の4つのいずれかのロールを持ちます。権限レベルは上位のものが下位の操作を包含します。

| ロール (Role) | 権限レベル | 説明 |
| --- | --- | --- |
| **GlobalAdmin** | 4 | システム全体に対する管理者権限。すべてのデータベースの作成・削除権限、全ユーザーの権限管理を行うことができます。 |
| **Manage** | 3 | 対象データベース内におけるテーブルの作成・削除、テーブル一覧の取得など、構造に関する操作を行うことができます。 |
| **Write** | 2 | 対象データベース内のテーブルに対するデータ書き込み（挿入、更新、削除）を行うことができます。 |
| **Read** | 1 | 対象データベース内のテーブルに格納されているデータを読み取る（検索・取得）ことができます。 |

> **GlobalAdmin について**: GlobalAdmin は「データベース単位のロール」ではなく、ユーザーに付与されるシステム全体のフラグ（`is_global_admin`）です。Read / Write / Manage はデータベースごとに `PUT /users/{username}/privileges/{db_name}` で付与しますが、**GlobalAdmin はこのエンドポイントでは付与できません**。ユーザー作成時 (`POST /users`) のフラグ、または `PUT /users/{username}/admin` で設定します。

### エンドポイントごとの必要権限例

- **データベース管理** (`POST /databases`, `DELETE /databases/{name}`)
  - **GlobalAdmin** が必要です。
- **データベース参照** (`GET /databases`, `GET /databases/{name}`)
  - `GET /databases/{name}` は対象データベースに対する **Read** 以上の権限が必要です。
  - `GET /databases` は **GlobalAdmin** ならすべてのデータベースを、一般ユーザーは自身が権限を持つデータベースのみを返します。
- **テーブル操作** (`POST /databases/{db_name}/tables`, `DELETE /databases/{db_name}/tables/{table_name}`)
  - 対象データベースに対する **Manage** 以上の権限が必要です。
- **データ書き込み** (`PUT /databases/{db_name}/tables/{table_name}/data` など)
  - 対象データベースに対する **Write** 以上の権限が必要です。
- **データ読み取り** (`POST /databases/{db_name}/tables/{table_name}/data/search`)
  - 対象データベースに対する **Read** 以上の権限が必要です。

## 3. ユーザーと権限の管理 (User & Privilege Management)

**GlobalAdmin** 権限を持つユーザーは、以下のRESTful APIエンドポイントを利用して、ユーザーの作成や権限の付与を行うことができます。

### ユーザー管理
- `GET /users`: ユーザー一覧の取得
- `POST /users`: 新規ユーザーの作成
- `DELETE /users/{username}`: ユーザーの削除
- `PUT /users/{username}/password`: パスワードの更新（本人または GlobalAdmin が実行可能）
- `PUT /users/{username}/admin`: GlobalAdmin 権限の付与・剥奪（root は変更不可）

### 権限管理
データベース単位の権限を付与・変更・削除するためのエンドポイントです。

- `GET /users/{username}/privileges`: 特定ユーザーが持つ全権限の一覧取得
- `PUT /users/{username}/privileges/{db_name}`: 特定データベースに対する権限（ロール）の設定・更新
- `DELETE /users/{username}/privileges/{db_name}`: 特定データベースに対する権限の剥奪

## 4. 高速化のためのキャッシュ機構
Kasaneは各リクエストごとのデータベース参照負荷を軽減するため、サーバー内に `AuthCache` 機構を持っています。
一度認証および権限チェックが行われたユーザーの情報はインメモリキャッシュ (`tokio::sync::RwLock` を利用) に保持されるため、シンプルさを保ったまま高速なAPIアクセスが可能です。

### キャッシュの安全対策
キャッシュには以下の制限が設けられており、長期運用時のメモリリークや権限変更の不整合（Stale Data）を防ぎます。
- **有効期限 (TTL)**: キャッシュされてから5分が経過すると自動的に無効化され、次回アクセス時に最新の権限をデータベースから取得し直します。
- **容量制限**: 最大10,000件のユーザー情報を保持します。上限に達した場合は、まず期限切れのエントリを削除し、それでも空きがなければ**古いものから順に追い出します**（全消去はしません）。
- **即時無効化**: ユーザーの削除・パスワード変更・GlobalAdmin 権限変更が行われると、対象ユーザーのキャッシュは即座に削除され、変更が次のリクエストから反映されます。

## 5. トークンの失効 (Token Revocation)
発行された JWT にはユーザーの UUID (`uid`) とトークン世代 (`ver`) が埋め込まれます。サーバーは認証時に、トークンの `uid`・`ver` が現在のユーザーの値と一致するかを検証します。

- **パスワード変更・GlobalAdmin 権限変更**: ユーザーのトークン世代がインクリメントされ、**それ以前に発行された全トークンが失効**します（再ログインが必要）。
- **ユーザーの削除・同名での再作成**: 再作成されたユーザーは新しい UUID を持つため、削除前に発行された旧トークンは `uid` 不一致で失効します。

> なお `JWT_SECRET` 未設定時はサーバー再起動のたびに署名鍵が変わるため、発行済みトークンはすべて無効になります。本番環境では `JWT_SECRET` を明示的に設定してください。
