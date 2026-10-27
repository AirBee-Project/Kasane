# Kasane Architecture

このドキュメントは、Kasane のレイヤー構造、依存の方向、各レイヤーの責務をまとめたものです。

## 基本方針

Kasane は「外側のレイヤーが内側のレイヤーを利用する」構造を取ります。依存の方向は常に内向きです。

```mermaid
flowchart TD
    Main[main.rs / lib.rs] --> Grpc[grpc]
    Grpc --> Service[services]
    Service --> Repository[repositories]
    Repository --> Storage[(LMDB / インメモリ)]

    Grpc --> Convert[grpc::convert*]
    Convert --> Model[models]
    Service --> Model
    Repository --> Model
    Service --> Error[error]
    Grpc --> Error
    Repository --> Error
    Grpc --> Kasane[Kasane]
```

## レイヤーの責務

### `main.rs` / `lib.rs`

- アプリケーション全体の起点。
- `lib.rs` は公開する入口の型 `Kasane` を定義する。中身はストレージのハンドルだけで、通信路は持たない。作成は server では `Kasane::open`（LMDB）、`default-features = false` では `Kasane::new`（インメモリ）で行う。
- `main.rs` は待ち受け（`TcpIncoming`）と HTTP のレイヤー（grpc-web、CORS、トレース）を用意し、`grpc::routes` を載せてサーバーを起動する。
- サーバーの起動以外の業務ロジックは持たない。

### `grpc`

- proto（`proto/*.proto`）から生成された gRPC サービス・メッセージ型（`grpc::pb`）と、各サービストレイトの実装（`grpc::database`, `grpc::table` 等）を置く。トレイトは `Kasane` を包む `grpc::Service` に実装し、`Kasane` のメソッドを呼ぶ。
- 入口は 2 つ。server では `grpc::routes` が全サービスを載せた `Routes` を返し、組み込みでは `grpc::handle` がメソッド名と protobuf のバイト列を受けて 1 リクエストを処理する。
- `grpc::convert*` が proto のメッセージ型とドメイン型（`models`）の相互変換を担う。
- proto とドメインには同じ名前の型が多い（`TableDataType`、`UserRole` など）。grpc 層では proto の型を常に `pb::` で修飾し、ドメインの型は修飾せずに使う。
- 認証は `grpc::auth`（JWT の検証、利用者レコードの読み直し、Login RPC）が集約して担う。
- ドメインロジックや DB 操作は持たない。

### `services`

- アプリケーションの処理手順を組み立てる。各操作は `Kasane` のメソッド（`create_database`、`search`、`execute_query` など）として実装し、ファイルは対象ごとに分ける。
- バリデーション、存在確認、作成、削除などのユースケースを表現する。
- トランザクションの開始と commit の責務を持つ。
- repository を呼び出して具体的な保存処理を行う。

### `repositories`

- ストレージへのアクセスを隠蔽する。server では LMDB（`repositories::lmdb`）、組み込みではインメモリ（`repositories::memory`）を使い、`repositories::Db` が feature で片方を指す。
- 利用者と ACL の書き込みは `UserRepository` に分けてあり、server のストレージだけが実装する。
- テーブルの open / get / insert / remove などの低レベル操作を担当する。
- 業務ルールは持たない。
- バックエンド固有の型や詳細を上位層に公開しない。
- データの整合性確保は services 層の責務である。

### `models`

- request / response / domain / entity の型定義を置く。
- 純粋なデータ構造を定義する。
- サービスや repository の実装詳細に依存しない。

### `error`

- アプリケーション全体で共通に扱うエラー（`AppError`, `AuthError`）を定義する。
- gRPC の `tonic::Status` および `google.rpc.ErrorInfo`（機械可読コード）への変換責務もここで持つ。
- 各層は `AppError` に寄せてエラーを返す。

## 依存ルール

### 許可される依存

- `grpc` -> `services`, `models`, `error`
- `services` -> `repositories`, `models`, `error`, `helpers`
- `repositories` -> `models`, `error`, ストレージエンジン
- `main.rs` / `lib.rs` -> `grpc`, `repositories`

### 禁止したい依存

- `repositories` から `services` や `grpc` を呼ぶこと
- `models` から上位レイヤーへ依存すること
- `grpc` に DB 操作を直接書くこと

## Table サービスの流れ

`TableService` 系の RPC は次の順序で処理されます。

1. `grpc::auth` が JWT を検証し、最新の利用者レコード（`User`）を読み直す。
2. `grpc::table` が proto のメッセージをドメイン型へ変換し、service を呼ぶ。
3. `services` がバリデーションとトランザクション管理を行う。
4. `repositories` がストレージ（LMDB またはインメモリ）を使って実データを読み書きする。
5. `error` が失敗を `tonic::Status` に変換する。

## 補足

- `create` は `TableService.Create`。
- `get` は `TableService.Get`。
- `delete` は `TableService.Delete`。
- 追加の CRUD が増えても、この依存方向は変えない。