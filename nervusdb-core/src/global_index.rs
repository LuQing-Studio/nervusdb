//! Very small in-memory global index facade.

use std::collections::BTreeMap;

use crate::partition::PartitionId;
use crate::triple::Triple;

/// Stores the partition id for each canonical SPO triple.
#[derive(Default, Debug)]
pub struct GlobalIndex {
    spo: BTreeMap<(u64, u64, u64), PartitionId>,
}

impl GlobalIndex {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, triple: Triple, partition: PartitionId) {
        self.spo.insert(
            (triple.subject_id, triple.predicate_id, triple.object_id),
            partition,
        );
    }

    pub fn lookup(&self, triple: &Triple) -> Option<PartitionId> {
        self.spo
            .get(&(triple.subject_id, triple.predicate_id, triple.object_id))
            .copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&(u64, u64, u64), &PartitionId)> {
        self.spo.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_and_lookup() {
        let mut index = GlobalIndex::new();
        let triple = Triple::new(1, 2, 3);
        index.insert(triple, 42);
        assert_eq!(index.lookup(&triple), Some(42));
    }
}
