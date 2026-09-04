# テスト
Kasaneは求められる挙動を可能な限り、自動的なテストでカバーする。これは`/tests`フォルダの中に書く。単純なケースから複雑なケースまでをカバーする。

テストは何のケースを検証しているのかをコメントに必ず残すこと。また、こちらは自動的に実行されるテストなので、比較的複雑なパターンも網羅する。また、テストコードを見て何を書いているのかわかりやすくすること。

# DBフォーマット互換性テスト
LMDB・TiKVどちらも、ディスク／クラスタ上の形式が意図せず壊れていないかを`tests/fixtures/db_compat/`のフィクスチャで確認する。フィクスチャは過去に実際に書き込まれた生のキー・バリューと、そこから読めるべき値の組。現在のコードでそれを注入して開き直し、記録どおりに読めることを確かめる（LMDBは版が食い違うフィクスチャを`SchemaVersionMismatch`として拒否できることも確かめる）。

`SCHEMA_VERSION`を上げる破壊的変更をした場合、古い版のフィクスチャは「読めなくなる」側の検証へ自動的に切り替わるので、そのままで構わない（テストを個別にignoreする必要はない）。新しい版のフィクスチャを追加したい場合は`tests/gen_db_compat_fixture.rs`を手動で実行する（`gen-fixtures` featureを立てたときだけコンパイルされ、通常のテスト実行やCIでは走らない）。

```bash
# LMDB
cargo test --no-default-features --features "backend-lmdb,gen-fixtures" \
    --test gen_db_compat_fixture -- --nocapture

# TiKV（先に `docker compose up -d --wait` でクラスタを起こしておくこと。
# 専有していないクラスタに対して走らせると、他の書き込みまでフィクスチャへ混ざる恐れがある）
cargo test --no-default-features --features "backend-tikv,gen-fixtures" \
    --test gen_db_compat_fixture -- --nocapture
```

# デバッグ
開発中にサーバーが正しく動いているかを確認したい場合、サーバー側で `ServerReflection` が有効になっているため、`grpcurl` や Buf Studio、Postman、および **Bruno** などの gRPC クライアントから対話的に RPC を呼び出してデバッグできます（詳細は `docs/grpc.md` を参照）。

## Bruno による GUI デバッグ
リポジトリ直下の `.bruno` フォルダは Bruno（v2.10.0+ / v3.1+）のコレクションとして構成されています。

1. **コレクションを開く**: Bruno の「Open Collection」から本リポジトリの `.bruno` フォルダを選択して開きます。
2. **環境の選択**: 右上の環境プルダウンから `dev`（`baseUrl: 127.0.0.1:5172`）を選択します。
3. **認証**: `Auth/login` を実行し、レスポンスの `token` 文字列をコピーして、環境変数 `dev` の `accessToken`（Current Value）に貼り付けます（※現在の Bruno では gRPC の自動スクリプト実行が未対応のため）。
4. **リクエスト実行**: `Databases/create`（`dev_db` 作成）や `Tables/create`（`dev_table` 作成）、`Data/insert`、`Data/search` などのリクエストをそのまま呼び出して動作確認やデバッグが行えます（トークンは 24 時間有効です）。

原則として、削除や変更には消極的で、追加には積極的である。これはあくまで指針であり、必要であれば削除することもある。