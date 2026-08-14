//! ACL 行の鍵と値。両バックエンドで同じ並びにする。
//!
//! ```text
//!   forward : principal_id(16) ‖ db_id(16) ‖ table_slot(16)  -> role(1)
//!   reverse : db_id(16) ‖ table_slot(16) ‖ principal_id(16)  -> ()
//! ```
//!
//! `table_slot` は全ゼロでデータベーススコープを表す（[`DataTarget::slot`]）。
//!
//! **前置が固定長 16 バイトなのが要点。** 利用者名で前置すると、`alice` を前置にした
//! 範囲検索が `alicebob` の行まで拾う。ID なら境界が構造的に決まる。
//!
//! 2 本持つのは向きが違うため。`forward` は「この主体が何を持つか」（認可・一覧）、
//! `reverse` は「この対象を誰が持つか」（データベース・テーブル削除時の掃除）に使う。
//! `reverse` がデータベース前置なので、データベース削除は 1 プレフィックスで
//! **データベーススコープ行と配下テーブル行の両方**を列挙できる。

use crate::error::AppError;
use crate::models::id::{DataTarget, DatabaseId, PrincipalId, TableId};
use crate::models::users::DataRole;

use super::UUID_LEN;

/// ACL の 1 行を一意に決める鍵。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AclKey {
    pub principal: PrincipalId,
    pub target: DataTarget,
}

/// 1 行を表す 2 本の鍵。
///
/// **必ず一組で扱う。** 片方だけ書くと、前向きにしか見えない行や、対象を消しても
/// 消えない行ができる。組にして返すことで、呼び出し側が片方を書き忘れられない。
pub struct AclRow {
    pub forward: Vec<u8>,
    pub reverse: Vec<u8>,
}

impl AclRow {
    /// 削除はどちらの向きも同じ扱いなので、まとめて渡せる形にしておく。
    pub fn into_keys(self) -> [Vec<u8>; 2] {
        [self.forward, self.reverse]
    }
}

impl AclKey {
    /// 3 つの識別子を並べただけなので、どちらの向きも同じ長さ。
    pub const LEN: usize = UUID_LEN * 3;

    pub fn new(principal: PrincipalId, target: DataTarget) -> Self {
        Self { principal, target }
    }

    /// この行を表す 2 本の鍵。
    pub fn rows(&self) -> AclRow {
        let (principal, db, slot) = (
            self.principal.into_bytes(),
            self.target.db_id.into_bytes(),
            self.target.slot().into_bytes(),
        );
        AclRow {
            forward: concat([principal, db, slot]),
            reverse: concat([db, slot, principal]),
        }
    }

    /// 引くだけなら前向きの 1 本で足りる。
    pub fn forward(&self) -> Vec<u8> {
        concat([
            self.principal.into_bytes(),
            self.target.db_id.into_bytes(),
            self.target.slot().into_bytes(),
        ])
    }

    pub fn decode_forward(key: &[u8]) -> Result<Self, AppError> {
        let [principal, db_id, slot] = split(key, "acl")?;
        Ok(Self::new(
            PrincipalId::from(principal),
            DataTarget::from_slot(DatabaseId::from(db_id), TableId::from(slot)),
        ))
    }

    pub fn decode_reverse(key: &[u8]) -> Result<Self, AppError> {
        let [db_id, slot, principal] = split(key, "acl reverse")?;
        Ok(Self::new(
            PrincipalId::from(principal),
            DataTarget::from_slot(DatabaseId::from(db_id), TableId::from(slot)),
        ))
    }

    // --- 走査の範囲（前置） ---

    /// この主体が持つ全行。
    pub fn owned_by(principal: PrincipalId) -> Vec<u8> {
        principal.into_bytes().to_vec()
    }

    /// この主体がこのデータベース配下に持つ全行。
    pub fn owned_by_in(principal: PrincipalId, db_id: DatabaseId) -> Vec<u8> {
        [principal.into_bytes(), db_id.into_bytes()].concat()
    }

