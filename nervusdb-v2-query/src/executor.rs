use crate::ast::{
    AggregateFunction, Direction, Expression, PathElement, Pattern, RelationshipDirection,
};
use crate::error::{Error, Result};
use crate::evaluator::evaluate_expression_value;
mod binding_utils;
mod create_delete_ops;
mod foreach_ops;
mod index_seek_plan;
mod join_apply;
mod label_constraint;
mod match_bound_rel_plan;
mod match_in_undirected_plan;
mod match_out_plan;
mod merge_execute_support;
mod merge_execution;
mod merge_helpers;
mod merge_overlay;
mod path_usage;
mod plan_head;
mod plan_iterators;
mod plan_mid;
mod plan_tail;
mod procedure_registry;
mod projection_sort;
mod property_bridge;
mod read_path;
mod txn_engine_impl;
mod write_dispatch;
mod write_orchestration;
mod write_path;
mod write_support;
use binding_utils::{
    apply_optional_unbinds_row, row_contains_all_bindings, row_matches_node_binding,
};
use join_apply::{ApplyIter, ProcedureCallIter};
use label_constraint::{LabelConstraint, node_matches_label_constraint, resolve_label_constraint};
use merge_overlay::{MergeOverlayEdge, MergeOverlayNode, MergeOverlayState};
pub use nervusdb_v2_api::LabelId;
use nervusdb_v2_api::{EdgeKey, ExternalId, GraphSnapshot, InternalNodeId, RelTypeId};
use path_usage::{edge_multiplicity, path_alias_contains_edge};
use plan_iterators::{CartesianProductIter, FilterIter, NodeScanIter};
use projection_sort::execute_aggregate;
use property_bridge::{
    api_property_map_to_storage, merge_props_to_values, merge_storage_property_to_api,
};
use serde::ser::{SerializeMap, SerializeSeq};
use std::hash::{Hash, Hasher};
use write_path::{
    apply_label_overlay_to_rows, apply_removed_property_overlay_to_rows,
    apply_set_map_overlay_to_rows, apply_set_property_overlay_to_rows, execute_remove,
    execute_remove_labels, execute_set, execute_set_from_maps, execute_set_labels,
};

const UNLABELED_LABEL_ID: LabelId = LabelId::MAX;
pub use procedure_registry::{
    ErasedSnapshot, Procedure, ProcedureRegistry, get_procedure_registry,
};

#[derive(Debug, Clone, PartialEq, PartialOrd, Hash)]
pub struct NodeValue {
    pub id: InternalNodeId,
    pub labels: Vec<String>,
    pub properties: std::collections::BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Hash)]
pub struct RelationshipValue {
    pub key: EdgeKey,
    pub rel_type: String,
    pub properties: std::collections::BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Hash)]
pub struct PathValue {
    pub nodes: Vec<InternalNodeId>,
    pub edges: Vec<EdgeKey>,
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Hash)]
pub struct ReifiedPathValue {
    pub nodes: Vec<NodeValue>,
    pub relationships: Vec<RelationshipValue>,
}

impl serde::Serialize for NodeValue {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("id", &self.id)?;
        map.serialize_entry("labels", &self.labels)?;
        map.serialize_entry("properties", &self.properties)?;
        map.end()
    }
}

impl serde::Serialize for RelationshipValue {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(4))?;
        map.serialize_entry("src", &self.key.src)?;
        map.serialize_entry("rel", &self.key.rel)?;
        map.serialize_entry("dst", &self.key.dst)?;
        map.serialize_entry("properties", &self.properties)?;
        map.end()
    }
}

impl serde::Serialize for PathValue {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("nodes", &self.nodes)?;
        map.serialize_entry("edges", &self.edges)?;
        map.end()
    }
}

