# 認証 (Authentication)

Kasaneでは、APIエンドポイントへのアクセスを制御するために、**APIキーベースの静的トークン認証**を採用しています。
読み取りと書き込みの操作に対して、それぞれ独立した権限レベル（ガード）を設定・適用することができます。

---

## 1. 認証の基本設計

認証は環境変数 `READ_KEY` および `WRITE_KEY` をベースに動作します。
これらが設定されているかどうかに応じて、APIの保護レベルが動的に変化します。

### 認証キーの種類

* **`READ_KEY`**: データの参照（読み取り）を許可するキーです。
* **`WRITE_KEY`**: データの作成・更新・削除（書き込み）を許可するキーです。

### 鍵の設定パターンと挙動マトリクス

| `READ_KEY` | `WRITE_KEY` | 読み取り操作 (READ) | 書き込み操作 (WRITE) | 説明 |
| :--- | :--- | :--- | :--- | :--- |
| **未設定** | **未設定** |  誰でも可能 |  誰でも可能 | ローカル開発・検証用の完全パブリックモードです。 |
| **設定あり** | **未設定** |  `READ_KEY` が必要 |  誰でも可能 | 書き込みは全公開し、読み取りのみを制限する特殊なケースです。 |
| **未設定** | **設定あり** |  誰でも可能 |  `WRITE_KEY` が必要 | 書き込みは制限されますが、読み取りは誰でも可能です。 |
| **設定あり** | **設定あり** |  `READ` or `WRITE` キーが必要 |  `WRITE_KEY` が必要 | 推奨されるセキュアな本番環境の構成です。`WRITE_KEY` の保持者は読み取りも可能です。 |

> [!NOTE]
> `READ_KEY` が有効な状態であれば、`WRITE_KEY` を提示した場合でも読み取りアクセスが許可されます。

---

## 2. クライアントからの利用方法 (APIの呼び出し方)

クライアントは、リクエストのHTTPヘッダーにAPIキーを付与して送信します。
以下の**いずれの形式**でも認識されます。

### ① Authorization ヘッダー (Bearer トークン形式)
もっとも一般的な認証ヘッダー形式です。

```http
Authorization: Bearer <YOUR_API_KEY>
```

#### cURL の例:
```bash
curl -X GET http://localhost:5173/layers \
  -H "Authorization: Bearer my_secret_read_key"
```

### ② x-api-key ヘッダー (カスタムヘッダー形式)
シンプルにカスタムヘッダーとしてキーを直接送信する形式です。

```http
x-api-key: <YOUR_API_KEY>
```

#### cURL の例:
```bash
curl -X POST http://localhost:5173/layers \
  -H "x-api-key: my_secret_write_key" \
  -H "Content-Type: application/json" \
  -d '{"name": "new_layer", "data_type": "Int", "max_zoom_level": 20}'
```

---

## 3. 実装の仕組み (開発者向け)

認証は Rust の Web フレームワークである **Axum の Extractor 機能** を利用して実現されています。
認証処理ロジック of 本体は `src/auth.rs` に定義されており、以下の2つのガード構造体が提供されています。

* `RequireRead` (読み取りガード)
* `RequireWrite` (書き込みガード)

### ハンドラーへの適用例

エンドポイント（ハンドラー関数）を保護するには、関数の最初の引数にガード（`RequireRead` または `RequireWrite`）を配置するだけです。
リクエストがハンドラーに到達する前に、Axum が自動的にヘッダーからキーを抽出し、`AppState` にあるキーとの比較検証を行います。

#### 読み取り用ハンドラーでの適用例
```rust
use crate::auth::RequireRead;

pub async fn layer_info(
    _auth: RequireRead, // ← 引数に指定するだけでエンドポイントが保護されます
    State(app_state): State<AppState>,
    Path(name): Path<String>,
) -> Result<Json<LayerInfoResponse>, AppError> {
    // 認証が成功した場合のみ、この処理が実行されます
    let layer = layer_info_service::info(&app_state, &name).await?;
    Ok(Json(layer))
}
```

#### 書き込み用ハンドラーでの適用例
```rust
use crate::auth::RequireWrite;

pub async fn layer_create(
    _auth: RequireWrite, // ← 書き込み用ガードを指定
    State(app_state): State<AppState>,
    Json(request): Json<CreateLayerRequest>,
) -> Result<Response, AppError> {
    // 認証が成功した場合のみ、この処理が実行されます
    ...
}
```

### 認証失敗時のエラーレスポンス

認証に失敗した場合、または必要なAPIキーがリクエストに含まれていなかった場合は、対象のハンドラーは実行されず、即座に以下の共通レスポンスが返されます。

* **HTTPステータスコード**: `401 Unauthorized`
* **レスポンスボディ (JSON)**:

```json
{
  "message": "Unauthorized"
}
```
