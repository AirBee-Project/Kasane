use std::collections::{BTreeMap, HashMap};
use std::time::{Duration, Instant};

use crate::models::users::User;

/// キャッシュ1件分のエントリ。
///
/// `time`・`seq` は時刻インデックス (`expiry_index`) のキーと対応し、
/// 削除・更新時に該当インデックスを引くために保持する。
#[derive(Debug, Clone)]
struct Entry {
    time: Instant,
    seq: u64,
    user: User,
}

/// 認証済みユーザー情報のインメモリキャッシュ。
///
/// - `users`: username -> エントリ。参照・更新は平均 O(1)。
/// - `expiry_index`: (挿入時刻, seq) -> username の時刻順インデックス。
///   容量超過時に最古エントリを `pop_first()` で O(log n) で追い出すために使う。
///   `seq` は同一 `Instant` での衝突を一意化するための単調増加カウンタ。
///
/// TTL は `get` 時に遅延判定する（期限切れは返さない）。期限切れエントリは
/// 容量圧迫時に最古から追い出される際に回収されるため、定期的な全走査は行わない。
#[derive(Debug, Clone)]
pub struct AuthCache {
    users: HashMap<String, Entry>,
    expiry_index: BTreeMap<(Instant, u64), String>,
    next_seq: u64,
    max_capacity: usize,
    ttl: Duration,
}

impl AuthCache {
    pub fn new() -> Self {
        Self::with_config(10_000, Duration::from_secs(5 * 60))
    }

    fn with_config(max_capacity: usize, ttl: Duration) -> Self {
        Self {
            users: HashMap::new(),
            expiry_index: BTreeMap::new(),
            next_seq: 0,
            max_capacity,
            ttl,
        }
    }

    pub fn get(&self, username: &str) -> Option<User> {
        self.users.get(username).and_then(|entry| {
            if entry.time.elapsed() < self.ttl {
                Some(entry.user.clone())
            } else {
                None
            }
        })
    }

    pub fn insert(&mut self, username: String, user: User) {
        // 同じ username を上書きする場合は、先に古い時刻インデックスを除去する
        if let Some(old) = self.users.remove(&username) {
            self.expiry_index.remove(&(old.time, old.seq));
        }

        // 容量確保: 最古のエントリから順に追い出す（O(log n)）
        while self.users.len() >= self.max_capacity {
            match self.expiry_index.pop_first() {
                Some((_, evicted)) => {
                    self.users.remove(&evicted);
                }
                None => break,
            }
        }

        let time = Instant::now();
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);

        self.users
            .insert(username.clone(), Entry { time, seq, user });
        self.expiry_index.insert((time, seq), username);
    }

    pub fn remove(&mut self, username: &str) {
        if let Some(entry) = self.users.remove(username) {
            self.expiry_index.remove(&(entry.time, entry.seq));
        }
    }
}

impl Default for AuthCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn dummy_user(name: &str) -> User {
        User {
            username: name.to_string(),
            id: Uuid::now_v7(),
            is_global_admin: false,
            token_version: 0,
        }
    }

    /// 容量超過時に最古エントリだけが追い出され、両マップが同期していること
    #[test]
    fn evicts_oldest_when_over_capacity() {
        let mut cache = AuthCache::with_config(3, Duration::from_secs(300));

        for name in ["a", "b", "c"] {
            cache.insert(name.to_string(), dummy_user(name));
        }
        // 4件目で最古の "a" が追い出される
        cache.insert("d".to_string(), dummy_user("d"));

        assert!(cache.get("a").is_none());
        assert!(cache.get("b").is_some());
        assert!(cache.get("c").is_some());
        assert!(cache.get("d").is_some());
        // 両マップのサイズが一致していること
        assert_eq!(cache.users.len(), 3);
        assert_eq!(cache.expiry_index.len(), 3);
    }

    /// 同一 username の上書きで時刻インデックスが肥大化しないこと
    #[test]
    fn overwrite_keeps_indexes_in_sync() {
        let mut cache = AuthCache::with_config(10, Duration::from_secs(300));

        cache.insert("a".to_string(), dummy_user("a"));
        cache.insert("a".to_string(), dummy_user("a"));
        cache.insert("a".to_string(), dummy_user("a"));

        assert_eq!(cache.users.len(), 1);
        assert_eq!(cache.expiry_index.len(), 1);
        assert!(cache.get("a").is_some());
    }

    /// remove で両マップから消えること
    #[test]
    fn remove_clears_both_maps() {
        let mut cache = AuthCache::with_config(10, Duration::from_secs(300));
        cache.insert("a".to_string(), dummy_user("a"));
        cache.remove("a");

        assert!(cache.get("a").is_none());
        assert_eq!(cache.users.len(), 0);
        assert_eq!(cache.expiry_index.len(), 0);
    }

    /// TTL 切れのエントリは get で返さないこと
    #[test]
    fn expired_entries_are_not_returned() {
        let mut cache = AuthCache::with_config(10, Duration::from_nanos(1));
        cache.insert("a".to_string(), dummy_user("a"));
        std::thread::sleep(Duration::from_millis(1));
        assert!(cache.get("a").is_none());
    }
}