impl serde::Serialize for ReifiedPathValue {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("nodes", &self.nodes)?;
        map.serialize_entry("relationships", &self.relationships)?;
        map.end()
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Value {
    NodeId(InternalNodeId),
    ExternalId(ExternalId),
    EdgeKey(EdgeKey),
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Null,
    List(Vec<Value>),
    DateTime(i64),
    Blob(Vec<u8>),
    Map(std::collections::BTreeMap<String, Value>),
    Path(PathValue),
    Node(NodeValue),
    Relationship(RelationshipValue),
    ReifiedPath(ReifiedPathValue),
}

impl serde::Serialize for Value {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        match self {
            Value::NodeId(iid) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "node_id")?;
                map.serialize_entry("id", iid)?;
                map.end()
            }
            Value::ExternalId(id) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "external_id")?;
                map.serialize_entry("id", id)?;
                map.end()
            }
            Value::EdgeKey(e) => {
                let mut map = serializer.serialize_map(Some(4))?;
                map.serialize_entry("type", "edge_key")?;
                map.serialize_entry("src", &e.src)?;
                map.serialize_entry("rel", &e.rel)?;
                map.serialize_entry("dst", &e.dst)?;
                map.end()
            }
            Value::Int(i) => serializer.serialize_i64(*i),
            Value::Float(f) => serializer.serialize_f64(*f),
            Value::String(s) => serializer.serialize_str(s),
            Value::Bool(b) => serializer.serialize_bool(*b),
            Value::Null => serializer.serialize_none(),
            Value::List(list) => {
                let mut seq = serializer.serialize_seq(Some(list.len()))?;
                for item in list {
                    seq.serialize_element(item)?;
                }
                seq.end()
            }
            Value::DateTime(i) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "datetime")?;
                map.serialize_entry("value", i)?;
                map.end()
            }
            Value::Blob(_) => {
                let mut map = serializer.serialize_map(Some(2))?;
                map.serialize_entry("type", "blob")?;
                map.serialize_entry("data", "<binary>")?;
                map.end()
            }
            Value::Map(map) => {
                let mut ser = serializer.serialize_map(Some(map.len()))?;
                for (k, v) in map {
                    ser.serialize_entry(k, v)?;
                }
                ser.end()
            }
            Value::Path(p) => {
                let mut map = serializer.serialize_map(Some(3))?;
                map.serialize_entry("type", "path")?;
                map.serialize_entry("nodes", &p.nodes)?;
                map.serialize_entry("edges", &p.edges)?;
                map.end()
            }
            Value::Node(n) => {
                let mut map = serializer.serialize_map(Some(3))?;
                map.serialize_entry("type", "node")?;
                map.serialize_entry("id", &n.id)?;
                map.serialize_entry("labels", &n.labels)?;
                map.end()
            }
            Value::Relationship(r) => {
                let mut map = serializer.serialize_map(Some(4))?;
                map.serialize_entry("type", "relationship")?;
                map.serialize_entry("src", &r.key.src)?;
                map.serialize_entry("rel", &r.key.rel)?;
                map.serialize_entry("dst", &r.key.dst)?;
                map.end()
            }
            Value::ReifiedPath(p) => {
                let mut map = serializer.serialize_map(Some(3))?;
                map.serialize_entry("type", "reified_path")?;
                map.serialize_entry("nodes", &p.nodes)?;
                map.serialize_entry("relationships", &p.relationships)?;
                map.end()
            }
        }
    }
}

impl Value {
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn reify(&self, snapshot: &dyn ErasedSnapshot) -> Result<Value> {
        match self {
            Value::NodeId(id) => {
                let mut labels = Vec::new();
                if let Some(label_ids) = snapshot.resolve_node_labels_erased(*id) {
                    for lid in label_ids {
                        if let Some(name) = snapshot.resolve_label_name_erased(lid) {
                            labels.push(name);
                        }
                    }
                }

                let mut properties = std::collections::BTreeMap::new();
                if let Some(props) = snapshot.node_properties_erased(*id) {
                    for (k, v) in props {
                        properties.insert(k, convert_api_property_to_value(&v));
                    }
                }

                Ok(Value::Node(NodeValue {
                    id: *id,
                    labels,
                    properties,
                }))
            }
            Value::EdgeKey(key) => {
                let rel_type = snapshot
                    .resolve_rel_type_name_erased(key.rel)
                    .unwrap_or_else(|| format!("<{}>", key.rel));

                let mut properties = std::collections::BTreeMap::new();
                if let Some(props) = snapshot.edge_properties_erased(*key) {
                    for (k, v) in props {
                        properties.insert(k, convert_api_property_to_value(&v));
                    }
                }

                Ok(Value::Relationship(RelationshipValue {
                    key: *key,
                    rel_type,
                    properties,
                }))
            }
            Value::Path(p) => {
                let mut nodes = Vec::new();
                for nid in &p.nodes {
                    if let Value::Node(n) = Value::NodeId(*nid).reify(snapshot)? {
                        nodes.push(n);
                    }
                }
                let mut relationships = Vec::new();
                for ekey in &p.edges {
                    if let Value::Relationship(r) = Value::EdgeKey(*ekey).reify(snapshot)? {
                        relationships.push(r);
                    }
                }
                Ok(Value::ReifiedPath(ReifiedPathValue {
                    nodes,
                    relationships,
                }))
            }
            Value::List(l) => {
                let mut out = Vec::new();
                for v in l {
                    out.push(v.reify(snapshot)?);
                }
                Ok(Value::List(out))
            }
            Value::Map(m) => {
                let mut out = std::collections::BTreeMap::new();
                for (k, v) in m {
                    out.insert(k.clone(), v.reify(snapshot)?);
                }
                Ok(Value::Map(out))
            }
            _ => Ok(self.clone()),
        }
    }
}

