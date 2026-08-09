//! ユーザーと権限の永続化。
//!
//! 専用のトランザクション型は持たず、[`KasaneDbRead`](super::KasaneDbRead) /
//! [`KasaneDbWrite`](super::KasaneDbWrite) への `impl` として実装している。
//! 権限の付与はデータベース・テーブルのカタログを同じトランザクション内で
//! 引く必要があるため、ストレージアクセスを 2 つの型に閉じておく。

pub mod read;
pub mod write;
