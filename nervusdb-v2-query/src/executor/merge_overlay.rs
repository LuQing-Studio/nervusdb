use super::PropertyValue;
use nervusdb_v2_api::{EdgeKey, InternalNodeId};

#[derive(Clone)]
pub(super) struct MergeOverlayNode {
    pub(super) iid: InternalNodeId,
    pub(super) labels: Vec<String>,
    pub(super) props: std::collections::BTreeMap<String, PropertyValue>,
}

#[derive(Clone)]
pub(super) struct MergeOverlayEdge {
    pub(super) key: EdgeKey,
    pub(super) props: std::collections::BTreeMap<String, PropertyValue>,
}

#[derive(Default)]
pub(super) struct MergeOverlayState {
    pub(super) nodes: Vec<MergeOverlayNode>,
    pub(super) edges: Vec<MergeOverlayEdge>,
}
