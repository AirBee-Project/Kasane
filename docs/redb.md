# 変数名のルール

- redbのトランザクションの変数名は`read_txn`か`write_txn`とする。
- redbのtableはopenして変数にするときに、`redb_{Table定義の定数の小文字}`という名前をつける。


# Repositories層のレイヤールール

- 公開関数の受付はlayer_nameで受け取る
- 内部関数はlayer_idを受け取ってもよい

