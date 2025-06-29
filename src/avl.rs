use std::cmp::{Ord, max};

type Link<K, V> = Option<Box<Node<K, V>>>;

/// Node in the AVL tree.
pub struct Node<K: Ord, V> {
    pub key: K,
    pub value: V,
    height: i32,
    pub left: Link<K, V>,
    pub right: Link<K, V>,
}

/// Generic AVL tree that supports JIT specialization via native compilation.
pub struct AvlTree<K: Ord, V> {
    pub root: Link<K, V>,
}

impl<K, V> Node<K, V>
where
    K: Ord,
{
    fn new(key: K, value: V) -> Self {
        Node {
            key,
            value,
            height: 1,
            left: None,
            right: None,
        }
    }

    fn height(node: &Link<K, V>) -> i32 {
        node.as_ref().map_or(0, |n| n.height)
    }

    fn balance_factor(&self) -> i32 {
        Node::height(&self.left) - Node::height(&self.right)
    }

    fn update_height(&mut self) {
        self.height = 1 + max(Node::height(&self.left), Node::height(&self.right));
    }
}

impl<K: Ord + Copy, V: Copy> AvlTree<K, V> {
    pub fn new() -> Self {
        AvlTree { root: None }
    }

    pub fn lookup(&self, key: &K) -> Option<V> {
        let mut current = &self.root;
        while let Some(node) = current {
            match key.cmp(&node.key) {
                std::cmp::Ordering::Less => current = &node.left,
                std::cmp::Ordering::Greater => current = &node.right,
                std::cmp::Ordering::Equal => return Some(node.value),
            }
        }
        None
    }

    pub fn insert(&mut self, key: K, value: V) {
        self.root = Self::insert_rec(self.root.take(), key, value);
    }

    // Returns the new root of the subtree
    fn insert_rec(mut node: Link<K, V>, key: K, value: V) -> Link<K, V> {
        let mut node = match node.take() {
            Some(n) => n,
            None => return Some(Box::new(Node::new(key, value))),
        };

        match key.cmp(&node.key) {
            std::cmp::Ordering::Less => {
                node.left = Self::insert_rec(node.left.take(), key, value);
            }
            std::cmp::Ordering::Greater => {
                node.right = Self::insert_rec(node.right.take(), key, value);
            }
            std::cmp::Ordering::Equal => {
                // Key already exists, update value
                node.value = value;
                return Some(node);
            }
        }

        node.update_height();
        Self::balance(node)
    }

    fn balance(mut node: Box<Node<K, V>>) -> Link<K, V> {
        let balance = node.balance_factor();

        // Left heavy
        if balance > 1 {
            if node.left.as_ref().unwrap().balance_factor() < 0 {
                node.left = Self::rotate_left(node.left.take().unwrap());
            }
            return Self::rotate_right(node);
        }
        // Right heavy
        if balance < -1 {
            if node.right.as_ref().unwrap().balance_factor() > 0 {
                node.right = Self::rotate_right(node.right.take().unwrap());
            }
            return Self::rotate_left(node);
        }

        Some(node)
    }

    fn rotate_left(mut node: Box<Node<K, V>>) -> Link<K, V> {
        let mut new_root = node.right.take().unwrap();
        node.right = new_root.left.take();
        node.update_height();
        new_root.left = Some(node);
        new_root.update_height();
        Some(new_root)
    }

    fn rotate_right(mut node: Box<Node<K, V>>) -> Link<K, V> {
        let mut new_root = node.left.take().unwrap();
        node.left = new_root.right.take();
        node.update_height();
        new_root.right = Some(node);
        new_root.update_height();
        Some(new_root)
    }

    /// Traverse the tree in pre-order.
    pub fn pre_order(&self) -> Vec<&Node<K, V>> {
        let mut result = Vec::new();
        Self::pre_order_rec(&self.root, &mut result);
        result
    }

    fn pre_order_rec<'a>(node: &'a Link<K, V>, result: &mut Vec<&'a Node<K, V>>) {
        if let Some(n) = node {
            result.push(n);
            Self::pre_order_rec(&n.left, result);
            Self::pre_order_rec(&n.right, result);
        }
    }
}