// Custom Hash implementation for Value (since Float doesn't implement Hash)
impl Hash for Value {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Value::NodeId(id) => id.hash(state),
            Value::ExternalId(ext) => ext.hash(state),
            Value::EdgeKey(key) => key.hash(state),
            Value::Int(i) => i.hash(state),
            Value::Float(f) => {
                // Hash by bit pattern for consistency
                f.to_bits().hash(state);
            }
            Value::String(s) => s.hash(state),
            Value::Bool(b) => b.hash(state),
            Value::Null => 0u8.hash(state),
            Value::List(l) => l.hash(state),
            Value::DateTime(i) => i.hash(state),
            Value::Blob(b) => b.hash(state),
            Value::Map(m) => m.hash(state),
            Value::Path(p) => p.hash(state),
            Value::Node(n) => n.hash(state),
            Value::Relationship(r) => r.hash(state),
            Value::ReifiedPath(p) => p.hash(state),
        }
    }
}

impl Eq for Value {}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Row {
    // Small row: linear search is fine for MVP.
    cols: Vec<(String, Value)>,
}

impl Row {
    pub fn new(cols: Vec<(String, Value)>) -> Self {
        Self { cols }
    }

    pub fn reify(&self, snapshot: &dyn ErasedSnapshot) -> Result<Row> {
        let mut cols = Vec::with_capacity(self.cols.len());
        for (k, v) in &self.cols {
            cols.push((k.clone(), v.reify(snapshot)?));
        }
        Ok(Row { cols })
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.cols.iter().find(|(k, _)| k == name).map(|(_, v)| v)
    }

    pub fn with(mut self, name: impl Into<String>, value: Value) -> Self {
        let name = name.into();
        if let Some((_k, v)) = self.cols.iter_mut().find(|(k, _)| *k == name) {
            *v = value;
        } else {
            self.cols.push((name, value));
        }
        self
    }

    pub fn get_node(&self, name: &str) -> Option<InternalNodeId> {
        self.cols.iter().find_map(|(k, v)| {
            if k == name {
                match v {
                    Value::NodeId(iid) => Some(*iid),
                    Value::Node(node) => Some(node.id),
                    _ => None,
                }
            } else {
                None
            }
        })
    }

    pub fn get_edge(&self, name: &str) -> Option<EdgeKey> {
        self.cols.iter().find_map(|(k, v)| {
            if k == name {
                match v {
                    Value::EdgeKey(e) => Some(*e),
                    Value::Relationship(rel) => Some(rel.key),
                    _ => None,
                }
            } else {
                None
            }
        })
    }

    pub fn project(&self, names: &[&str]) -> Row {
        let mut out = Row::default();
        for &name in names {
            if let Some((k, v)) = self.cols.iter().find(|(k, _)| k == name) {
                out.cols.push((k.clone(), v.clone()));
            } else {
                out.cols.push((name.to_string(), Value::Null));
            }
        }
        out
    }

    pub fn columns(&self) -> &[(String, Value)] {
        &self.cols
    }

    pub fn join(&self, other: &Row) -> Row {
        let mut out = self.clone();
        out.cols.extend(other.cols.clone());
        out
    }

    pub fn join_path(
        &mut self,
        alias: &str,
        src: InternalNodeId,
        edge: EdgeKey,
        dst: InternalNodeId,
    ) {
        let path = match self.get(alias) {
            Some(Value::Path(p)) => {
                let mut p = p.clone();
                p.edges.push(edge);
                p.nodes.push(dst);
                Value::Path(p)
            }
            _ => {
                // Initialize path
                Value::Path(PathValue {
                    nodes: vec![src, dst],
                    edges: vec![edge],
                })
            }
        };
        self.with_mut(alias, path);
    }

