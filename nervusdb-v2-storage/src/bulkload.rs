use crate::idmap::{ExternalId, InternalNodeId, LabelId};
use crate::property::PropertyValue;
use crate::{Error, Result};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// A node to be bulk-loaded into the database.
#[derive(Debug, Clone)]
pub struct BulkNode {
    pub external_id: ExternalId,
    pub label: String,
    pub properties: BTreeMap<String, PropertyValue>,
}

/// An edge to be bulk-loaded into the database.
#[derive(Debug, Clone)]
pub struct BulkEdge {
    pub src_external_id: ExternalId,
    pub rel_type: String,
    pub dst_external_id: ExternalId,
    pub properties: BTreeMap<String, PropertyValue>,
}

/// Offline bulk loader for high-performance data import.
///
/// This bypasses WAL and directly generates L1 Segments and IdMap,
/// achieving significantly higher throughput than regular write transactions.
///
/// # Example
///
/// ```ignore
/// let mut loader = BulkLoader::new(PathBuf::from("db.ndb"))?;
///
/// loader.add_node(BulkNode {
///     external_id: 1,
///     label: "Person".to_string(),
///     properties: BTreeMap::from([
///         ("name".to_string(), PropertyValue::String("Alice".to_string())),
///     ]),
/// })?;
///
/// loader.add_edge(BulkEdge {
///     src_external_id: 1,
///     rel_type: "KNOWS".to_string(),
///     dst_external_id: 2,
///     properties: BTreeMap::new(),
/// })?;
///
/// loader.commit()?;
/// ```
pub struct BulkLoader {
    db_path: PathBuf,
    wal_path: PathBuf,
    nodes: Vec<BulkNode>,
    edges: Vec<BulkEdge>,
}

impl BulkLoader {
    /// Creates a new bulk loader for the specified database path.
    ///
    /// The database MUST NOT exist yet. The loader will create a new database
    /// from scratch.
    pub fn new(db_path: PathBuf) -> Result<Self> {
        // Verify database doesn't exist
        if db_path.exists() {
            return Err(Error::WalProtocol(
                "Database file already exists. BulkLoader only works with new databases.",
            ));
        }

        // Derive WAL path
        let wal_path = db_path.with_extension("wal");

        Ok(Self {
            db_path,
            wal_path,
            nodes: Vec::new(),
            edges: Vec::new(),
        })
    }

    /// Adds a node to the bulk load batch.
    ///
    /// Nodes are buffered in memory until `commit()` is called.
    ///
    /// # Errors
    ///
    /// Returns an error if the external_id is not unique.
    pub fn add_node(&mut self, node: BulkNode) -> Result<()> {
        // TODO: Validate uniqueness of external_id
        self.nodes.push(node);
        Ok(())
    }

    /// Adds an edge to the bulk load batch.
    ///
    /// Edges are buffered in memory until `commit()` is called.
    ///
    /// # Errors
    ///
    /// Returns an error if src or dst external_id doesn't reference a node.
    pub fn add_edge(&mut self, edge: BulkEdge) -> Result<()> {
        // TODO: Validate src and dst exist in nodes
        self.edges.push(edge);
        Ok(())
    }

    /// Commits the bulk load, writing all data to disk.
    ///
    /// This performs the following steps:
    /// 1. Validates all data (uniqueness, referential integrity)
    /// 2. Assigns internal IDs to all nodes
    /// 3. Generates L1 Segments from edges
    /// 4. Writes properties to B-Tree
    /// 5. Initializes WAL with manifest
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Validation fails
    /// - Disk I/O fails
    /// - Database constraints are violated
    pub fn commit(self) -> Result<()> {
        // Step 1: Validate data
        self.validate()?;

        // Step 2: Create Pager
        let mut pager = crate::pager::Pager::open(&self.db_path)?;

        // Step 3: Build IdMap and get label mappings
        let (external_to_internal, label_snapshot) = self.build_idmap_and_labels(&mut pager)?;

        // Step 4: Generate L1 Segments
        let segments = self.build_segments(&external_to_internal)?;

        // Step 5: Write segments to pager and get segment pointers
        let segment_pointers = self.write_segments(&mut pager, &segments)?;

        // Step 6: Write properties and get properties_root
        let properties_root = self.write_properties(&mut pager, &external_to_internal)?;

        // Step 7: Collect statistics and get stats_root
        let stats_root = self.write_statistics(&mut pager, &label_snapshot, &segments)?;

        // Step 8: Initialize WAL with manifest
        self.initialize_wal(&segment_pointers, properties_root, stats_root)?;

        Ok(())
    }

