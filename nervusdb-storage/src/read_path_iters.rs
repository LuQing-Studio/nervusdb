use crate::csr::CsrSegment;
use crate::idmap::InternalNodeId;
use crate::read_path_neighbors::{
    apply_run_tombstones, edge_blocked_incoming, edge_blocked_outgoing, load_incoming_run_edges,
    load_incoming_segment_edges, load_outgoing_run_edges, load_outgoing_segment_edges,
};
use crate::snapshot::{EdgeKey, L0Run, RelTypeId};
use std::collections::HashSet;
use std::sync::Arc;

pub struct NeighborsIter {
    runs: Arc<Vec<Arc<L0Run>>>,
    segments: Arc<Vec<Arc<CsrSegment>>>,
    src: InternalNodeId,
    rel: Option<RelTypeId>,
    run_idx: usize,
    edge_idx: usize,
    current_edges: Vec<EdgeKey>,
    segment_idx: usize,
    segment_edge_idx: usize,
    current_segment_edges: Vec<EdgeKey>,
    blocked_nodes: HashSet<InternalNodeId>,
    blocked_edges: HashSet<EdgeKey>,
    terminated: bool,
}

impl NeighborsIter {
    pub(crate) fn new(
        runs: Arc<Vec<Arc<L0Run>>>,
        segments: Arc<Vec<Arc<CsrSegment>>>,
        src: InternalNodeId,
        rel: Option<RelTypeId>,
    ) -> Self {
        let base_cap = runs.len().saturating_mul(8).saturating_add(16);
        Self {
            runs,
            segments,
            src,
            rel,
            run_idx: 0,
            edge_idx: 0,
            current_edges: Vec::with_capacity(16),
            segment_idx: 0,
            segment_edge_idx: 0,
            current_segment_edges: Vec::with_capacity(16),
            blocked_nodes: HashSet::with_capacity(base_cap),
            blocked_edges: HashSet::with_capacity(base_cap),
            terminated: false,
        }
    }

    fn load_run(&mut self) {
        self.current_edges.clear();
        self.edge_idx = 0;

        let Some(run) = self.runs.get(self.run_idx) else {
            self.terminated = true;
            return;
        };

        apply_run_tombstones(run, &mut self.blocked_nodes, &mut self.blocked_edges);

        if self.blocked_nodes.contains(&self.src) {
            self.terminated = true;
            return;
        }

        load_outgoing_run_edges(run, self.src, &mut self.current_edges);
    }

    fn load_segment(&mut self) {
        self.current_segment_edges.clear();
        self.segment_edge_idx = 0;

        let Some(seg) = self.segments.get(self.segment_idx) else {
            return;
        };

        load_outgoing_segment_edges(seg, self.src, self.rel, &mut self.current_segment_edges);
    }
}

impl Iterator for NeighborsIter {
    type Item = EdgeKey;

    fn next(&mut self) -> Option<Self::Item> {
        if self.terminated {
            return None;
        }

        loop {
            if self.edge_idx >= self.current_edges.len() {
                if self.run_idx < self.runs.len() {
                    self.load_run();
                    self.run_idx += 1;
                    continue;
                }

                if self.segment_edge_idx >= self.current_segment_edges.len() {
                    if self.segment_idx >= self.segments.len() {
                        self.terminated = true;
                        return None;
                    }
                    self.load_segment();
                    self.segment_idx += 1;
                    continue;
                }

                let edge = self.current_segment_edges[self.segment_edge_idx];
                self.segment_edge_idx += 1;

                if edge_blocked_outgoing(edge, &self.blocked_nodes, &self.blocked_edges) {
                    continue;
                }

                return Some(edge);
            }

            let edge = self.current_edges[self.edge_idx];
            self.edge_idx += 1;

            if let Some(rel) = self.rel
                && edge.rel != rel
            {
                continue;
            }

            if edge_blocked_outgoing(edge, &self.blocked_nodes, &self.blocked_edges) {
                continue;
            }

            return Some(edge);
        }
    }
}

pub struct IncomingNeighborsIter {
    runs: Arc<Vec<Arc<L0Run>>>,
    segments: Arc<Vec<Arc<CsrSegment>>>,
    dst_node: InternalNodeId,
    rel: Option<RelTypeId>,
    run_idx: usize,
    edge_idx: usize,
    current_edges: Vec<EdgeKey>,
    segment_idx: usize,
    segment_edge_idx: usize,
    current_segment_edges: Vec<EdgeKey>,
    blocked_nodes: HashSet<InternalNodeId>,
    blocked_edges: HashSet<EdgeKey>,
    terminated: bool,
}

impl IncomingNeighborsIter {
    pub(crate) fn new(
        runs: Arc<Vec<Arc<L0Run>>>,
        segments: Arc<Vec<Arc<CsrSegment>>>,
        dst_node: InternalNodeId,
        rel: Option<RelTypeId>,
    ) -> Self {
        let base_cap = runs.len().saturating_mul(8).saturating_add(16);
        Self {
            runs,
            segments,
            dst_node,
            rel,
            run_idx: 0,
            edge_idx: 0,
            current_edges: Vec::with_capacity(16),
            segment_idx: 0,
            segment_edge_idx: 0,
            current_segment_edges: Vec::with_capacity(16),
            blocked_nodes: HashSet::with_capacity(base_cap),
            blocked_edges: HashSet::with_capacity(base_cap),
            terminated: false,
        }
    }

    fn load_run(&mut self) {
        self.current_edges.clear();
        self.edge_idx = 0;

        let Some(run) = self.runs.get(self.run_idx) else {
            self.terminated = true;
            return;
        };

        apply_run_tombstones(run, &mut self.blocked_nodes, &mut self.blocked_edges);

        if self.blocked_nodes.contains(&self.dst_node) {
            self.terminated = true;
            return;
        }

        load_incoming_run_edges(run, self.dst_node, &mut self.current_edges);
    }

    fn load_segment(&mut self) {
        self.current_segment_edges.clear();
        self.segment_edge_idx = 0;

        let Some(seg) = self.segments.get(self.segment_idx) else {
            return;
        };

        load_incoming_segment_edges(
            seg,
            self.dst_node,
            self.rel,
            &mut self.current_segment_edges,
        );
    }
}

impl Iterator for IncomingNeighborsIter {
    type Item = EdgeKey;

    fn next(&mut self) -> Option<Self::Item> {
        if self.terminated {
            return None;
        }

        loop {
            if self.edge_idx >= self.current_edges.len() {
                if self.run_idx < self.runs.len() {
                    self.load_run();
                    self.run_idx += 1;
                    continue;
                }

                if self.segment_edge_idx >= self.current_segment_edges.len() {
                    if self.segment_idx >= self.segments.len() {
                        self.terminated = true;
                        return None;
                    }
                    self.load_segment();
                    self.segment_idx += 1;
                    continue;
                }

                let edge = self.current_segment_edges[self.segment_edge_idx];
                self.segment_edge_idx += 1;

                if edge_blocked_incoming(edge, &self.blocked_nodes, &self.blocked_edges) {
                    continue;
                }

                return Some(edge);
            }

            let edge = self.current_edges[self.edge_idx];
            self.edge_idx += 1;

            if let Some(rel) = self.rel
                && edge.rel != rel
            {
                continue;
            }

            if edge_blocked_incoming(edge, &self.blocked_nodes, &self.blocked_edges) {
                continue;
            }

            return Some(edge);
        }
    }
}
