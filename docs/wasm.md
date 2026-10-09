# Wasm 版

`wasm/` クレートは、Kasane をブラウザ（および Node.js）で動かすための Wasm パッケージです。
サーバーを立てずに、gRPC と同じ API で Kasane を使えます。

## 構成

- ストレージはメモリバックエンド（`backend-memory` feature）です。テーブルの中身は 1 テーブルにつき
  1 つの `SpatialIdMap` で持ちます。永続化はせず、インスタンスを捨てるとデータも消えます。
- サーバーと同じ gRPC のハンドラ（`grpc` feature）を、通信路を通さずに直接呼びます。待ち受けの一式
  （`server` feature）は含みません。
- 入出力は proto（`proto/*.proto`）のメッセージをエンコードしたバイト列です。クライアントは gRPC と同じ
  メッセージ型をそのまま使えます。
- すべての RPC を root として実行します。`AuthService`（ログイン）は扱いません。

| feature | 内容 |
| --- | --- |
| `grpc` | proto のメッセージ型、変換、各サービスのハンドラ |
| `server` | `grpc` に加えて、gRPC サーバーとして待ち受けるための一式 |

## ビルド

```bash
wasm-pack build wasm --target web --release
```

`wasm/pkg/` に JS のグルーコードと `.wasm` が出力されます。ローカルの `.cargo/config.toml` は dev
プロファイルで Cranelift を使い、Cranelift は wasm32 を出力できないため、`--release` を付けます。

テストは Node.js 上で実行します。

```bash
wasm-pack test --node --release wasm
```

## API

```ts
class Kasane {
  static open(): Promise<Kasane>;
  call(path: string, request: Uint8Array): Promise<Uint8Array[]>;
}
```

- `open()` は root だけを持つ空のインスタンスを作ります。
- `call(path, request)` は 1 回の RPC です。
  - `path` は gRPC のメソッドパス（`/kasane.DataService/Search` など）です。
  - `request` は要求メッセージをエンコードしたバイト列です。
  - 戻り値は応答メッセージをエンコードしたバイト列の配列です。単項 RPC なら 1 要素、サーバー
    ストリーミング RPC ならチャンクの数だけ並びます。

```ts
import init, { Kasane } from "./pkg/kasane_wasm.js";

await init();
const kasane = await Kasane.open();
const [bytes] = await kasane.call(
  "/kasane.DatabaseService/Create",
  toBinary(CreateDatabaseRequestSchema, create(CreateDatabaseRequestSchema, { name: "demo" })),
);
const info = fromBinary(DatabaseInfoSchema, bytes);
```

### エラー

失敗すると `Error` で reject します。次のプロパティを持ちます。

| プロパティ | 内容 |
| --- | --- |
| `code` | gRPC のステータスコード（数値）。サーバーが返すものと同じ |
| `message` | エラーメッセージ |
| `details` | `grpc-status-details-bin` と同じバイト列（`google.rpc.Status`） |

扱わないメソッド（`AuthService` など）は `Unimplemented`（12）になります。
