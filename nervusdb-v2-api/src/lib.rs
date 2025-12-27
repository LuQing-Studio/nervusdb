pub type ExternalId = u64;
pub type InternalNodeId = u32;
pub type LabelId = u32;
pub type RelTypeId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EdgeKey {
    pub src: InternalNodeId,
    pub rel: RelTypeId,
    pub dst: InternalNodeId,
}

pub trait GraphStore {
    type Snapshot: GraphSnapshot;

    fn snapshot(&self) -> Self::Snapshot;
}

pub trait GraphSnapshot {
    type Neighbors<'a>: Iterator<Item = EdgeKey> + 'a
    where
        Self: 'a;

    fn neighbors(&self, src: InternalNodeId, rel: Option<RelTypeId>) -> Self::Neighbors<'_>;

    fn nodes(&self) -> Box<dyn Iterator<Item = InternalNodeId> + '_> {
        Box::new(std::iter::empty())
    }

    fn resolve_external(&self, _iid: InternalNodeId) -> Option<ExternalId> {
        None
    }

    fn node_label(&self, _iid: InternalNodeId) -> Option<LabelId> {
        None
    }

    fn is_tombstoned_node(&self, _iid: InternalNodeId) -> bool {
        false
    }
}