    fn with_mut(&mut self, name: &str, value: Value) {
        if let Some((_, v)) = self.cols.iter_mut().find(|(k, _)| k == name) {
            *v = value;
        } else {
            self.cols.push((name.to_string(), value));
        }
    }
}

#[derive(Debug, Clone)]
pub enum Plan {
    /// `RETURN 1`
    ReturnOne,
    /// `MATCH (n) RETURN ...`
    NodeScan {
        alias: String,
        label: Option<String>,
        optional: bool,
    },
    /// `MATCH (a)-[:rel]->(b) RETURN ...`
    MatchOut {
        input: Option<Box<Plan>>,
        src_alias: String,
        rels: Vec<String>,
        edge_alias: Option<String>,
        dst_alias: String,
        dst_labels: Vec<String>,
        src_prebound: bool,
        limit: Option<u32>,
        // Note: project is kept for backward compatibility but projection
        // should happen after filtering (see Plan::Project)
        project: Vec<String>,
        project_external: bool,
        optional: bool,
        optional_unbind: Vec<String>,
        path_alias: Option<String>,
    },
    /// `MATCH (a)-[:rel*min..max]->(b) RETURN ...` (variable length)
    MatchOutVarLen {
        input: Option<Box<Plan>>,
        src_alias: String,
        rels: Vec<String>,
        edge_alias: Option<String>,
        dst_alias: String,
        dst_labels: Vec<String>,
        src_prebound: bool,
        direction: RelationshipDirection,
        min_hops: u32,
        max_hops: Option<u32>,
        limit: Option<u32>,
        project: Vec<String>,
        project_external: bool,
        optional: bool,
        optional_unbind: Vec<String>,
        path_alias: Option<String>,
    },
    MatchIn {
        input: Option<Box<Plan>>,
        src_alias: String,
        rels: Vec<String>,
        edge_alias: Option<String>,
        dst_alias: String,
        dst_labels: Vec<String>,
        src_prebound: bool,
        limit: Option<u32>,
        optional: bool,
        optional_unbind: Vec<String>,
        path_alias: Option<String>,
    },
    MatchUndirected {
        input: Option<Box<Plan>>,
        src_alias: String,
        rels: Vec<String>,
        edge_alias: Option<String>,
        dst_alias: String,
        dst_labels: Vec<String>,
        src_prebound: bool,
        limit: Option<u32>,
        optional: bool,
        optional_unbind: Vec<String>,
        path_alias: Option<String>,
    },
    MatchBoundRel {
        input: Box<Plan>,
        rel_alias: String,
        src_alias: String,
        dst_alias: String,
        dst_labels: Vec<String>,
        src_prebound: bool,
        rels: Vec<String>,
        direction: RelationshipDirection,
        optional: bool,
        optional_unbind: Vec<String>,
        path_alias: Option<String>,
    },
    /// `MATCH ... WHERE ... RETURN ...` (with filter)
    Filter {
        input: Box<Plan>,
        predicate: Expression,
    },
    /// 修复 OPTIONAL MATCH + WHERE 语义：按外层行回填 null 行
    OptionalWhereFixup {
        outer: Box<Plan>,
        filtered: Box<Plan>,
        null_aliases: Vec<String>,
    },
    /// Project expressions to new variables
    Project {
        input: Box<Plan>,
        projections: Vec<(String, Expression)>, // (Result/Alias Name, Expression to Eval)
    },
    /// Aggregation: COUNT, SUM, AVG with optional grouping
    Aggregate {
        input: Box<Plan>,
        group_by: Vec<String>,                        // Variables to group by
        aggregates: Vec<(AggregateFunction, String)>, // (Function, Alias)
    },
    /// `ORDER BY` - sort results
    OrderBy {
        input: Box<Plan>,
        items: Vec<(Expression, Direction)>, // (Expression to sort by, ASC|DESC)
    },
    /// `SKIP` - skip first n rows
    Skip {
        input: Box<Plan>,
        skip: u32,
    },
    /// `LIMIT` - limit result count
    Limit {
        input: Box<Plan>,
        limit: u32,
    },
    /// `RETURN DISTINCT` - deduplicate results
    Distinct {
        input: Box<Plan>,
    },
    /// `UNWIND` - expand a list into multiple rows
    Unwind {
        input: Box<Plan>,
        expression: Expression,
        alias: String,
    },
    /// `UNION` / `UNION ALL` - combine results from two queries
    Union {
        left: Box<Plan>,
        right: Box<Plan>,
        all: bool, // true = UNION ALL (keep duplicates), false = UNION (distinct)
    },

