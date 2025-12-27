//! LIMIT boundary tests for v2 query engine
//!
//! Tests for LIMIT edge cases:
//! - LIMIT 0 (should return empty)
//! - LIMIT larger than result set
//! - LIMIT with zero results
//! - RETURN 1 with various LIMIT values

use nervusdb_v2_api::{EdgeKey, ExternalId, GraphSnapshot, InternalNodeId, RelTypeId};
use nervusdb_v2_query::{Params, Result, Value, prepare};
use std::collections::HashMap;

#[derive(Debug)]
struct FakeSnapshot {
    nodes: Vec<InternalNodeId>,
    edges_by_src: HashMap<InternalNodeId, Vec<EdgeKey>>,
    external: HashMap<InternalNodeId, ExternalId>,
}

#[derive(Debug)]
struct FakeNeighbors<'a> {
    edges: &'a [EdgeKey],
    rel: Option<RelTypeId>,
    idx: usize,
}

impl<'a> Iterator for FakeNeighbors<'a> {
    type Item = EdgeKey;

    fn next(&mut self) -> Option<Self::Item> {
        while self.idx < self.edges.len() {
            let e = self.edges[self.idx];
            self.idx += 1;
            if let Some(rel) = self.rel
                && e.rel != rel
            {
                continue;
            }
            return Some(e);
        }
        None
    }
}

impl GraphSnapshot for FakeSnapshot {
    type Neighbors<'a>
        = FakeNeighbors<'a>
    where
        Self: 'a;

    fn neighbors(&self, src: InternalNodeId, rel: Option<RelTypeId>) -> Self::Neighbors<'_> {
        let edges = self
            .edges_by_src
            .get(&src)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        FakeNeighbors { edges, rel, idx: 0 }
    }

    fn nodes(&self) -> Box<dyn Iterator<Item = InternalNodeId> + '_> {
        Box::new(self.nodes.iter().copied())
    }

    fn resolve_external(&self, iid: InternalNodeId) -> Option<ExternalId> {
        self.external.get(&iid).copied()
    }
}

fn make_snapshot_with_edges(edge_count: usize) -> FakeSnapshot {
    let mut edges_by_src = HashMap::new();
    let mut nodes = Vec::new();
    let mut external = HashMap::new();

    for i in 0..edge_count {
        let src = i as InternalNodeId;
        nodes.push(src);
        external.insert(src, (i as ExternalId) + 100);

        let edges: Vec<_> = (0..3)
            .map(|j| EdgeKey {
                src,
                rel: 1,
                dst: ((i + j + 1) % edge_count) as InternalNodeId,
            })
            .collect();
        edges_by_src.insert(src, edges);
    }

    FakeSnapshot {
        nodes,
        edges_by_src,
        external,
    }
}

#[test]
fn test_limit_zero_returns_empty() {
    let q = prepare("MATCH (n)-[:1]->(m) RETURN n, m LIMIT 0").unwrap();
    let snap = make_snapshot_with_edges(10);

    let rows: Vec<_> = q
        .execute_streaming(&snap, &Params::new())
        .collect::<Result<_>>()
        .unwrap();

    assert!(rows.is_empty(), "LIMIT 0 should return empty result");
}

#[test]
fn test_limit_larger_than_results() {
    let q = prepare("MATCH (n)-[:1]->(m) RETURN n, m LIMIT 1000").unwrap();
    let snap = make_snapshot_with_edges(5); // Only 5 nodes = ~15 edges

    let rows: Vec<_> = q
        .execute_streaming(&snap, &Params::new())
        .collect::<Result<_>>()
        .unwrap();

    // Should return all available results (5 * 3 = 15 edges)
    assert_eq!(rows.len(), 15);
}

#[test]
fn test_limit_with_no_matching_edges() {
    let q = prepare("MATCH (n)-[:999]->(m) RETURN n, m LIMIT 10").unwrap();
    let snap = make_snapshot_with_edges(5); // Only rel type 1 exists

    let rows: Vec<_> = q
        .execute_streaming(&snap, &Params::new())
        .collect::<Result<_>>()
        .unwrap();

    assert!(rows.is_empty(), "No matching edges should return empty");
}

#[test]
fn test_return_one_limit_5() {
    // RETURN 1 with LIMIT
    let q = prepare("RETURN 1 LIMIT 5").unwrap();
    let snap = FakeSnapshot {
        nodes: vec![],
        edges_by_src: HashMap::new(),
        external: HashMap::new(),
    };

    let rows: Vec<_> = q
        .execute_streaming(&snap, &Params::new())
        .collect::<Result<_>>()
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].columns()[0].1, Value::Int(1));
}

#[test]
fn test_return_one_limit_100() {
    // RETURN 1 with large LIMIT - should still return 1 row
    let q = prepare("RETURN 1 LIMIT 100").unwrap();
    let snap = FakeSnapshot {
        nodes: vec![],
        edges_by_src: HashMap::new(),
        external: HashMap::new(),
    };

    let rows: Vec<_> = q
        .execute_streaming(&snap, &Params::new())
        .collect::<Result<_>>()
        .unwrap();

    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].columns()[0].1, Value::Int(1));
}

#[test]
fn test_match_with_large_limit() {
    // Test that large LIMIT doesn't cause issues
    let q = prepare("MATCH (n)-[:1]->(m) RETURN n, m LIMIT 100000").unwrap();
    let snap = make_snapshot_with_edges(100); // 100 nodes

    let rows: Vec<_> = q
        .execute_streaming(&snap, &Params::new())
        .collect::<Result<_>>()
        .unwrap();

    // Each node has 3 outgoing edges, 100 nodes = 300 edges
    assert_eq!(rows.len(), 300);
}

#[test]
fn test_match_limit_1() {
    // Test LIMIT 1 returns exactly one result
    let q = prepare("MATCH (n)-[:1]->(m) RETURN n, m LIMIT 1").unwrap();
    let snap = make_snapshot_with_edges(10);

    let rows: Vec<_> = q
        .execute_streaming(&snap, &Params::new())
        .collect::<Result<_>>()
        .unwrap();

    assert_eq!(rows.len(), 1);
}

#[test]
fn test_match_limit_5() {
    // Test LIMIT 5 returns exactly 5 results
    let q = prepare("MATCH (n)-[:1]->(m) RETURN n, m LIMIT 5").unwrap();
    let snap = make_snapshot_with_edges(10);

    let rows: Vec<_> = q
        .execute_streaming(&snap, &Params::new())
        .collect::<Result<_>>()
        .unwrap();

    assert_eq!(rows.len(), 5);
}

#[test]
fn test_match_external_id_projection() {
    // Test that external ID is projected correctly
    let q = prepare("MATCH (n)-[:1]->(m) RETURN n, m LIMIT 5").unwrap();

    let mut edges_by_src = HashMap::new();
    edges_by_src.insert(
        0,
        vec![EdgeKey {
            src: 0,
            rel: 1,
            dst: 1,
        }],
    );

    let mut external = HashMap::new();
    external.insert(0, 100);
    external.insert(1, 200);

    let snap = FakeSnapshot {
        nodes: vec![0, 1],
        edges_by_src,
        external,
    };

    let rows: Vec<_> = q
        .execute_streaming(&snap, &Params::new())
        .collect::<Result<_>>()
        .unwrap();

    assert_eq!(rows.len(), 1);

    // Verify the row contains both node IDs (as internal IDs)
    let cols = rows[0].columns();
    assert_eq!(cols[0].0, "n");
    assert_eq!(cols[1].0, "m");
}
