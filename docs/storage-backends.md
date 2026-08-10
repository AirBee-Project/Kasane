# ストレージバックエンド

Kasane は LMDB と TiKV のどちらでも動く。バックエンドは Cargo feature で**ビルドごとに 1 つ**
選ぶ（両方有効・どちらも無効はコンパイルエラーになる）。

```bash
cargo build                                              # LMDB（既定）
cargo build --no-default-features --features backend-tikv
```

## 層の構成

アプリケーションの状態はすべてストレージ層の向こう側に閉じている。サービス層・ハンドラ層が
触れるのは trait だけで、`heed::Env` や TiKV クライアントは外に出ない。

| 場所 | 役割 |
|---|---|
| `repositories/storage.rs` | 抽象 API。`Storage` / `ReadRepository` / `WriteRepository` / `MetaRepository` |
| `repositories/encoding/` | 両バックエンドで共通のバイト表現（シャードノード形式、値インデックスのキー） |
| `repositories/lmdb/` | LMDB 実装 |
| `repositories/tikv/` | TiKV 実装 |
| `backend.rs` | feature で選ばれた実装を `Db` という 1 つの名前に束ね、その構築（`open`）も行う |

2 つのバックエンドは**同じファイル構成**を持つ。片方を読めばもう片方の対応箇所がすぐ分かり、
新しいバックエンドを追加するときの雛形にもなる。

| ファイル | 役割 |
|---|---|
| `mod.rs` | ストレージ本体と、トランザクション境界（`Storage` 実装） |
| `init.rs` | そのバックエンド固有の初期化設定（環境変数・接続先・既定ユーザーの投入） |
| `keys.rs` | キーのバイト表現 |
| `shard.rs`（LMDB）/ `kv.rs`（TiKV） | そのバックエンド固有の低レベルアクセス |
| `catalog.rs` | データベース・テーブルのカタログ操作 |
| `data.rs` | FlexTree のデータ操作 |
| `users.rs` | ユーザーと権限 |
| `query_source.rs` | クエリ実行器への入力源 |
| `repository.rs` | 抽象 trait への適合（委譲のみ） |

バックエンド固有のキーレイアウトは各実装の `keys.rs` にある。`encoding/` に置くのは
**どちらのバックエンドでも同じになる**表現だけ。

トランザクション境界はクロージャで表される。

```rust
storage.read(async |r| r.database_info("mydb").await).await?;
storage.write(async move |w| w.database_create(&name, None).await).await?;
```

ハンドルを返す形にしていないのは、**書き込みがやり直しになりうる**ため。やり直しを
ストレージ層の内部に閉じ込めることで、サービス層は競合の存在を知らずに済み、
「書き込みは待たされても失敗しない」という性質が呼び出し側から見て保たれる。

> **書き込みクロージャは複数回呼ばれうる。** 副作用をクロージャの外へ持ち出さないこと。
> 引数は毎回複製して渡す。

## LMDB バックエンド

- 起動時にデータディレクトリを開く（`--database-path`、既定は `DATABASE_DIR` 環境変数）
- 単一プロセスの組み込みストレージ。書き込みは環境全体で単一ライタ
- `Storage::read` / `write` は blocking タスクを 1 つ起こし、**その中でトランザクションを
  開き・クロージャを回し・閉じる**。LMDB の「トランザクションは単一スレッドから」という
  制約が構造的に守られ、unsafe な自己参照も不要になる

## TiKV バックエンド

- PD エンドポイントは `KASANE_TIKV_PD_ENDPOINTS`（カンマ区切り、既定 `127.0.0.1:2379`）
- 開発・テスト用のクラスタは `deployment/tikv/docker-compose.yml` で起動する

### キーレイアウト

TiKV のキースペースはフラットな 1 本なので、先頭 1 バイトの名前空間タグで論理テーブルを分ける。
バイト順序は LMDB と同じなので、値インデックスの順序保存エンコーディングはそのまま通用する。

```text
0x01 ‖ name                          -> DatabaseMetadata
0x02 ‖ db_id ‖ table_name            -> TableMetadata
0x03 ‖ db_id                         -> データベース名
0x04 ‖ table_id                      -> テーブル名
0x05 ‖ username                      -> UserMetadata
0x06 ‖ table_id ‖ flex_id            -> シャードエントリ
0x07 ‖ table_id ‖ vkey ‖ flex_id     -> 値インデックス
0x7F ‖ scope ‖ id                    -> ロック専用（値を書かない）
```

### トランザクションとロック

TiKV の悲観ロックは取得時に取り直した `for_update_ts` で取られるのに対し、`txn.get()` は
開始時の `start_ts` スナップショットを読む。そのため「1 つのトランザクション内でロックしてから
読む」と、ロック取得前にコミットされた他者の変更を見落として **lost update になる**
（実測済み。`tikv-migration-phase0.md` を参照）。

そこで LMDB の `env.write_txn()` と同じ順序を 2 つのトランザクションで再現する。

1. ロック専用トランザクションでロックを取得（ミューテックス取得に相当）
2. **その後に**作業トランザクションを開始（start_ts が前任者のコミットより後になる）
3. 作業をコミット
4. ロック専用トランザクションを rollback して解放

ロック側は常に rollback で終わるので、ロックキーに MVCC のバージョンは作られない。

### ロックの粒度

無関係な書き込みが直列化されないよう、触る範囲ごとにロックを分けている。

| スコープ | 使う操作 |
|---|---|
| データベース単位 | テーブルの作成・削除・複製、データベースの削除・改名・複製 |
| テーブル単位 | データ書き込み、テーブル削除・複製、既存データ検証 |
| ユーザー単位 | ユーザーと権限の変更 |

複数スコープが必要な操作は必ず**データベース単位 → テーブル単位**の順に取得する。
全操作がこの順序を守る限り、待ちグラフに循環ができずデッドロックしない。

必要なロックを事前に列挙できない操作（`database_remove` は DB ロックを取って初めて対象
テーブルが判る）に対応するため、各操作は触る範囲を `require_lock` で宣言する。まだ保持して
いないロックが宣言されたら、その試行は破棄され、**ロックを揃えた状態でクロージャが最初から
実行し直される**。単純な操作は最初の宣言で判明するので、1 周目は I/O を伴わない。

### LMDB との実装上の違い

- ゼロコピー（mmap 上の archived マップを直接読む）は使えないため、常に所有バイト列から復元する
- 木の降下では同じ深さのノードを `batch_get` でまとめて取得し、往復回数を深さ分に抑える
- 読み取りの rayon 並列化は行わない（ネットワーク I/O が支配的なため）

## テスト

```bash
# LMDB
cargo test

# TiKV（クラスタが必要）
docker compose -f deployment/tikv/docker-compose.yml up -d
cargo test --no-default-features --features backend-tikv
```

TiKV の結合テスト（`tests/tikv_backend.rs`）は、テストごとに固有のデータベース名を使うので
同じクラスタに対して並行実行してよい。シャード分割を跨いだセルの保全と、並行書き込みでの
lost update が起きないことを含めて確認している。
