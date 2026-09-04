# 権限管理と認証システム (Permissions & Authentication)

Kasane は JWT (JSON Web Token) による Bearer 認証と、スコープ付きのロールベースアクセス制御を提供します。

## 1. 認証 (Authentication)

パスワードのハッシュには `argon2` を使います。

### トークンの取得

`AuthService.Login` にクレデンシャルを送るとアクセストークン (JWT) が発行されます。

```bash
grpcurl -plaintext -d '{"username": "your_username", "password": "your_password"}' \
  localhost:5172 kasane.AuthService/Login
```

### API へのアクセス

発行されたトークンは gRPC メタデータの `authorization` に `Bearer <token>` の形式で付与します。

```bash
grpcurl -plaintext -H "authorization: Bearer eyJhb..." \
  -d '{}' localhost:5172 kasane.DatabaseService/List
```

### エラーレスポンスの形式

すべてのエラーは `tonic::Status` として返り、gRPC の標準コードに加えて
`google.rpc.ErrorInfo`（`reason` フィールド）にクライアントが分岐に使える安定した
機械可読コードを載せます。

| code | gRPCコード | 意味 |
| :--- | :--- | :--- |
| `missing_token` | `UNAUTHENTICATED` | `authorization` メタデータが無い |
| `malformed_header` | `UNAUTHENTICATED` | ヘッダが `Bearer <token>` 形式でない |
| `invalid_token` | `UNAUTHENTICATED` | 署名・期限などの検証に失敗 |
| `token_revoked` | `UNAUTHENTICATED` | トークンが失効済み（パスワード変更、ユーザー削除・再作成など） |
| `invalid_credentials` | `UNAUTHENTICATED` | ログインのユーザー名/パスワード不一致（存在有無は区別しない） |
| `requires_global_admin` | `PERMISSION_DENIED` | `global` / `admin` が必要（制御面の操作） |
| `requires_global_role` | `PERMISSION_DENIED` | `global` スコープで一定以上のロールが必要（`admin` 未満で足りる操作） |
| `not_self_or_admin` | `PERMISSION_DENIED` | 本人または `global` / `admin` のみ許可される操作 |
| `insufficient_privilege` | `PERMISSION_DENIED` | 対象データベース・テーブルへの権限不足 |
| `root_protected` | `PERMISSION_DENIED` | root ユーザーに対して許可されない操作 |
| `invalid_privilege` | `INVALID_ARGUMENT` | 権限ルールが不正（保持数の上限超過など） |
| `user_not_found` | `NOT_FOUND` | 指定した利用者が存在しない |
| `user_already_exists` | `ALREADY_EXISTS` | 同名の利用者が既に存在する |
| `privilege_not_found` | `NOT_FOUND` | 剥奪しようとした対象の権限を持っていない |

サーバー側の失敗は原因で 3 つに分かれます。混ぜないのは、運用時に「直すべき場所」を
区別できるようにするためです。

| code | gRPCコード | 意味 |
| :--- | :--- | :--- |
| `storage_error` | `INTERNAL` | ストレージエンジン自身が失敗した（I/O・競合・接続） |
| `corrupt_storage` | `INTERNAL` | 読めたバイト列が、書いたときの形式と違う |
| `schema_version_mismatch` | `INTERNAL` | ディスク形式の版がこのビルドと合わない |
| `internal_error` | `INTERNAL` | このプログラムの不変条件が破れた（バグ） |

## 2. スコープとロール (Scopes & Roles)

権限は **スコープ** と **ロール** の組で表します。ロールは上位が下位をすべて含みます。

| ロール | 強さ | 説明 |
| --- | --- | --- |
| **admin** | 4 | 制御面。ユーザーの作成・削除・権限の付与。**`global` スコープにのみ指定できます。** |
| **manage** | 3 | データベース・テーブルの作成・削除・更新といった構造の操作。 |
| **write** | 2 | データの書き込み（挿入・上書き・削除）。 |
| **read** | 1 | データとメタデータの読み取り。 |

スコープは 3 つあります。

| スコープ | 適用範囲 |
| --- | --- |
| `global` | サーバー全体。`admin` を指定できる唯一のスコープ。 |
| `database` | そのデータベースと、配下のテーブルすべて。 |
| `table` | そのテーブル 1 つだけ。 |

