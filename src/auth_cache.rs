use std::collections::HashMap;
use std::time::{Duration, Instant};

use crate::models::users::User;

#[derive(Debug, Clone)]
pub struct AuthCache {
    pub users: HashMap<String, (Instant, User)>,
    pub max_capacity: usize,
    pub ttl: Duration,
}

impl AuthCache {
    pub fn new() -> Self {
        Self {
            users: HashMap::new(),
            max_capacity: 10_000,
            ttl: Duration::from_secs(5 * 60), // 5 minutes TTL
        }
    }

    pub fn get(&self, username: &str) -> Option<User> {
        self.users.get(username).and_then(|(time, user)| {
            if time.elapsed() < self.ttl {
                Some(user.clone())
            } else {
                None
            }
        })
    }

    pub fn insert(&mut self, username: String, user: User) {
        if self.users.len() >= self.max_capacity && !self.users.contains_key(&username) {
            // First try to remove expired entries
            let now = Instant::now();
            let ttl = self.ttl;
            self.users
                .retain(|_, (time, _)| now.duration_since(*time) < ttl);

            // If still at or over capacity, evict the oldest entries until there is
            // room. 全消去ではなく古い順に追い出すことで、有効なエントリの大量喪失を避ける。
            if self.users.len() >= self.max_capacity {
                let mut entries: Vec<(String, Instant)> = self
                    .users
                    .iter()
                    .map(|(k, (time, _))| (k.clone(), *time))
                    .collect();
                // 古いものが先頭に来るようソート
                entries.sort_by_key(|(_, time)| *time);

                let to_remove = self.users.len() + 1 - self.max_capacity;
                for (key, _) in entries.into_iter().take(to_remove) {
                    self.users.remove(&key);
                }
            }
        }
        self.users.insert(username, (Instant::now(), user));
    }

    pub fn remove(&mut self, username: &str) {
        self.users.remove(username);
    }
}

impl Default for AuthCache {
    fn default() -> Self {
        Self::new()
    }
}