    /// `DELETE` - delete nodes/edges (with input plan for variable resolution)
    Delete {
        input: Box<Plan>,
        detach: bool,
        expressions: Vec<Expression>,
    },
    /// `SetProperty` - update properties on nodes/edges
    SetProperty {
        input: Box<Plan>,
        items: Vec<(String, String, Expression)>, // (variable, key, value_expression)
    },
    /// `SET n = {...}` / `SET n += {...}` - replace or append properties from map expression
    SetPropertiesFromMap {
        input: Box<Plan>,
        items: Vec<(String, Expression, bool)>, // (variable, map_expression, append)
    },
    /// `SET n:Label` - add labels on nodes
    SetLabels {
        input: Box<Plan>,
        items: Vec<(String, Vec<String>)>, // (variable, labels)
    },
    /// `REMOVE n.prop` - remove properties from nodes/edges
    RemoveProperty {
        input: Box<Plan>,
        items: Vec<(String, String)>, // (variable, key)
    },
    /// `REMOVE n:Label` - remove labels from nodes
    RemoveLabels {
        input: Box<Plan>,
        items: Vec<(String, Vec<String>)>, // (variable, labels)
    },
    /// `IndexSeek` - optimize scan using index if available, else fallback
    IndexSeek {
        alias: String,
        label: String,
        field: String,
        value_expr: Expression,
        fallback: Box<Plan>,
    },
    /// `CartesianProduct` - multiply two plans (join without shared variables)
    CartesianProduct {
        left: Box<Plan>,
        right: Box<Plan>,
    },
    /// `Apply` - execute subquery for each row (Correlated Subquery)
    Apply {
        input: Box<Plan>,
        subquery: Box<Plan>,
        alias: Option<String>, // Optional alias for subquery result? usually subquery projects...
    },
    /// `CALL namespace.name(args) YIELD x, y`
    ProcedureCall {
        input: Box<Plan>,
        name: Vec<String>,
        args: Vec<Expression>,
        yields: Vec<(String, Option<String>)>, // (field_name, alias)
    },
    Foreach {
        input: Box<Plan>,
        variable: String,
        list: Expression,
        sub_plan: Box<Plan>,
    },
    // Injects specific rows into the pipeline (used for FOREACH context and constructing literal rows)
    Values {
        rows: Vec<Row>,
    },
    Create {
        input: Box<Plan>,
        pattern: Pattern,
    },
}

