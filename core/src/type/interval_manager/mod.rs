use std::cmp::{max, Ordering};

#[derive(Debug, Clone)]
struct Interval<T> {
    start: u64,
    end: u64,
    value: T,
}

#[derive(Debug)]
struct Node<T> {
    interval: Interval<T>,
    height: i32,
    max_end: u64,
    left: Option<Box<Node<T>>>,
    right: Option<Box<Node<T>>>,
}

impl<T: Clone + Ord> Node<T> {
    fn new(interval: Interval<T>) -> Self {
        Self {
            max_end: interval.end,
            height: 1,
            interval,
            left: None,
            right: None,
        }
    }

    fn height(node: &Option<Box<Node<T>>>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn max_end(node: &Option<Box<Node<T>>>) -> u64 {
        node.as_ref().map_or(0, |n| n.max_end)
    }

    fn update(node: &mut Box<Node<T>>) {
        node.height = 1 + max(Self::height(&node.left), Self::height(&node.right));
        node.max_end = max(
            node.interval.end,
            max(Self::max_end(&node.left), Self::max_end(&node.right)),
        );
    }

    fn balance_factor(node: &Box<Node<T>>) -> i32 {
        Self::height(&node.left) - Self::height(&node.right)
    }

    fn rotate_right(mut y: Box<Node<T>>) -> Box<Node<T>> {
        let mut x = y.left.take().expect("rotate_right requires left child");
        let t2 = x.right.take();

        y.left = t2;
        Self::update(&mut y);

        x.right = Some(y);
        Self::update(&mut x);
        x
    }

    fn rotate_left(mut x: Box<Node<T>>) -> Box<Node<T>> {
        let mut y = x.right.take().expect("rotate_left requires right child");
        let t2 = y.left.take();

        x.right = t2;
        Self::update(&mut x);

        y.left = Some(x);
        Self::update(&mut y);
        y
    }

    fn insert(node: Option<Box<Node<T>>>, interval: Interval<T>) -> Option<Box<Node<T>>> {
        let mut n = match node {
            None => return Some(Box::new(Node::new(interval))),
            Some(n) => n,
        };

        // start値で比較、同じなら value（ID代わり）で比較
        match (
            interval.start.cmp(&n.interval.start),
            interval.value.cmp(&n.interval.value),
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

    fn rebalance(mut n: Box<Node<T>>) -> Option<Box<Node<T>>> {
        let balance = Self::balance_factor(&n);

        if balance > 1 {
            let left_balance = n.left.as_ref().map_or(0, |l| Self::balance_factor(l));
            if left_balance < 0 {
                n.left = n.left.take().map(Self::rotate_left);
            }
            return Some(Self::rotate_right(n));
        }

        if balance < -1 {
            let right_balance = n.right.as_ref().map_or(0, |r| Self::balance_factor(r));
            if right_balance > 0 {
                n.right = n.right.take().map(Self::rotate_right);
            }
            return Some(Self::rotate_left(n));
        }

        Some(n)
    }

    fn search_contained(node: &Option<Box<Node<T>>>, start: u64, end: u64, result: &mut Vec<T>) {
        if let Some(n) = node {
            if Self::max_end(&n.left) >= start {
                Self::search_contained(&n.left, start, end, result);
            }

            if n.interval.start >= start && n.interval.end <= end {
                result.push(n.interval.value.clone());
            }

            if n.interval.start <= end {
                Self::search_contained(&n.right, start, end, result);
            }
        }
    }

    fn search_overlapping(node: &Option<Box<Node<T>>>, start: u64, end: u64, result: &mut Vec<T>) {
        if let Some(n) = node {
            if Self::max_end(&n.left) >= start {
                Self::search_overlapping(&n.left, start, end, result);
            }

            // ★重なっているか？
            if n.interval.start <= end && n.interval.end >= start {
                result.push(n.interval.value.clone());
            }

            if n.interval.start <= end {
                Self::search_overlapping(&n.right, start, end, result);
            }
        }
    }

    fn inorder(node: &Option<Box<Node<T>>>, result: &mut Vec<Interval<T>>) {
        if let Some(n) = node {
            Self::inorder(&n.left, result);
            result.push(n.interval.clone());
            Self::inorder(&n.right, result);
        }
    }

    fn delete(node: Option<Box<Node<T>>>, value: &T) -> Option<Box<Node<T>>> {
        let mut n = node?;

        let found = &n.interval.value == value;

        if !found {
            n.left = Self::delete(n.left.take(), value);
            n.right = Self::delete(n.right.take(), value);
            Self::update(&mut n);
            return Self::rebalance(n);
        }

        match (n.left.take(), n.right.take()) {
            (None, None) => return None,
            (Some(left), None) => return Some(left),
            (None, Some(right)) => return Some(right),
            (Some(left), Some(right)) => {
                let (successor_interval, new_right) = Self::extract_min(right);
                n.interval = successor_interval;
                n.left = Some(left);
                n.right = new_right;
            }
        }

        Self::update(&mut n);
        Self::rebalance(n)
    }

    fn extract_min(mut node: Box<Node<T>>) -> (Interval<T>, Option<Box<Node<T>>>) {
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
pub struct IntervalManager<T> {
    root: Option<Box<Node<T>>>,
}

impl<T: Clone + Ord> IntervalManager<T> {
    pub fn new() -> Self {
        Self { root: None }
    }

    pub fn insert(&mut self, start: u64, end: u64, value: T) {
        let interval = Interval { start, end, value };
        self.root = Node::insert(self.root.take(), interval);
    }

    pub fn get_ids_in_range(&self, start: u64, end: u64) -> Vec<T> {
        let mut result = Vec::new();
        Node::search_contained(&self.root, start, end, &mut result);
        result
    }

    pub fn get_overlapping_ids(&self, start: u64, end: u64) -> Vec<T> {
        let mut result = Vec::new();
        Node::search_overlapping(&self.root, start, end, &mut result);
        result
    }

    pub fn get_all_intervals(&self) -> Vec<Interval<T>> {
        let mut result = Vec::new();
        Node::inorder(&self.root, &mut result);
        result
    }

    pub fn delete(&mut self, value: &T) {
        self.root = Node::delete(self.root.take(), value);
    }
}
