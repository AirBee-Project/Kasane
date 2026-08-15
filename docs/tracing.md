# Kasane トレーシング・ポリシー

このドキュメントでは、Kasaneプロジェクトにおける可観測性（トレース・メトリクス・ログ）のポリシーを定めます。

## 基本方針: 細粒度収集、ただし「繰り返し呼ばれるもの」は除外

Kasaneは **1 リクエスト・1 バックグラウンドジョブに対応する操作は、できる限り個別のスパンとして記録します**。データベースの作成、テーブルの更新、ACL 1 行の付与・剥奪、GC の 1 周回など、意味のある単位はすべて `#[tracing::instrument]` を付けます。以前の方針（「リポジトリ層には原則付けない」）は撤回しました。理由は次のとおりです。

- スパンの粒度が粗いと、リクエスト全体のうちどの内部操作が遅いのか・失敗したのかが分かりません
- ログの肥大化は「粒度」ではなく「呼ばれる回数」で決まります。1 リクエストにつき 1 回しか呼ばれない関数は、細かく分けても総スパン数は増えません

ただし、次に該当する関数には **付けません**。付けると 1 リクエストが数百〜数千の子スパンを生み、輻輳時にトレースそのものが読めなくなるためです。

- **同じキー・同じ要素に対して繰り返し呼ばれるもの**（例: シャードのリーフ 1 枚ごとの復号、木の降下（BFS/再帰）の 1 段ごと）
- **再帰、または実質再帰と同じ形の `loop`**（例: 親への統合を辿る `try_merge_up`）
- **`rayon` で並列展開される葉ごとの処理**
- **リトライループの 1 試行ごと**（TiKV の書き込みは競合で何度もやり直すのが前提の制御フロー。試行ごとにスパンを張ると輻輳時に子スパンが際限なく増える。代わりに [4. メトリクス](#4-メトリクス) の計器で試行の結末を数える）
- 点参照の内部ヘルパ（例: 名前 → ID の 1 回きりの引き当て）。これらは呼び出し元の**1 つ上の**操作がすでにスパンを持っているので、分けても情報量が増えません

判断に迷ったら「この関数は 1 回のリクエスト／1 回のジョブ実行で、入力サイズに関わらず**定数回**しか呼ばれないか」を基準にしてください。定数回なら計装し、要素数や木の深さに比例して呼ばれるなら計装しません。

### 層ごとの扱い

- **HTTP/アプリケーション境界**: `tower_http::trace` と `OtelAxumLayer` に一任します。ここは変更していません
- **ハンドラー層**: 引き続き `#[tracing::instrument]` を使いません。HTTP 境界のスパンとほぼ同じ内容になるためです
- **サービス層・リポジトリ層**: 意味のある操作単位（データベース／テーブル／ユーザー／ACL 行 1 つに対する 1 操作）には付けます。要素ごとに繰り返し呼ばれる内部関数には付けません

## 1. HTTP/アプリケーション境界 (Edge)

`TraceLayer` により、HTTPメソッド、URI、全体的なレイテンシ、レスポンスのステータスコードが自動的に記録されます。`url.route` はテンプレート化されたルート（例: `/databases/{db_name}`）で、生パスではありません。

`url.scheme` は追加のミドルウェア（[`middleware::scheme::normalize`](../src/middleware/scheme.rs)）で補っています。HTTP/1.1 の受信リクエストはそもそもスキームを持たない（origin-form）ため、何もしないと常に空になります。Kasane 自身は TLS を終端しないので、`X-Forwarded-Proto` があればそれを採用し、無ければ `http` とみなします。

## 2. エンドポイント設定

`OTEL_EXPORTER_OTLP_ENDPOINT` は **OTel 仕様どおり、パスを付けずに指定します**（例: `https://otlp.nr-data.net`）。信号ごとのパス（`/v1/traces` 等）は自動で補います。信号ごとに別のコレクタへ送りたい場合だけ `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT` / `_METRICS_ENDPOINT` / `_LOGS_ENDPOINT` を使ってください（こちらはパスを含めた完全な URL を渡します）。

対応する標準環境変数:

| 変数 | 用途 |
| --- | --- |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | 3 信号共通のベース URL（パス無し） |
| `OTEL_EXPORTER_OTLP_{TRACES,METRICS,LOGS}_ENDPOINT` | 信号別の完全な URL |
| `OTEL_EXPORTER_OTLP_HEADERS` | `k=v,k=v`（例: `api-key=xxxx`）。値は percent-decode される |
| `OTEL_SERVICE_NAME` | サービス名。既定 `kasane` |
| `OTEL_RESOURCE_ATTRIBUTES` | `k=v,k=v` 形式の追加リソース属性 |
| `OTEL_TRACES_SAMPLER` | `parentbased_always_on`（既定） / `always_on` / `always_off` / `traceidratio` / `parentbased_traceidratio` / `parentbased_always_off` |
| `OTEL_TRACES_SAMPLER_ARG` | `traceidratio` 系のサンプリング率（0.0〜1.0） |

`CLOUD_REGION` / `CLOUD_AVAILABILITY_ZONE` / `CLOUD_PROVIDER` / `DEPLOYMENT_ENVIRONMENT_NAME` / `HOST_NAME` は Kasane 独自の変数として今も使えます（`OTEL_RESOURCE_ATTRIBUTES` と併用可）。

## 3. トレース

`--features production` かつ `OTEL_EXPORTER_OTLP_ENDPOINT`（または `_TRACES_ENDPOINT`）が設定されているときだけ送信します。既定のサンプリングは仕様どおり `parentbased_always_on`（全件収集）です。

## 4. メトリクス

トレースと同じ OTLP エンドポイントへ、[`telemetry::metrics`](../src/telemetry.rs) が持つアプリ固有の計器を送ります。スパンから導ける量（リクエスト数・レイテンシ）はサンプリングを入れると不正確になるため、計器としても別に持ちます。

| 計器 | 種別 | 意味 |
| --- | --- | --- |
| `http.server.request.duration` | histogram (s) | HTTP リクエストの所要時間。`http.route` / `http.response.status_code` 別 |
| `kasane.storage.write.attempts` | counter | 書き込みトランザクションの試行。`outcome`（`committed` / `conflict` / `lock_declared` / `stale` / `failed`）別 |
| `kasane.storage.write.duration` | histogram (s) | 書き込みトランザクション全体の所要時間（リトライぶんも含む） |
| `kasane.storage.read.duration` | histogram (s) | 読み取りトランザクションの所要時間 |
| `kasane.write.batch.size` | histogram | 1 トランザクションへ畳んだ書き込み要求数（[`WriteCoalescer`](../src/services/database/table/data/coalesce.rs)） |
| `kasane.gc.reclaimed` | counter | 削除済みテーブルの回収で消えたキー数（TiKV のみ） |

`production` 以外のビルドではこれらの関数はすべて無コストな no-op になります（呼び出し側は `cfg` を書かずに済みます）。

## 5. ログ

`tracing` のイベントは 2 系統へ流れます。

1. **標準出力**: `KASANE_LOG_FORMAT=json` なら JSON、それ以外はプレーンテキスト。これは従来どおりです
2. **OTLP ログ**（`production` かつエンドポイント設定時）: [`opentelemetry-appender-tracing`](https://docs.rs/opentelemetry-appender-tracing) がイベントをそのまま OTLP のログレコードへ橋渡しします。**現在のスパン文脈（trace_id / span_id）が自動で載る**ので、New Relic などの UI でトレースからログへ、ログからトレースへ相互に辿れます

ログレベルの使い分けは変更していません。

- **`error!`**: クリティカルな障害、予期せぬデータベースの切断、システムにおける不変条件の破壊（あってはならない状態）などに使用します。（注: 通常の `AppError` はHTTPステータスコードに変換され、`tower_http` が自動的に4xx/5xxとして記録します）
- **`warn!`**: 致命的ではないエラー、代替処理（フォールバック）の発生、不審だが仕様上有効なリクエストなどに使用します。
- **`info!`**: 「データベースの初期化完了」や「サーバーの起動」など、ライフサイクル上の主要なイベントに使用します。**すべての関数の開始を告げる目的で `info!` を使用しないでください**（例: APIパスから明らかであるにも関わらず `info!("Creating table...")` のようにログを出さないこと）。
- **`debug!` / `trace!`**: ローカル開発時に、内部状態を詳細に出力したい場合に使用します。これらは本番環境ではデフォルトで無効化されます。

## 6. シャットダウン時のフラッシュ

`TelemetryGuard`（[`main.rs`](../src/main.rs)）が `Drop` の中でトレース・メトリクス・ログの 3 プロバイダをまとめて `shutdown()` します。バッチエクスポータは溜めてから送るので、これを通さないとプロセス終了直前のデータが送信されずに消えます。