`database` / `table` スコープのロールは `read` / `write` / `manage` の 3 つで、**`admin` は型として表現できません**（リクエストのデシリアライズ時点で弾かれます）。

### 到達の規則

- `global` のロールは、あらゆるスコープの判定を満たします。
- `database` スコープの行は、そのデータベース自体の操作にも配下テーブルの操作にも効きます。
- `table` スコープの行は、**そのテーブルの操作にだけ**効きます。データベース全体の操作（改名など）には届きません。テーブル 1 つへの `manage` がデータベース全体への `manage` に化けないようにするためです。
- 「配下のどれかに届けば足りる」判定（データベースの存在確認・一覧）は、テーブル単位の行しか持たない利用者も通します。自分のテーブルへ辿り着く手段が無くなるためです。ただしこの経路は **`read` より上を決して満たしません**。

### 権限は名前ではなく ID に紐づく

保存されるのはデータベース ID・テーブル ID です。したがって

- **改名しても権限は追従します。**
- **削除して同名で作り直すと、旧権限は一致しません**（新しい ID が振られるため）。
- 対象を削除すると、それを指す権限は**その場で削除されます**。表示に残ることも、一覧の可視性に影響することもありません。

### 保持数の上限

1 ユーザーが保持できる `database` / `table` スコープの権限は最大 50,000 件です（`global` はスコープの性質上 1 件なので数えません）。超えると `invalid_privilege`（`INVALID_ARGUMENT`）になります。

### 操作ごとの必要権限

| 操作 | 必要な権限 |
| --- | --- |
| `DatabaseService.Create` / `Delete` / `Copy` | `global` / `manage` |
| `DatabaseService.Update` | 対象データベースへの `manage` |
| `DatabaseService.Get` | 配下のどれかに `read`（テーブル単位の権限でも可） |
| `DatabaseService.List` | 認証のみ（結果が権限で絞られます） |
| `TableService.Create` | 対象データベースへの `manage` |
| `TableService.List` | 配下のどれかに `read`（結果が権限で絞られます） |
| `TableService.Get` | 対象テーブルへの `read` |
| `TableService.Update` / `Delete` | 対象テーブルへの `manage` |
| `TableService.Copy` | 複製元テーブルへの `read` と、**複製先データベース**への `manage` |
| データ書き込み（`DataService.Insert` / `Upsert` / `Remove`） | 対象テーブルへの `write` |
| データ読み取り（`DataService.Search`, `QueryService.Execute`） | 参照するすべてのテーブルへの `read` |
| ユーザーの作成・削除・権限の付与剥奪 | `global` / `admin` |
| ユーザー情報・権限の参照、パスワード変更 | 本人 または `global` / `admin` |

### 「存在しない」より先に「権限が無い」を返す

権限のない利用者に `NOT_FOUND` を返すと、名前の存在有無を教えることになります。そのため認可は必ず存在確認より先に走り、権限がなければ対象の有無に関わらず `PERMISSION_DENIED` を返します。

## 3. ユーザーと権限の管理

### ユーザー管理

- `UserService.List`: 一覧（`global` / `admin`）。利用者名の辞書順で、既定 100 件・上限 1000 件。続きがあれば `next` に次の `after` が入ります。返るのは利用者名と `global` のロールだけで、個々の権限ルールは含みません。
- `UserService.Create`: 作成。`privileges` を指定すると作成と同時に付与できます。
- `UserService.Delete`: 削除（root は不可）
- `UserService.Get`: 情報の取得（本人 または `global` / `admin`）
- `UserService.UpdatePassword`: パスワードの更新（本人 または `global` / `admin`）

```bash
grpcurl -plaintext -H "authorization: Bearer $TOKEN" \
  -d '{"limit": 50, "after": "alice"}' localhost:5172 kasane.UserService/List
```

### 権限管理

権限の付与は `UserService.GrantPrivilege`、剥奪は `UserService.RevokePrivilege` で行います。剥奪はロールを問わず対象ごと落とすので、「`manage` を指定したが実際は `read` だったので何も消えなかった」は起きません。

- `UserService.GetPrivileges`: 保持する権限一覧の取得
- `UserService.GrantPrivilege`: 権限ルールの付与（既存ルールは置換）
- `UserService.RevokePrivilege`: 対象リソースに対する権限の剥奪