    /// Validates all bulk data for consistency.
    fn validate(&self) -> Result<()> {
        // Check external_id uniqueness
        let mut seen_ids = BTreeMap::new();
        for (idx, node) in self.nodes.iter().enumerate() {
            if seen_ids.insert(node.external_id, idx).is_some() {
                return Err(Error::WalProtocol("Duplicate external_id in bulk load"));
            }
        }

        // Build set of valid external IDs for edge validation
        let valid_ids: BTreeMap<ExternalId, ()> =
            self.nodes.iter().map(|n| (n.external_id, ())).collect();

        // Validate edges reference existing nodes
        for edge in &self.edges {
            if !valid_ids.contains_key(&edge.src_external_id) {
                return Err(Error::WalProtocol(
                    "Edge src_external_id references non-existent node",
                ));
            }
            if !valid_ids.contains_key(&edge.dst_external_id) {
                return Err(Error::WalProtocol(
                    "Edge dst_external_id references non-existent node",
                ));
            }
        }

        Ok(())
    }

    /// Builds IdMap and label interner, returns mappings for subsequent steps.
    fn build_idmap_and_labels(
        &self,
        pager: &mut crate::pager::Pager,
    ) -> Result<(BTreeMap<ExternalId, InternalNodeId>, Vec<LabelId>)> {
        use crate::idmap::IdMap;
        use crate::label_interner::LabelInterner;

        // Create label interner
        let mut label_interner = LabelInterner::new();

        // Load IdMap (should be empty for new database)
        let mut idmap = IdMap::load(pager)?;

        // Build external_to_internal mapping
        let mut external_to_internal = BTreeMap::new();
        let mut label_snapshot = Vec::new();

        // Assign internal IDs and register labels
        for (idx, node) in self.nodes.iter().enumerate() {
            let internal_id = idx as InternalNodeId;
            let label_id = label_interner.get_or_create(&node.label);

            external_to_internal.insert(node.external_id, internal_id);
            label_snapshot.push(label_id);

            // Write to IdMap
            idmap.apply_create_node(pager, node.external_id, label_id, internal_id)?;
        }

        // TODO: Persist label interner snapshot to pager metadata
        // For MVP, we rely on IdMap's i2e records which contain label_id

        Ok((external_to_internal, label_snapshot))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_bulkloader_creation() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.ndb");

        let loader = BulkLoader::new(db_path).unwrap();
        assert_eq!(loader.nodes.len(), 0);
        assert_eq!(loader.edges.len(), 0);
    }

    #[test]
    fn test_bulkloader_rejects_existing_db() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.ndb");

        // Create existing file
        std::fs::write(&db_path, b"dummy").unwrap();

        let result = BulkLoader::new(db_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_add_node() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.ndb");

        let mut loader = BulkLoader::new(db_path).unwrap();
        loader
            .add_node(BulkNode {
                external_id: 1,
                label: "Person".to_string(),
                properties: BTreeMap::new(),
            })
            .unwrap();

        assert_eq!(loader.nodes.len(), 1);
    }

    #[test]
    fn test_add_edge() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("test.ndb");

        let mut loader = BulkLoader::new(db_path).unwrap();
        loader
            .add_edge(BulkEdge {
                src_external_id: 1,
                rel_type: "KNOWS".to_string(),
                dst_external_id: 2,
                properties: BTreeMap::new(),
            })
            .unwrap();

        assert_eq!(loader.edges.len(), 1);
    }
}
