# Kasane Command Summary Table / Kasane コマンド一覧表

## English

| Category | Command | Input Parameters | Output Type | Description |
|----------|---------|------------------|-------------|-------------|
| **Database** | CreateSpace | spaceName | Success | Creates a new space |
| | DropSpace | spaceName | Success | Deletes a space |
| | InfoSpace | spaceName | InfoSpace | Gets space information |
| | ShowSpaces | (none) | ShowSpaces | Lists all spaces |
| | Version | (none) | Version | Returns version info |
| **Key** | CreateKey | spaceName, keyName, keyType, keyMode | Success | Creates a new key |
| | DropKey | spaceName, keyName | Success | Deletes a key |
| | ShowKeys | spaceName | Showkeys | Lists keys in space |
| | InfoKey | spaceName, keyName | InfoKey | Gets key information |
| **Value** | InsertValue | spaceName, keyName, range, value | Success | Inserts a value |
| | PatchValue | spaceName, keyName, range, value | Success | Updates a value |
| | DeleteValue | spaceName, keyName, range | Success | Deletes a value |
| | SelectValue | spaceName, keyNames, range, options | SelectValue | Queries values |
| | ShowValues | spaceName, keyName | ShowValues | Lists all values |
| **User** | CreateUser | userName, password | Success | Creates a user |
| | DropUser | userName | Success | Deletes a user |
| | InfoUser | userName | InfoUser | Gets user information |
| | ShowUsers | (none) | ShowUsers | Lists all users |

## 日本語

| カテゴリ | コマンド | 入力パラメータ | 出力タイプ | 説明 |
|---------|---------|-------------|-----------|------|
| **データベース** | CreateSpace | spaceName | Success | 新しいスペースを作成 |
| | DropSpace | spaceName | Success | スペースを削除 |
| | InfoSpace | spaceName | InfoSpace | スペース情報を取得 |
| | ShowSpaces | (なし) | ShowSpaces | 全スペースを一覧表示 |
| | Version | (なし) | Version | バージョン情報を返す |
| **キー** | CreateKey | spaceName, keyName, keyType, keyMode | Success | 新しいキーを作成 |
| | DropKey | spaceName, keyName | Success | キーを削除 |
| | ShowKeys | spaceName | Showkeys | スペース内のキーを一覧表示 |
| | InfoKey | spaceName, keyName | InfoKey | キー情報を取得 |
| **値** | InsertValue | spaceName, keyName, range, value | Success | 値を挿入 |
| | PatchValue | spaceName, keyName, range, value | Success | 値を更新 |
| | DeleteValue | spaceName, keyName, range | Success | 値を削除 |
| | SelectValue | spaceName, keyNames, range, options | SelectValue | 値を検索 |
| | ShowValues | spaceName, keyName | ShowValues | 全値を一覧表示 |
| **ユーザー** | CreateUser | userName, password | Success | ユーザーを作成 |
| | DropUser | userName | Success | ユーザーを削除 |
| | InfoUser | userName | InfoUser | ユーザー情報を取得 |
| | ShowUsers | (なし) | ShowUsers | 全ユーザーを一覧表示 |

## Key Types / キータイプ

- `INT` - Integer values / 整数値
- `BOOLEAN` - Boolean values / 真偽値  
- `TEXT` - String values / 文字列値
- `FLOAT` - Floating point values / 浮動小数点値

## Key Modes / キーモード

- `UniqueKey` - Single value per spatial location / 空間位置ごとに単一値
- `MultiKey` - Multiple values per spatial location / 空間位置ごとに複数値

## Currently Disabled Commands / 現在無効なコマンド

The following commands are defined but currently commented out in the implementation:

以下のコマンドは定義されていますが、現在の実装では無効化されています：

- UpdateValue
- Transaction
- GrantDatabase
- GrantSpacePrivilege  
- GrantKeyPrivilege
- RevokeDatabase
- RevokeSpacePrivilege
- RevokeKeyPrivilege

## Authentication / 認証

All commands require authentication through the `/login` endpoint, which returns a session token valid for 1 hour.

すべてのコマンドは `/login` エンドポイントによる認証が必要で、1時間有効なセッショントークンが返されます。