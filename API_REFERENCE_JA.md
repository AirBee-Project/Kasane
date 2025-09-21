# Kasane API リファレンス

このドキュメントは、Kasane で利用可能なすべての INPUT コマンドと対応する OUTPUT 構造の包括的なリファレンスを提供します。

## 概要

Kasane は、以下の構造を持つ JSON パケットを通じてコマンドを受け付けます。

```json
{
  "session": "string",
  "command": [Command, ...]
}
```

## データベース操作

### CreateSpace

データベースに新しいスペースを作成します。

**INPUT:**
```json
{
  "createSpace": {
    "spaceName": "string"
  }
}
```

**OUTPUT:**
```json
"success"
```

### DropSpace

データベースから既存のスペースを削除します。

**INPUT:**
```json
{
  "dropSpace": {
    "spaceName": "string"
  }
}
```

**OUTPUT:**
```json
"success"
```

### InfoSpace

指定されたスペースの情報を取得します。

**INPUT:**
```json
{
  "infoSpace": {
    "spaceName": "string"
  }
}
```

**OUTPUT:**
```json
{
  "infoSpace": {
    "spacename": "string",
    "keynames": [
      {
        "keyname": "string",
        "keytype": "INT|BOOLEAN|TEXT|FLOAT",
        "keymode": "UniqueKey|MultiKey"
      }
    ]
  }
}
```

### ShowSpaces

利用可能なすべてのスペースを一覧表示します。

**INPUT:**
```json
"showSpaces"
```

**OUTPUT:**
```json
{
  "showSpaces": {
    "spacenames": ["string", ...]
  }
}
```

### Version

Kasane の現在のバージョンを返します。

**INPUT:**
```json
"version"
```

**OUTPUT:**
```json
{
  "version": {
    "version": "string"
  }
}
```

## キー操作

### CreateKey

スペース内に新しいキーを作成します。

**INPUT:**
```json
{
  "createKey": {
    "spaceName": "string",
    "keyName": "string",
    "keyType": "INT|BOOLEAN|TEXT|FLOAT",
    "keyMode": "UniqueKey|MultiKey"
  }
}
```

**OUTPUT:**
```json
"success"
```

### DropKey

スペースからキーを削除します。

**INPUT:**
```json
{
  "dropKey": {
    "spaceName": "string",
    "keyName": "string"
  }
}
```

**OUTPUT:**
```json
"success"
```

### ShowKeys

指定されたスペース内のすべてのキーを一覧表示します。

**INPUT:**
```json
{
  "showKeys": {
    "spaceName": "string"
  }
}
```

**OUTPUT:**
```json
{
  "showkeys": {
    "keynames": ["string", ...]
  }
}
```

### InfoKey

指定されたキーの情報を取得します。

**INPUT:**
```json
{
  "infoKey": {
    "spaceName": "string",
    "keyName": "string"
  }
}
```

**OUTPUT:**
```json
{
  "infoKey": {
    "keyname": "string",
    "keytype": "INT|BOOLEAN|TEXT|FLOAT",
    "keymode": "UniqueKey|MultiKey"
  }
}
```

## 値操作

### InsertValue

指定された範囲内のキーに新しい値を挿入します。

**INPUT:**
```json
{
  "insertValue": {
    "spaceName": "string",
    "keyName": "string",
    "range": Range,
    "value": ValueEntry
  }
}
```

**OUTPUT:**
```json
"success"
```

### PatchValue

指定された範囲内のキーの既存の値を更新します。

**INPUT:**
```json
{
  "patchValue": {
    "spaceName": "string",
    "keyName": "string",
    "range": Range,
    "value": ValueEntry
  }
}
```

**OUTPUT:**
```json
"success"
```

### DeleteValue

指定された範囲内のキーから値を削除します。

**INPUT:**
```json
{
  "deleteValue": {
    "spaceName": "string",
    "keyName": "string",
    "range": Range
  }
}
```

**OUTPUT:**
```json
"success"
```

### SelectValue

範囲内の指定されたキーから値を取得します。

