# Kasane
Kasaneは空間IDとそこに紐づいた値を管理するためのデータベースです

## 起動方法

`.env` を置くか、CLI 引数で指定して起動できます。

```bash
cargo run -- --database-path default.kasane --port 3000
```

`.env` を使う場合は、プロジェクトルートに次のように置きます。

```env
FILE=default.kasane
PORT=3000
LOG_MODE=kasane=info,tower_http=info
```

起動すると、実際に待ち受けている URL と使用中のデータベースファイル名が表示されます。
ログは `LOG_MODE` で設定できます。未指定の場合は `kasane=info,tower_http=info` が使われます。

`LOG_MODE` には、`tracing-subscriber` の `EnvFilter` と同じフィルタ式を指定できます。1つの `LOG_MODE` には、カンマ区切りで複数の指示を並べられます。

- `debug` のような全体レベル指定
- `kasane=debug` のような対象別の指定
- `kasane::handlers=info` のようなモジュール階層の指定
- `kasane[request]=trace` のようなスパン名を含む指定
- `kasane[request{method}]=debug` のようなスパンのフィールド名を含む指定
- `kasane[request{method="GET"}]=debug` のようなフィールド値まで含めた指定
- `kasane=debug,axum=info,tower_http=warn` のような複数指定

指定できるレベルは `error`、`warn`、`info`、`debug`、`trace`、`off` です。

たとえば、全体を詳しく見たい場合は `LOG_MODE=debug`、Kasane だけ詳細にしたい場合は `LOG_MODE=kasane=debug`、HTTP 周りを少し抑えたい場合は `LOG_MODE=kasane=info,tower_http=warn` のように指定できます。
