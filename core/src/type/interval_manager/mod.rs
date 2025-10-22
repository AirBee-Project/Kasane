use std::cmp::{Ordering, max};

type ID = u64;

#[derive(Debug, Clone)]
struct Interval {
    start: u64,
    end: u64,
    id: ID,
}

#[derive(Debug)]
struct Node {
    interval: Interval,
    height: i32,
    max_end: u64,
    left: Option<Box<Node>>,
    right: Option<Box<Node>>,
}

impl Node {
    fn new(interval: Interval) -> Self {
        Self {
            max_end: interval.end,
            height: 1,
            interval,
            left: None,
            right: None,
        }
    }

    fn height(node: &Option<Box<Node>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn max_end(node: &Option<Box<Node>>) -> u64 {
        node.as_ref().map_or(0, |n| n.max_end)
    }

    fn update(node: &mut Box<Node>) {
        node.height = 1 + max(Self::height(&node.left), Self::height(&node.right));
        node.max_end = max(
            node.interval.end,
            max(Self::max_end(&node.left), Self::max_end(&node.right)),
        );
    }

    fn balance_factor(node: &Box<Node>) -> i32 {
        Self::height(&node.left) - Self::height(&node.right)
    }

    fn rotate_right(mut y: Box<Node>) -> Box<Node> {
        let mut x = y.left.take().expect("rotate_right requires left child");
        let t2 = x.right.take();

        y.left = t2;
        Self::update(&mut y);

        x.right = Some(y);
        Self::update(&mut x);
        x
    }

    fn rotate_left(mut x: Box<Node>) -> Box<Node> {
        let mut y = x.right.take().expect("rotate_left requires right child");
        let t2 = y.left.take();

        x.right = t2;
        Self::update(&mut x);

        y.left = Some(x);
        Self::update(&mut y);
        y
    }

    fn insert(node: Option<Box<Node>>, interval: Interval) -> Option<Box<Node>> {
        let mut n = match node {
            None => return Some(Box::new(Node::new(interval))),
            Some(n) => n,
        };

        // start値で比較、同じならidで比較（一意性を保証）
        match (
            interval.start.cmp(&n.interval.start),
            interval.id.cmp(&n.interval.id),
        ) {
            (Ordering::Less, _) | (Ordering::Equal, Ordering::Less) => {
                n.left = Self::insert(n.left.take(), interval);
            }
            _ => {
                n.right = Self::insert(n.right.take(), interval);
            }
        }

        Self::update(&mut n);
        Self::rebalance(n)
    }

    fn rebalance(mut n: Box<Node>) -> Option<Box<Node>> {
        let balance = Self::balance_factor(&n);

        // Left Heavy
        if balance > 1 {
            let left_balance = n.left.as_ref().map_or(0, |l| Self::balance_factor(l));
            if left_balance < 0 {
                // LR case
                n.left = n.left.take().map(Self::rotate_left);
            }
            // LL case
            return Some(Self::rotate_right(n));
        }

        // Right Heavy
        if balance < -1 {
            let right_balance = n.right.as_ref().map_or(0, |r| Self::balance_factor(r));
            if right_balance > 0 {
                // RL case
                n.right = n.right.take().map(Self::rotate_right);
            }
            // RR case
            return Some(Self::rotate_left(n));
        }

        Some(n)
    }

    fn search_contained(node: &Option<Box<Node>>, start: u64, end: u64, result: &mut Vec<ID>) {
        if let Some(n) = node {
            // 左部分木を探索（max_endが条件を満たす場合のみ）
            if Self::max_end(&n.left) >= start {
                Self::search_contained(&n.left, start, end, result);
            }

            // 現在のノードをチェック
            if n.interval.start >= start && n.interval.end <= end {
                result.push(n.interval.id);
            }

            // 右部分木を探索（startが条件を満たす場合のみ）
            if n.interval.start <= end {
                Self::search_contained(&n.right, start, end, result);
            }
        }
    }

    fn inorder(node: &Option<Box<Node>>, result: &mut Vec<Interval>) {
        if let Some(n) = node {
            Self::inorder(&n.left, result);
            result.push(n.interval.clone());
            Self::inorder(&n.right, result);
        }
    }

    fn delete(node: Option<Box<Node>>, id: ID) -> Option<Box<Node>> {
        let mut n = node?;

        // IDで検索（木構造はstart値ベース）
        let found = n.interval.id == id;

        if !found {
            // 左右両方を探索
            n.left = Self::delete(n.left.take(), id);
            n.right = Self::delete(n.right.take(), id);
            Self::update(&mut n);
            return Self::rebalance(n);
        }

        // ノード削除
        match (n.left.take(), n.right.take()) {
            (None, None) => return None,
            (Some(left), None) => return Some(left),
            (None, Some(right)) => return Some(right),
            (Some(left), Some(right)) => {
                // 後継ノードを見つける（右部分木の最小値）
                let (successor_interval, new_right) = Self::extract_min(right);
                n.interval = successor_interval;
                n.left = Some(left);
                n.right = new_right;
            }
        }

        Self::update(&mut n);
        Self::rebalance(n)
    }

    fn extract_min(mut node: Box<Node>) -> (Interval, Option<Box<Node>>) {
        match node.left.take() {
            None => {
                let interval = node.interval.clone();
                (interval, node.right.take())
            }
            Some(left) => {
                let (interval, new_left) = Self::extract_min(left);
                node.left = new_left;
                Self::update(&mut node);
                (interval, Self::rebalance(node))
            }
        }
    }
}

#[derive(Debug)]
pub struct IntervalManager {
    root: Option<Box<Node>>,
}

impl IntervalManager {
    pub fn new() -> Self {
        Self { root: None }
    }

    // O(log n) - AVL木の挿入
    pub fn insert(&mut self, start: u64, end: u64, id: ID) {
        let interval = Interval { start, end, id };
        self.root = Node::insert(self.root.take(), interval);
    }

    // O(k + log n) - kは結果の数
    pub fn get_ids_in_range(&self, start: u64, end: u64) -> Vec<ID> {
        let mut result = Vec::new();
        Node::search_contained(&self.root, start, end, &mut result);
        result
    }

    // O(n) - 全ノード走査
    pub fn get_all_intervals(&self) -> Vec<Interval> {
        let mut result = Vec::new();
        Node::inorder(&self.root, &mut result);
        result
    }

    // O(log n) - AVL木の削除
    pub fn delete_id(&mut self, id: ID) {
        self.root = Node::delete(self.root.take(), id);
    }
}