**INPUT:**
```json
{
  "selectValue": {
    "spaceName": "string",
    "keyNames": ["string", ...],
    "range": Range,
    "vertex": boolean,
    "center": boolean,
    "idString": boolean,
    "idPure": boolean
  }
}
```

**OUTPUT:**
```json
{
  "selectValue": [
    {
      "id": SpaceTimeId,
      "center": Point,
      "vertex": [Point, Point, Point, Point, Point, Point, Point, Point],
      "idString": "string",
      "value": [["string", ValueEntry], ...]
    }
  ]
}
```

### ShowValues

指定されたキーのすべての値を表示します。

**INPUT:**
```json
{
  "showValues": {
    "spaceName": "string",
    "keyName": "string"
  }
}
```

**OUTPUT:**
```json
{
  "showValues": [
    {
      "id": SpaceTimeId,
      "center": Point,
      "vertex": [Point, Point, Point, Point, Point, Point, Point, Point],
      "idString": "string",
      "value": [["string", ValueEntry], ...]
    }
  ]
}
```

## ユーザー操作

### CreateUser

新しいユーザーアカウントを作成します。

**INPUT:**
```json
{
  "createUser": {
    "userName": "string",
    "password": "string"
  }
}
```

**OUTPUT:**
```json
"success"
```

### DropUser

ユーザーアカウントを削除します。

**INPUT:**
```json
{
  "dropUser": {
    "userName": "string"
  }
}
```

**OUTPUT:**
```json
"success"
```

### InfoUser

指定されたユーザーの情報を取得します。

**INPUT:**
```json
{
  "infoUser": {
    "userName": "string"
  }
}
```

**OUTPUT:**
```json
{
  "infoUser": {
    "userName": "string"
  }
}
```

### ShowUsers

すべてのユーザーアカウントを一覧表示します。

**INPUT:**
```json
"showUsers"
```

**OUTPUT:**
```json
{
  "showUsers": {
    "users": ["string", ...]
  }
}
```

## データ型

### Range

操作の時空間範囲を定義します：

```json
{
  "function": {
    "spot": {
      "point1": Point,
      "zoom": number
    }
  }
}
```

または

```json
{
  "function": {
    "line": {
      "point1": Point,
      "point2": Point,
      "zoom": number
    }
  }
}
```

または

```json
{
  "function": {
    "triangle": {
      "point1": Point,
      "point2": Point,
      "point3": Point,
      "zoom": number
    }
  }
}
```

または

```json
{
  "prefix": {
    "and": [Range, ...]
  }
}
```

または

```json
{
  "prefix": {
    "or": [Range, ...]
  }
}
```

または

```json
{
  "idSet": [
    {
      "z": number,
      "f": DimensionRange,
      "x": DimensionRange,
      "y": DimensionRange,
      "i": number,
      "t": DimensionRange
    }
  ]
}
```

### Point

地理的な点を表します：

```json
{
  "lat": number,
  "lng": number
}
```

### ValueEntry

型付きの値を表します：

- `{ "int": number }`
- `{ "boolean": boolean }`
- `{ "text": "string" }`
- `{ "float": number }`

### SpaceTimeId

時空間データの複合識別子（実装固有）。

### DimensionRange

次元の範囲を表します：

- `{ "single": number }`
- `{ "range": [number, number] }`

## 認証

ログイン以外のすべてのコマンドには有効なセッショントークンが必要です。セッションは `/login` エンドポイントを通じて取得されます：

**ログインリクエスト:**
```json
{
  "username": "string",
  "password": "string"
}
```

**ログインレスポンス:**
```json
{
  "sessionId": "string",
  "expiresInSecs": number
}
```

## エラーレスポンス

エラーが発生した場合、システムは期待される出力構造の代わりにエラー文字列を返します。

## 注意事項

- コマンドは配列として処理され、単一のリクエストで複数の操作が可能です
- セッショントークンは非アクティブ状態が1時間（3600秒）続くと期限切れになります
- すべての空間座標は WGS84（緯度/経度）を使用します
- システムは IPA の空間ID ガイドラインに従った4次元時空間データをサポートします