pub enum PlanIterator<'a, S: GraphSnapshot> {
    ReturnOne(std::iter::Once<Result<Row>>),
    NodeScan(NodeScanIter<'a, S>),
    Filter(FilterIter<'a, S>),
    CartesianProduct(Box<CartesianProductIter<'a, S>>),
    Apply(Box<ApplyIter<'a, S>>),
    ProcedureCall(Box<ProcedureCallIter<'a, S>>),

    Dynamic(Box<dyn Iterator<Item = Result<Row>> + 'a>),
}

impl<'a, S: GraphSnapshot> Iterator for PlanIterator<'a, S> {
    type Item = Result<Row>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            PlanIterator::ReturnOne(iter) => iter.next(),
            PlanIterator::NodeScan(iter) => iter.next(),
            PlanIterator::Filter(iter) => iter.next(),
            PlanIterator::CartesianProduct(iter) => iter.next(),
            PlanIterator::Apply(iter) => iter.next(),
            PlanIterator::ProcedureCall(iter) => iter.next(),

            PlanIterator::Dynamic(iter) => iter.next(),
        }
    }
}

pub fn execute_plan<'a, S: GraphSnapshot + 'a>(
    snapshot: &'a S,
    plan: &'a Plan,
    params: &'a crate::query_api::Params,
) -> PlanIterator<'a, S> {
    match plan {
        Plan::ReturnOne => PlanIterator::ReturnOne(std::iter::once(Ok(Row::default()))),
        Plan::CartesianProduct { left, right } => {
            plan_head::execute_cartesian_product(snapshot, left, right, params)
        }
        Plan::Apply {
            input,
            subquery,
            alias: _,
        } => plan_head::execute_apply(snapshot, input, subquery, params),
        Plan::ProcedureCall {
            input,
            name,
            args,
            yields,
        } => plan_head::execute_procedure_call(snapshot, input, name, args, yields, params),
        Plan::Foreach { .. } => plan_head::write_only_foreach_error(),
        Plan::NodeScan {
            alias,
            label,
            optional,
        } => plan_head::execute_node_scan(snapshot, alias, label, *optional),
        Plan::MatchOut {
            input,
            src_alias,
            rels,
            edge_alias,
            dst_alias,
            dst_labels,
            src_prebound,
            limit,
            project: _,
            project_external: _,
            optional,
            optional_unbind,
            path_alias,
        } => match_out_plan::execute_match_out(
            snapshot,
            input,
            src_alias,
            rels,
            edge_alias,
            dst_alias,
            dst_labels,
            *src_prebound,
            *limit,
            *optional,
            optional_unbind,
            path_alias,
            params,
        ),
        Plan::MatchOutVarLen {
            input,
            src_alias,
            rels,
            edge_alias,
            dst_alias,
            dst_labels,
            src_prebound,
            direction,
            min_hops,
            max_hops,
            limit,
            project: _,
            project_external: _,
            optional,
            optional_unbind,
            path_alias,
        } => match_out_plan::execute_match_out_var_len(
            snapshot,
            input,
            src_alias,
            rels,
            edge_alias,
            dst_alias,
            dst_labels,
            *src_prebound,
            direction,
            *min_hops,
            *max_hops,
            *limit,
            *optional,
            optional_unbind,
            path_alias,
            params,
        ),
        Plan::MatchIn {
            input,
            src_alias,
            rels,
            edge_alias,
            dst_alias,
            dst_labels,
            src_prebound,
            limit: _,
            optional,
            optional_unbind,
            path_alias,
        } => match_in_undirected_plan::execute_match_in(
            snapshot,
            input,
            src_alias,
            rels,
            edge_alias,
            dst_alias,
            dst_labels,
            *src_prebound,
            *optional,
            optional_unbind,
            path_alias,
            params,
        ),
        Plan::MatchUndirected {
            input,
            src_alias,
            rels,
            edge_alias,
            dst_alias,
            dst_labels,
            src_prebound,
            limit,
            optional,
            optional_unbind,
            path_alias,
        } => match_in_undirected_plan::execute_match_undirected(
            snapshot,
            input,
            src_alias,
            rels,
            edge_alias,
            dst_alias,
            dst_labels,
            *src_prebound,
            limit.map(|n| n as usize),
            *optional,
            optional_unbind,
            path_alias,
            params,
        ),
        Plan::MatchBoundRel {
            input,
            rel_alias,
            src_alias,
            dst_alias,
            dst_labels,
            src_prebound,
            rels,
            direction,
            optional,
            optional_unbind,
            path_alias,
        } => match_bound_rel_plan::execute_match_bound_rel(
            snapshot,
            input,
            rel_alias,
            src_alias,
            dst_alias,
            dst_labels,
            *src_prebound,
            rels,
            direction,
            *optional,
            optional_unbind,
            path_alias,
            params,
        ),
        Plan::Filter { input, predicate } => {
            plan_mid::execute_filter(snapshot, input, predicate, params)
        }
        Plan::OptionalWhereFixup {
            outer,
            filtered,
            null_aliases,
        } => {
            plan_mid::execute_optional_where_fixup(snapshot, outer, filtered, null_aliases, params)
        }
        Plan::Project { input, projections } => {
            plan_mid::execute_project(snapshot, input, projections, params)
        }
        Plan::Aggregate {
            input,
            group_by,
            aggregates,
        } => plan_mid::execute_aggregate(snapshot, input, group_by, aggregates, params),
        Plan::OrderBy { input, items } => {
            plan_mid::execute_order_by(snapshot, input, items, params)
        }
        Plan::Skip { input, skip } => plan_tail::execute_skip(snapshot, input, *skip, params),
        Plan::Limit { input, limit } => plan_tail::execute_limit(snapshot, input, *limit, params),
        Plan::Distinct { input } => plan_tail::execute_distinct(snapshot, input, params),
        Plan::Unwind {
            input,
            expression,
            alias,
        } => plan_tail::execute_unwind(snapshot, input, expression, alias, params),
        Plan::Union { left, right, all } => {
            plan_tail::execute_union(snapshot, left, right, *all, params)
        }
        Plan::Create { .. } => {
            plan_tail::write_only_plan_error("CREATE must be executed via execute_write")
        }
        Plan::Delete { .. } => {
            plan_tail::write_only_plan_error("DELETE must be executed via execute_write")
        }
        Plan::SetProperty { .. } | Plan::SetPropertiesFromMap { .. } | Plan::SetLabels { .. } => {
            plan_tail::write_only_plan_error("SET must be executed via execute_write")
        }
        Plan::RemoveProperty { .. } | Plan::RemoveLabels { .. } => {
            plan_tail::write_only_plan_error("REMOVE must be executed via execute_write")
        }
        Plan::IndexSeek {
            alias,
            label,
            field,
            value_expr,
            fallback,
        } => index_seek_plan::execute_index_seek(
            snapshot, alias, label, field, value_expr, fallback, params,
        ),
        Plan::Values { rows } => plan_tail::execute_values(rows),
    }
}

/// Execute a write plan (CREATE/DELETE/SET/REMOVE) with a transaction
pub fn execute_write<S: GraphSnapshot>(
    plan: &Plan,
    snapshot: &S,
    txn: &mut dyn WriteableGraph,
    params: &crate::query_api::Params,
) -> Result<u32> {
    write_dispatch::execute_write(plan, snapshot, txn, params)
}

pub fn execute_write_with_rows<S: GraphSnapshot>(
    plan: &Plan,
    snapshot: &S,
    txn: &mut dyn WriteableGraph,
    params: &crate::query_api::Params,
) -> Result<(u32, Vec<Row>)> {
    write_orchestration::execute_write_with_rows(plan, snapshot, txn, params)
}

pub fn execute_merge_with_rows<S: GraphSnapshot>(
    plan: &Plan,
    snapshot: &S,
    txn: &mut dyn WriteableGraph,
    params: &crate::query_api::Params,
    on_create_items: &[(String, String, Expression)],
    on_match_items: &[(String, String, Expression)],
) -> Result<(u32, Vec<Row>)> {
    write_orchestration::execute_merge_with_rows(
        plan,
        snapshot,
        txn,
        params,
        on_create_items,
        on_match_items,
    )
}

fn execute_merge_create_from_rows<S: GraphSnapshot>(
    snapshot: &S,
    input_rows: Vec<Row>,
    txn: &mut dyn WriteableGraph,
    pattern: &Pattern,
    params: &crate::query_api::Params,
    on_create_items: &[(String, String, Expression)],
    on_match_items: &[(String, String, Expression)],
    overlay: &mut MergeOverlayState,
) -> Result<(u32, Vec<Row>)> {
    merge_execution::execute_merge_create_from_rows(
        snapshot,
        input_rows,
        txn,
        pattern,
        params,
        on_create_items,
        on_match_items,
        overlay,
    )
}

pub(crate) fn execute_merge<S: GraphSnapshot>(
    plan: &Plan,
    snapshot: &S,
    txn: &mut dyn WriteableGraph,
    params: &crate::query_api::Params,
    on_create_items: &[(String, String, Expression)],
    on_match_items: &[(String, String, Expression)],
) -> Result<u32> {
    merge_execution::execute_merge(plan, snapshot, txn, params, on_create_items, on_match_items)
}

pub trait WriteableGraph {
    fn create_node(&mut self, external_id: ExternalId, label_id: LabelId)
    -> Result<InternalNodeId>;
    fn add_node_label(&mut self, node: InternalNodeId, label_id: LabelId) -> Result<()>;
    fn remove_node_label(&mut self, node: InternalNodeId, label_id: LabelId) -> Result<()>;
    fn create_edge(
        &mut self,
        src: InternalNodeId,
        rel: RelTypeId,
        dst: InternalNodeId,
    ) -> Result<()>;
    fn set_node_property(
        &mut self,
        node: InternalNodeId,
        key: String,
        value: PropertyValue,
    ) -> Result<()>;
    fn set_edge_property(
        &mut self,
        src: InternalNodeId,
        rel: RelTypeId,
        dst: InternalNodeId,
        key: String,
        value: PropertyValue,
    ) -> Result<()>;
    fn remove_node_property(&mut self, node: InternalNodeId, key: &str) -> Result<()>;
    fn remove_edge_property(
        &mut self,
        src: InternalNodeId,
        rel: RelTypeId,
        dst: InternalNodeId,
        key: &str,
    ) -> Result<()>;
    fn tombstone_node(&mut self, node: InternalNodeId) -> Result<()>;
    fn tombstone_edge(
        &mut self,
        src: InternalNodeId,
        rel: RelTypeId,
        dst: InternalNodeId,
    ) -> Result<()>;

    // T65: Dynamic schema support
    fn get_or_create_label_id(&mut self, name: &str) -> Result<LabelId>;
    fn get_or_create_rel_type_id(&mut self, name: &str) -> Result<RelTypeId>;
}

pub use nervusdb_v2_storage::property::PropertyValue;

fn execute_foreach<S: GraphSnapshot>(
    snapshot: &S,
    input: &Plan,
    txn: &mut dyn WriteableGraph,
    variable: &str,
    list: &Expression,
    sub_plan: &Plan,
    params: &crate::query_api::Params,
) -> Result<u32> {
    foreach_ops::execute_foreach(snapshot, input, txn, variable, list, sub_plan, params)
}

fn execute_delete_on_rows<S: GraphSnapshot>(
    snapshot: &S,
    rows: &[Row],
    txn: &mut dyn WriteableGraph,
    detach: bool,
    expressions: &[Expression],
    params: &crate::query_api::Params,
) -> Result<u32> {
    create_delete_ops::execute_delete_on_rows(snapshot, rows, txn, detach, expressions, params)
}

fn execute_create_write_rows<S: GraphSnapshot>(
    plan: &Plan,
    snapshot: &S,
    txn: &mut dyn WriteableGraph,
    params: &crate::query_api::Params,
) -> Result<(u32, Vec<Row>)> {
    create_delete_ops::execute_create_write_rows(plan, snapshot, txn, params)
}

fn execute_create<S: GraphSnapshot>(
    snapshot: &S,
    input: &Plan,
    txn: &mut dyn WriteableGraph,
    pattern: &Pattern,
    params: &crate::query_api::Params,
) -> Result<u32> {
    create_delete_ops::execute_create(snapshot, input, txn, pattern, params)
}

fn execute_delete<S: GraphSnapshot>(
    snapshot: &S,
    input: &Plan,
    txn: &mut dyn WriteableGraph,
    detach: bool,
    expressions: &[Expression],
    params: &crate::query_api::Params,
) -> Result<u32> {
    create_delete_ops::execute_delete(snapshot, input, txn, detach, expressions, params)
}

fn evaluate_property_value(
    expr: &Expression,
    params: &crate::query_api::Params,
) -> Result<PropertyValue> {
    write_path::evaluate_property_value(expr, params)
}

fn convert_executor_value_to_property(value: &Value) -> Result<PropertyValue> {
    write_path::convert_executor_value_to_property(value)
}

pub fn convert_api_property_to_value(api_value: &nervusdb_v2_api::PropertyValue) -> Value {
    match api_value {
        nervusdb_v2_api::PropertyValue::Null => Value::Null,
        nervusdb_v2_api::PropertyValue::Bool(b) => Value::Bool(*b),
        nervusdb_v2_api::PropertyValue::Int(i) => Value::Int(*i),
        nervusdb_v2_api::PropertyValue::Float(f) => Value::Float(*f),
        nervusdb_v2_api::PropertyValue::String(s) => Value::String(s.clone()),
        nervusdb_v2_api::PropertyValue::DateTime(i) => Value::DateTime(*i),
        nervusdb_v2_api::PropertyValue::Blob(b) => Value::Blob(b.clone()),
        nervusdb_v2_api::PropertyValue::List(l) => {
            Value::List(l.iter().map(convert_api_property_to_value).collect())
        }
        nervusdb_v2_api::PropertyValue::Map(m) => Value::Map(
            m.iter()
                .map(|(k, v)| (k.clone(), convert_api_property_to_value(v)))
                .collect(),
        ),
    }
}

pub fn parse_u32_identifier(name: &str) -> Result<u32> {
    name.parse::<u32>()
        .map_err(|_| Error::NotImplemented("non-numeric label/rel identifiers in M3"))
}
