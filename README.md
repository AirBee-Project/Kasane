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
```

起動すると、実際に待ち受けている URL と使用中のデータベースファイル名が表示されます。
