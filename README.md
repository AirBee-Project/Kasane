# Kasane
Kasaneは空間IDとそこに紐づいた値を管理するためのデータベースです

## 起動方法

設定は環境変数（または `.env`）で渡します。

```bash
DATABASE_DIR=default_kasane_db PORT=3000 cargo run
```

`DATABASE_DIR` には LMDB のデータディレクトリを指定します。

`.env` を使う場合は、`.env.example` をコピーしてください。**環境変数の一覧と既定値は
`.env.example` に集約してあります。**

```bash
cp .env.example .env
```

起動すると、実際に待ち受けている URL と接続先が表示されます。API は gRPC のみです。詳細は
[docs/grpc.md](docs/grpc.md) を参照してください。

## 組み込み利用（インメモリ・root 固定）

`default-features = false` でビルドすると、ネットワーク待受や LMDB を使わず、プロセス内でインメモリデータベース（`Embedded`）として動かせます。利用者は root 固定となり、`AuthService` と `UserService` は提供されません。

```rust
use kasane::Embedded;

let db = Embedded::new();
// gRPC メソッド名と protobuf リクエストのバイト列を渡して実行
let responses = db.handle("/kasane.DatabaseService/List", &req_bytes)?;
```

`wasm32-unknown-unknown` ターゲットにも対応しているため、利用側で `wasm-bindgen` などのバインディングを用意することで WebAssembly 環境（ブラウザや Node.js 等）でも利用できます。

## ログ

ログのレベルは `RUST_LOG` で設定します。未指定の場合は `info` 以上を出します。`tracing-subscriber` の `EnvFilter` と同じフィルタ式を、カンマ区切りで並べられます。

- `debug` のような全体レベル指定
- `kasane=debug` のような対象別の指定
- `kasane::grpc=info` のようなモジュール階層の指定
- `kasane[request]=trace` のようなスパン名を含む指定
- `kasane[request{method}]=debug` のようなスパンのフィールド名を含む指定
- `kasane[request{method="GET"}]=debug` のようなフィールド値まで含めた指定
- `kasane=debug,tonic=info,tower_http=warn` のような複数指定

指定できるレベルは `error`、`warn`、`info`、`debug`、`trace`、`off` です。

たとえば、全体を詳しく見たい場合は `RUST_LOG=debug`、Kasane だけ詳細にしたい場合は
`RUST_LOG=kasane=debug`、gRPC 周りを少し抑えたい場合は `RUST_LOG=kasane=info,tower_http=warn`
のように指定できます。