    /// この 1 対象を持つ全主体（逆引き）。
    pub fn holders_of(target: DataTarget) -> Vec<u8> {
        [target.db_id.into_bytes(), target.slot().into_bytes()].concat()
    }

    /// このデータベースに関する全行（配下テーブルの行も含む、逆引き）。
    pub fn holders_in(db_id: DatabaseId) -> Vec<u8> {
        db_id.into_bytes().to_vec()
    }
}

/// 行の値（ロール 1 バイト）を読む。
pub fn decode_role(value: &[u8]) -> Result<DataRole, AppError> {
    let byte = value
        .first()
        .copied()
        .ok_or_else(|| AppError::InternalError("acl row has an empty value".to_string()))?;
    DataRole::try_from(byte)
}

fn concat(parts: [[u8; UUID_LEN]; 3]) -> Vec<u8> {
    parts.concat()
}

fn split(key: &[u8], what: &str) -> Result<[[u8; UUID_LEN]; 3], AppError> {
    let bytes: &[u8; AclKey::LEN] = key.try_into().map_err(|_| {
        AppError::InternalError(format!(
            "{what} key has the wrong length (expected {}, found {})",
            AclKey::LEN,
            key.len()
        ))
    })?;
    let mut parts = [[0u8; UUID_LEN]; 3];
    for (slot, chunk) in parts.iter_mut().zip(bytes.chunks_exact(UUID_LEN)) {
        slot.copy_from_slice(chunk);
    }
    Ok(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids() -> (PrincipalId, DatabaseId, TableId) {
        (
            PrincipalId::from([1u8; 16]),
            DatabaseId::from([2u8; 16]),
            TableId::from([3u8; 16]),
        )
    }

    #[test]
    fn both_directions_round_trip() {
        let (p, d, t) = ids();
        for target in [DataTarget::database(d), DataTarget::table(d, t)] {
            let key = AclKey::new(p, target);
            let row = key.rows();
            assert_eq!(AclKey::decode_forward(&row.forward).unwrap(), key);
            assert_eq!(AclKey::decode_reverse(&row.reverse).unwrap(), key);
        }
    }

    /// 名前で前置していたら `alice` / `alicebob` で起きていた取り違え。
    #[test]
    fn principal_prefix_cannot_bleed_into_another_principal() {
        let (p, d, t) = ids();
        let other = PrincipalId::from([9u8; 16]);
        let target = DataTarget::table(d, t);

        assert!(
            AclKey::new(p, target)
                .forward()
                .starts_with(&AclKey::owned_by(p))
        );
        assert!(
            !AclKey::new(other, target)
                .forward()
                .starts_with(&AclKey::owned_by(p))
        );
    }

    /// データベース削除は 1 プレフィックスで両方の行を掴めること。
    #[test]
    fn holders_in_covers_scope_and_table_rows() {
        let (p, d, t) = ids();
        let prefix = AclKey::holders_in(d);

        for target in [DataTarget::database(d), DataTarget::table(d, t)] {
            assert!(AclKey::new(p, target).rows().reverse.starts_with(&prefix));
        }

        let other_db = DatabaseId::from([7u8; 16]);
        assert!(
            !AclKey::new(p, DataTarget::table(other_db, t))
                .rows()
                .reverse
                .starts_with(&prefix)
        );
    }

    #[test]
    fn database_scope_sorts_before_every_table() {
        let (p, d, t) = ids();
        assert!(
            AclKey::new(p, DataTarget::database(d)).forward()
                < AclKey::new(p, DataTarget::table(d, t)).forward()
        );
    }

    #[test]
    fn malformed_keys_and_values_are_rejected() {
        assert!(AclKey::decode_forward(&[0u8; 10]).is_err());
        assert!(AclKey::decode_reverse(&[]).is_err());
        assert!(decode_role(&[]).is_err());
        assert!(decode_role(&[9]).is_err());
        assert_eq!(decode_role(&[2]).unwrap(), DataRole::Write);
    }
}