```bash
# テーブルへの権限付与
grpcurl -plaintext -H "authorization: Bearer $TOKEN" \
  -d '{"username": "alice", "privilege": {"table": {"db_name": "sensors", "table_name": "temperature", "role": "DATA_ROLE_WRITE"}}}' \
  localhost:5172 kasane.UserService/GrantPrivilege

# テーブルからの権限剥奪
grpcurl -plaintext -H "authorization: Bearer $TOKEN" \
  -d '{"username": "alice", "target": {"table": {"db_name": "sensors", "table_name": "temperature"}}}' \
  localhost:5172 kasane.UserService/RevokePrivilege
```

root の権限は変更できません（`root_protected`）。

## 4. 権限の保存のしかた

権限は利用者レコードとは別に、**対象ごとの独立した行**として持ちます。

```text
acl           : principal_id ‖ db_id ‖ table_slot  -> ロール 1 バイト
acl_by_object : db_id ‖ table_slot ‖ principal_id  -> 値なし
```

`table_slot` が全ゼロならデータベーススコープの行です（UUIDv7 は全ゼロにならないので、実在するテーブルとぶつかりません）。`global` のロールは利用者ごとに高々 1 つなので、行にせず利用者レコードへ埋め込みます。

この形にしている理由:

- **認証のコストが保持権限数に依存しない。** 毎リクエスト読むのは利用者レコード 1 件だけです。
- **認可が定数回の参照で決まる。** 判定に要る行はスコープから決まるので、保持数を舐める必要がありません。`table` スコープの 2 行は 1 回の一括取得に束ねます。
- **付与・剥奪が 1 行で済む。** 配列全体の読み直しと書き戻しが要りません。
- **一覧を権限の側から引ける。** 全データベースを列挙して捨てるのではなく、`acl` の自分の範囲を読むので、権限を持たない対象のコストを払いません。
- **対象の削除で権限を正確に落とせる。** `acl_by_object` がデータベース前置なので、データベースの削除は 1 プレフィックスでスコープ行と配下テーブルの行の両方を掴めます。

前置が**固定長 16 バイトの ID** なのは重要です。利用者名で前置すると、`alice` を前置にした範囲検索が `alicebob` の行まで拾ってしまいます。

なお、この識別子は将来グループを導入するときも同じ空間から採番できるよう `principal_id` と呼んでいます（現状は利用者と 1 対 1）。

## 5. 認可と対象解決を分けない

認可は**対象を解決したのと同じトランザクションの中**で行い、解決した ID をそのまま処理に使います。認可のためだけに読み取りを開くと、直後に同じ名前を引き直すことになり、名前の解決がネットワーク往復になるバックエンドではその往復が丸ごと二重になります。断面が分かれるぶん、判定した対象と操作した対象がずれる隙も生まれます。

キャッシュ層は持ちません。権限の変更は次のリクエストから確実に反映されます。

## 6. トークンの失効 (Token Revocation)

発行された JWT にはユーザーの UUID (`uid`) とトークン世代 (`ver`) が埋め込まれます。サーバーは認証時に、トークンの `uid` / `ver` が現在の値と一致するかを検証します。

- **パスワード変更**: トークン世代が進み、**それ以前に発行された全トークンが失効**します（再ログインが必要）。
- **ユーザーの削除・同名での再作成**: 再作成されたユーザーは新しい UUID を持つため、削除前に発行された旧トークンは `uid` 不一致で失効します。
- **権限の変更ではトークンを失効させません。** 認証ミドルウェアが毎リクエスト利用者レコードを読み直し、認可のたびに ACL を引くので、変更は次のリクエストから反映されます。

> `JWT_SECRET` 未設定時はサーバー再起動のたびに署名鍵が変わるため、発行済みトークンはすべて無効になります。本番環境では `JWT_SECRET` を明示的に設定してください。

## 7. ディスク形式の版

権限の保存形式は `schema_version` で管理しています（現在 **2**）。版 1（権限が利用者レコードの配列に同居していた形式）のデータを開こうとすると、**黙って読み替えずに起動を止めます**。移行は提供していないので、新しいディレクトリ／クラスタで作り直してください。

両バックエンドとも同じ `AppError::SchemaVersionMismatch` を返します（片方が panic、もう片方が `Result` という食い違いはありません）。
