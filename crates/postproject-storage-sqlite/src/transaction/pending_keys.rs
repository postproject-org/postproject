use std::collections::{BTreeMap, btree_map::Entry};

use postproject_core::SemanticConflictKey;

// Canonical keys identify immutable semantic keys. Only first insertion needs
// an undo entry; rolling back a nested operation preserves earlier guards.
#[derive(Default)]
pub(super) struct PendingKeys {
    keys: BTreeMap<Vec<u8>, SemanticConflictKey>,
    insertions: Vec<Vec<u8>>,
}

impl PendingKeys {
    pub(super) fn insert(&mut self, encoded: Vec<u8>, key: SemanticConflictKey) {
        if let Entry::Vacant(entry) = self.keys.entry(encoded.clone()) {
            entry.insert(key);
            self.insertions.push(encoded);
        }
    }

    pub(super) fn checkpoint(&self) -> usize {
        self.insertions.len()
    }

    pub(super) fn rollback_to(&mut self, checkpoint: usize) {
        for encoded in self.insertions.drain(checkpoint..) {
            self.keys.remove(&encoded);
        }
    }

    pub(super) fn clear(&mut self) {
        self.keys.clear();
        self.insertions.clear();
    }

    pub(super) fn snapshot(&self) -> BTreeMap<Vec<u8>, SemanticConflictKey> {
        self.keys.clone()
    }
}

#[cfg(test)]
mod tests {
    use postproject_core::MediaRootId;

    use super::*;

    #[test]
    fn nested_rollback_preserves_preexisting_and_successful_outer_keys() {
        let mut keys = PendingKeys::default();
        let first = SemanticConflictKey::MediaRoot(MediaRootId::new());
        let second = SemanticConflictKey::MediaRoot(MediaRootId::new());
        let third = SemanticConflictKey::MediaRoot(MediaRootId::new());
        keys.insert(vec![1], first.clone());
        let outer = keys.checkpoint();
        keys.insert(vec![1], first.clone());
        keys.insert(vec![2], second.clone());
        let inner = keys.checkpoint();
        keys.insert(vec![2], second.clone());
        keys.insert(vec![3], third);
        keys.rollback_to(inner);
        assert_eq!(
            keys.snapshot(),
            BTreeMap::from([(vec![1], first.clone()), (vec![2], second)])
        );
        keys.rollback_to(outer);
        assert_eq!(keys.snapshot(), BTreeMap::from([(vec![1], first)]));
        keys.clear();
        assert_eq!(keys.snapshot(), BTreeMap::new());
        assert_eq!(keys.checkpoint(), 0);
    }
}
