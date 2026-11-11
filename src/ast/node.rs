use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::Position;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(u64);

impl NodeId {
    pub(super) fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let id = NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .expect("No node identifiers remain.");
        Self(id)
    }
}

#[derive(Debug)]
pub(crate) struct NodeMap<T> {
    values: HashMap<NodeId, T>,
    positions: HashMap<Position, Option<NodeId>>,
}

impl<T> NodeMap<T> {
    pub(crate) fn new() -> Self {
        Self {
            values: HashMap::new(),
            positions: HashMap::new(),
        }
    }

    pub(crate) fn insert(&mut self, id: NodeId, position: Position, value: T) {
        self.values.insert(id, value);
        self.positions
            .entry(position)
            .and_modify(|entry| {
                if *entry != Some(id) {
                    *entry = None;
                }
            })
            .or_insert(Some(id));
    }

    pub(crate) fn get(&self, id: NodeId) -> Option<&T> {
        self.values.get(&id)
    }

    pub(crate) fn at(&self, position: Position) -> Option<&T> {
        self.positions
            .get(&position)
            .copied()
            .flatten()
            .and_then(|id| self.get(id))
    }
}
