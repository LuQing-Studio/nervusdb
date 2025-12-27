use crate::engine::GraphEngine;
use crate::idmap::I2eRecord;
use crate::snapshot;
use nervusdb_v2_api::{
    EdgeKey, ExternalId, GraphSnapshot, GraphStore, InternalNodeId, LabelId, RelTypeId,
};
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct StorageSnapshot {
    inner: snapshot::Snapshot,
    i2e: Arc<Vec<I2eRecord>>,
    tombstoned_nodes: Arc<HashSet<InternalNodeId>>,
}

impl GraphStore for GraphEngine {
    type Snapshot = StorageSnapshot;

    fn snapshot(&self) -> Self::Snapshot {
        let i2e = Arc::new(
            self.scan_i2e_records()
                .expect("scan_i2e_records must succeed after open()"),
        );
        let inner = self.begin_read();
        let tombstoned_nodes: HashSet<InternalNodeId> = inner
            .runs()
            .iter()
            .flat_map(|r| r.iter_tombstoned_nodes())
            .collect();
        StorageSnapshot {
            inner,
            i2e,
            tombstoned_nodes: Arc::new(tombstoned_nodes),
        }
    }
}

impl GraphSnapshot for StorageSnapshot {
    type Neighbors<'a>
        = std::iter::Map<snapshot::NeighborsIter, fn(snapshot::EdgeKey) -> EdgeKey>
    where
        Self: 'a;

    fn neighbors(&self, src: InternalNodeId, rel: Option<RelTypeId>) -> Self::Neighbors<'_> {
        fn conv(e: snapshot::EdgeKey) -> EdgeKey {
            EdgeKey {
                src: e.src,
                rel: e.rel,
                dst: e.dst,
            }
        }
        self.inner
            .neighbors(src, rel)
            .map(conv as fn(snapshot::EdgeKey) -> EdgeKey)
    }

    fn nodes(&self) -> Box<dyn Iterator<Item = InternalNodeId> + '_> {
        let len = self.i2e.len();
        let tombstoned = self.tombstoned_nodes.clone();
        Box::new((0..len).filter_map(move |iid_usize| {
            let iid = u32::try_from(iid_usize).ok()?;
            if tombstoned.contains(&iid) {
                return None;
            }
            Some(iid)
        }))
    }

    fn resolve_external(&self, iid: InternalNodeId) -> Option<ExternalId> {
        let r = self.i2e.get(iid as usize)?;
        if r.external_id == 0 {
            None
        } else {
            Some(r.external_id)
        }
    }

    fn node_label(&self, iid: InternalNodeId) -> Option<LabelId> {
        self.i2e.get(iid as usize).map(|r| r.label_id)
    }

    fn is_tombstoned_node(&self, iid: InternalNodeId) -> bool {
        self.tombstoned_nodes.contains(&iid)
    }
}
