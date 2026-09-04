# gRPC API

Kasane は gRPC（[tonic](https://github.com/hyperium/tonic)）のみで通信します。ネイティブ gRPC
クライアント（HTTP/2）とブラウザの grpc-web クライアント（HTTP/1.1）の両方を同じポートで受け付けます。

## proto 定義

`proto/*.proto` にリソース単位でサービスを定義しています。パッケージ名は `kasane` です。

| ファイル | サービス | 内容 |
| --- | --- | --- |
| `system.proto` | `SystemService` | サーバー情報の取得 |
| `auth.proto` | `AuthService` | ログイン（認証不要） |
| `database.proto` | `DatabaseService` | データベースの CRUD・コピー |
| `table.proto` | `TableService` | テーブルの CRUD・コピー |
| `data.proto` | `DataService` | 空間IDと値の検索（Server Streaming）・挿入・更新・削除 |
| `query.proto` | `QueryService` | クロステーブルのクエリ式実行（Server Streaming） |
| `users.proto` | `UserService` | ユーザーと権限の管理 |
| `common.proto` | - | `SpatialId` / `TableConstraints` 等、リソース横断の共有型 |

`grpc.health.v1.Health`（[`tonic-health`](https://docs.rs/tonic-health)）と
`grpc.reflection.v1.ServerReflection`（[`tonic-reflection`](https://docs.rs/tonic-reflection)）
も同じサーバーで提供します。認証は不要です。

## 認証

`AuthService.Login` で発行した JWT を、以後の呼び出しのメタデータ `authorization` に
`Bearer <token>` の形式で付与します（`AuthService` 自身と `Health`/`ServerReflection` を除く
全サービスが必須）。詳細は [permissions.md](permissions.md) を参照してください。

```bash
TOKEN=$(grpcurl -plaintext -d '{"username":"root","password":"password"}' \
  localhost:5172 kasane.AuthService/Login | jq -r .token)

grpcurl -plaintext -H "authorization: Bearer $TOKEN" \
  -d '{}' localhost:5172 kasane.DatabaseService/List
```

## エラー

失敗は `tonic::Status` として返ります。gRPC の標準コードに加え、
`google.rpc.ErrorInfo`（`reason` フィールド、`domain` は `"kasane"`）に、クライアントが
分岐に使える安定した機械可読コードを載せます。コード一覧は [permissions.md](permissions.md)
を参照してください。

## ブラウザから呼ぶ（grpc-web）

サーバーは [`tonic-web`](https://docs.rs/tonic-web) を通して grpc-web プロトコルを
同じポートで受け付けます。ブラウザからは [`@connectrpc/connect-web`](https://connectrpc.com/docs/web/getting-started)
や [`grpc-web`](https://github.com/grpc/grpc-web) のクライアントで直接呼び出せます
（別途プロキシは不要です）。

許可オリジンは `KASANE_CORS_ALLOWED_ORIGINS`（カンマ区切り）で絞れます。未設定時は全オリジンを
許可します（Bearer トークン方式で Cookie を使わないため、絞らなくてもただちに悪用できるわけ
ではありません）。

`DataService.Search` および `QueryService.Execute` の Server Streaming は gRPC-Web でも完全にサポートされており、ブラウザ側で `for await (const chunk of client.search(req))` のようにストリーミング受信が可能です（大量データ取得時もブラウザ側のメモリ消費を抑えられます）。

## 動的に API を探索する

`grpc.reflection.v1.ServerReflection` が有効なので、`.proto` を手元に持たなくても
[grpcurl](https://github.com/fullstorydev/grpcurl) や [Buf Studio](https://buf.build/studio)、
Postman の gRPC クライアントなどからサービス・メッセージの定義を動的に取得できます。

```bash
# サービス一覧
grpcurl -plaintext localhost:5172 list

# 1 サービスのメソッド一覧
grpcurl -plaintext localhost:5172 list kasane.DatabaseService

# メッセージ定義
grpcurl -plaintext localhost:5172 describe kasane.CreateDatabaseRequest
```
