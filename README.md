# Kasane
Kasaneは空間IDとそこに紐づいた値を管理するためのデータベースです

## 起動方法

`.env` を置くか、CLI 引数で指定して起動できます。

```bash
cargo run -- --database-path default_kasane_db --port 3000
```

`--database-path` が何を指すかはビルド時に選ばれたバックエンドによります（LMDB はデータ
ディレクトリ、TiKV は PD エンドポイントのカンマ区切り）。省略時は `.env` の値が使われます。

`.env` を使う場合は、`.env.example` をコピーしてください。**環境変数の一覧と既定値は
`.env.example` に集約してあります。**

```bash
cp .env.example .env
```

起動すると、実際に待ち受けている URL と接続先が表示されます。API は gRPC のみです。詳細は
[docs/grpc.md](docs/grpc.md) を参照してください。

## ログ

ログのレベルは `RUST_LOG` で設定します。未指定の場合は `.env.example` に載せてある既定が
使われます。`tracing-subscriber` の `EnvFilter` と同じフィルタ式を、カンマ区切りで並べられます。

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
のように指定できます。出力形式は `KASANE_LOG_FORMAT` で `plain` / `json` を選べます。
