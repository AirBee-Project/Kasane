use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::RwLock;
use std::time::{Duration, Instant};

use crate::models::users::User;

const NUM_SHARDS: usize = 64;

#[derive(Debug, Clone)]
struct Entry {
    time: Instant,
    user: User,
}

#[derive(Debug)]
struct Partition {
    users: HashMap<String, Entry>,
    queue: VecDeque<(String, Instant)>,
}

impl Partition {
    fn new() -> Self {
        Self {
            users: HashMap::new(),
            queue: VecDeque::new(),
        }
    }
}

/// 認証済みユーザー情報のインメモリキャッシュ。
///
/// スレッドセーフな内部シャーディング（64分割）と O(1) での最古エントリ追い出しを提供し、
/// 高並行アクセス時のロック競合を最小化します。
/// TTL は `get` 時に遅延判定されます。
#[derive(Debug)]
pub struct AuthCache {
    shards: Vec<RwLock<Partition>>,
    max_capacity_per_shard: usize,
    ttl: Duration,
}

impl AuthCache {
    pub fn new() -> Self {
        Self::with_config(10_000, Duration::from_secs(5 * 60))
    }

    fn with_config(max_capacity: usize, ttl: Duration) -> Self {
        let max_capacity_per_shard = (max_capacity / NUM_SHARDS).max(1);
        let mut shards = Vec::with_capacity(NUM_SHARDS);
        for _ in 0..NUM_SHARDS {
            shards.push(RwLock::new(Partition::new()));
        }

        Self {
            shards,
            max_capacity_per_shard,
            ttl,
        }
    }

    fn get_shard_index(username: &str) -> usize {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        username.hash(&mut hasher);
        (hasher.finish() as usize) % NUM_SHARDS
    }

    pub fn get(&self, username: &str) -> Option<User> {
        let shard_idx = Self::get_shard_index(username);
        let partition = self.shards[shard_idx].read().unwrap();

        partition.users.get(username).and_then(|entry| {
            if entry.time.elapsed() < self.ttl {
                Some(entry.user.clone())
            } else {
                None
            }
        })
    }

    pub fn insert(&self, username: String, user: User) {
        let shard_idx = Self::get_shard_index(&username);
        let mut partition = self.shards[shard_idx].write().unwrap();

        // 容量確保: 最古のエントリから順に追い出す（O(1)）
        while partition.users.len() >= self.max_capacity_per_shard {
            if let Some((evicted_name, evicted_time)) = partition.queue.pop_front() {
                // キューから取り出した時刻が現在の HashMap 内のものと一致する場合のみ削除
                // （一致しない場合は同一ユーザーで後から insert され上書きされたエントリ）
                if let Some(current_entry) = partition.users.get(&evicted_name)
                    && current_entry.time == evicted_time
                {
                    partition.users.remove(&evicted_name);
                }
            } else {
                // キューが空の場合はループを抜ける
                break;
            }
        }

        let time = Instant::now();
        partition
            .users
            .insert(username.clone(), Entry { time, user });
        partition.queue.push_back((username, time));
    }

    pub fn remove(&self, username: &str) {
        let shard_idx = Self::get_shard_index(username);
        let mut partition = self.shards[shard_idx].write().unwrap();
        partition.users.remove(username);
        // queue の中のエントリは放置してよい（追い出し時に time が合わないため無視される）
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

    /// 容量超過時に最古エントリだけが追い出されること
    #[test]
    fn evicts_oldest_when_over_capacity() {
        // max_capacity = 64 -> シャードあたり1件
        let cache = AuthCache::with_config(64, Duration::from_secs(300));

        // 1000件挿入。各シャードにはおおよそ15件ずつ挿入されるはず。
        for i in 0..1000 {
            let name = format!("user_{}", i);
            cache.insert(name.clone(), dummy_user(&name));
        }

        // シャードの合計サイズが最大64であることを確認
        let total_users: usize = cache
            .shards
            .iter()
            .map(|s| s.read().unwrap().users.len())
            .sum();
        assert!(total_users <= 64);
        assert!(total_users > 0);
    }

    /// 同一 username の上書きで不整合が起きないこと
    #[test]
    fn overwrite_keeps_data_valid() {
        let cache = AuthCache::with_config(6400, Duration::from_secs(300));

        cache.insert("a".to_string(), dummy_user("a"));
        cache.insert("a".to_string(), dummy_user("a"));
        cache.insert("a".to_string(), dummy_user("a"));

        let shard_idx = AuthCache::get_shard_index("a");
        let partition = cache.shards[shard_idx].read().unwrap();

        assert_eq!(partition.users.len(), 1);
        // 古いエントリは残るが、追い出しロジックで無視される
        assert_eq!(partition.queue.len(), 3);
        drop(partition);

        assert!(cache.get("a").is_some());
    }

    /// remove でマップから消えること
    #[test]
    fn remove_clears_map() {
        let cache = AuthCache::with_config(10, Duration::from_secs(300));
        cache.insert("a".to_string(), dummy_user("a"));
        cache.remove("a");

        assert!(cache.get("a").is_none());
    }

    /// TTL 切れのエントリは get で返さないこと
    #[test]
    fn expired_entries_are_not_returned() {
        let cache = AuthCache::with_config(10, Duration::from_nanos(1));
        cache.insert("a".to_string(), dummy_user("a"));
        std::thread::sleep(Duration::from_millis(1));
        assert!(cache.get("a").is_none());
    }
}
