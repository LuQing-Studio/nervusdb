use napi::bindgen_prelude::Result;
use napi::Error;
use napi_derive::napi;
use nervusdb::{
    backup as rust_backup, bulkload as rust_bulkload, vacuum as rust_vacuum, BulkEdge, BulkNode,
    Db as RustDb, Error as V2Error, PropertyValue,
};
use nervusdb_query::{Params, Value};
use serde_json::{json, Map as JsonMap, Value as JsonValue};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

fn error_payload(code: &str, category: &str, message: impl ToString) -> String {
    json!({
        "code": code,
        "category": category,
        "message": message.to_string(),
    })
    .to_string()
}

fn classify_err_message(msg: &str) -> (&'static str, &'static str) {
    let lower = msg.to_lowercase();
    if lower.contains("resourcelimitexceeded") {
        ("NERVUS_RESOURCE_LIMIT", "execution")
    } else if lower.contains("storage format mismatch") || lower.contains("compatibility") {
        ("NERVUS_COMPATIBILITY", "compatibility")
    } else if lower.contains("syntax")
        || lower.contains("parse")
        || lower.contains("unexpected token")
        || lower.starts_with("expected ")
        || lower.contains("variabletypeconflict")
        || lower.contains("variablealreadybound")
    {
        ("NERVUS_SYNTAX", "syntax")
    } else if lower.contains("wal")
        || lower.contains("checkpoint")
        || lower.contains("io error")
        || lower.contains("permission denied")
        || lower.contains("no such file")
        || lower.contains("disk full")
        || lower.contains("database is closed")
    {
        ("NERVUS_STORAGE", "storage")
    } else {
        ("NERVUS_EXECUTION", "execution")
    }
}

fn napi_err(err: impl ToString) -> Error {
    let message = err.to_string();
    let (code, category) = classify_err_message(&message);
    Error::from_reason(error_payload(code, category, message))
}

fn napi_err_v2(err: V2Error) -> Error {
    let payload = match err {
        V2Error::Compatibility(message) => {
            error_payload("NERVUS_COMPATIBILITY", "compatibility", message)
        }
        V2Error::Storage(message) => error_payload("NERVUS_STORAGE", "storage", message),
        V2Error::Query(message) => error_payload("NERVUS_EXECUTION", "execution", message),
        V2Error::Other(message) => error_payload("NERVUS_EXECUTION", "execution", message),
        V2Error::Io(io_err) => error_payload("NERVUS_STORAGE", "storage", io_err),
    };
    Error::from_reason(payload)
}

fn derive_paths(path: &Path) -> (PathBuf, PathBuf) {
    match path.extension().and_then(|e| e.to_str()) {
        Some("ndb") => (path.to_path_buf(), path.with_extension("wal")),
        Some("wal") => (path.with_extension("ndb"), path.to_path_buf()),
        _ => (path.with_extension("ndb"), path.with_extension("wal")),
    }
}

fn json_to_query_value(v: &JsonValue) -> std::result::Result<Value, String> {
    match v {
        JsonValue::Null => Ok(Value::Null),
        JsonValue::Bool(b) => Ok(Value::Bool(*b)),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Int(i))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Float(f))
            } else {
                Err("unsupported numeric value".to_string())
            }
        }
        JsonValue::String(s) => Ok(Value::String(s.clone())),
        JsonValue::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(json_to_query_value(item)?);
            }
            Ok(Value::List(out))
        }
        JsonValue::Object(map) => {
            let mut out = BTreeMap::new();
            for (k, v) in map {
                out.insert(k.clone(), json_to_query_value(v)?);
            }
            Ok(Value::Map(out))
        }
    }
}

fn json_to_property_value(v: &JsonValue) -> std::result::Result<PropertyValue, String> {
    match v {
        JsonValue::Null => Ok(PropertyValue::Null),
        JsonValue::Bool(b) => Ok(PropertyValue::Bool(*b)),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(PropertyValue::Int(i))
            } else if let Some(f) = n.as_f64() {
                Ok(PropertyValue::Float(f))
            } else {
                Err("unsupported numeric value".to_string())
            }
        }
        JsonValue::String(s) => Ok(PropertyValue::String(s.clone())),
        JsonValue::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(json_to_property_value(item)?);
            }
            Ok(PropertyValue::List(out))
        }
        JsonValue::Object(map) => {
            let mut out = BTreeMap::new();
            for (k, v) in map {
                out.insert(k.clone(), json_to_property_value(v)?);
            }
            Ok(PropertyValue::Map(out))
        }
    }
}

fn parse_params(params: Option<JsonValue>) -> std::result::Result<Params, String> {
    let mut out = Params::new();
    let Some(params) = params else {
        return Ok(out);
    };

    let JsonValue::Object(map) = params else {
        return Err("params must be an object".to_string());
    };

    for (k, v) in map {
        out.insert(k, json_to_query_value(&v)?);
    }
    Ok(out)
}

fn parse_property_map(
    props: Option<JsonValue>,
) -> std::result::Result<BTreeMap<String, PropertyValue>, String> {
    let Some(props) = props else {
        return Ok(BTreeMap::new());
    };
    let JsonValue::Object(map) = props else {
        return Err("properties must be an object".to_string());
    };

    let mut out = BTreeMap::new();
    for (k, v) in map {
        out.insert(k, json_to_property_value(&v)?);
    }
    Ok(out)
}

fn value_to_json(v: Value) -> JsonValue {
    match v {
        Value::Null => JsonValue::Null,
        Value::Bool(b) => json!(b),
        Value::Int(i) => json!(i),
        Value::Float(f) => json!(f),
        Value::String(s) => json!(s),
        Value::DateTime(ts) => json!({"type": "datetime", "value": ts}),
        Value::Blob(bytes) => json!({"type": "blob", "len": bytes.len()}),
        Value::List(list) => JsonValue::Array(list.into_iter().map(value_to_json).collect()),
        Value::Map(map) => {
            let mut out = JsonMap::new();
            for (k, v) in map {
                out.insert(k, value_to_json(v));
            }
            JsonValue::Object(out)
        }
        Value::Node(n) => {
            let mut props = JsonMap::new();
            for (k, v) in n.properties {
                props.insert(k, value_to_json(v));
            }
            json!({
                "type": "node",
                "id": n.id,
                "labels": n.labels,
                "properties": props,
            })
        }
        Value::Relationship(r) => {
            let mut props = JsonMap::new();
            for (k, v) in r.properties {
                props.insert(k, value_to_json(v));
            }
            json!({
                "type": "relationship",
                "src": r.key.src,
                "dst": r.key.dst,
                "rel_type": r.rel_type,
                "properties": props,
            })
        }
        Value::ReifiedPath(p) => {
            let nodes = p
                .nodes
                .into_iter()
                .map(Value::Node)
                .map(value_to_json)
                .collect::<Vec<_>>();
            let rels = p
                .relationships
                .into_iter()
                .map(Value::Relationship)
                .map(value_to_json)
                .collect::<Vec<_>>();
            json!({"type": "path", "nodes": nodes, "relationships": rels})
        }
        Value::NodeId(id) => json!({"type": "node_id", "value": id}),
        Value::ExternalId(id) => json!({"type": "external_id", "value": id}),
        Value::EdgeKey(k) => json!({"type": "edge_key", "src": k.src, "dst": k.dst}),
        Value::Path(p) => {
            let edges = p
                .edges
                .into_iter()
                .map(|e| json!({"src": e.src, "dst": e.dst}))
                .collect::<Vec<_>>();
            json!({"type": "path_legacy", "nodes": p.nodes, "edges": edges})
        }
    }
}

fn run_query(
    db: &RustDb,
    cypher: &str,
    params: Option<JsonValue>,
) -> std::result::Result<Vec<JsonValue>, String> {
    let prepared = nervusdb_query::prepare(cypher).map_err(|e| e.to_string())?;
    let params = parse_params(params)?;
    let snapshot = db.snapshot();
    let rows: Vec<_> = prepared
        .execute_streaming(&snapshot, &params)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let mut map = JsonMap::new();
        for (col, val) in row.columns() {
            let rv = val.reify(&snapshot).map_err(|e| e.to_string())?;
            map.insert(col.clone(), value_to_json(rv));
        }
        out.push(JsonValue::Object(map));
    }
    Ok(out)
}

#[napi(object)]
pub struct BulkNodeInput {
    pub external_id: i64,
    pub label: String,
    pub properties: Option<JsonValue>,
}

#[napi(object)]
pub struct BulkEdgeInput {
    pub src_external_id: i64,
    pub rel_type: String,
    pub dst_external_id: i64,
    pub properties: Option<JsonValue>,
}

#[napi]
pub struct Db {
    inner: Arc<Mutex<Option<RustDb>>>,
    path: String,
    ndb_path: String,
    wal_path: String,
    refresh_epoch: Arc<AtomicU64>,
    last_refresh_epoch: AtomicU64,
    active_write_txns: Arc<AtomicU64>,
}

impl Db {
    fn make_open(ndb_path: String, wal_path: String, logical_path: String) -> Result<Self> {
        let inner = RustDb::open_paths(&ndb_path, &wal_path).map_err(napi_err_v2)?;
        Ok(Self {
            inner: Arc::new(Mutex::new(Some(inner))),
            path: logical_path,
            ndb_path,
            wal_path,
            refresh_epoch: Arc::new(AtomicU64::new(0)),
            last_refresh_epoch: AtomicU64::new(0),
            active_write_txns: Arc::new(AtomicU64::new(0)),
        })
    }

    fn refresh_if_needed(&self) -> Result<()> {
        let target_epoch = self.refresh_epoch.load(Ordering::SeqCst);
        let seen_epoch = self.last_refresh_epoch.load(Ordering::SeqCst);
        if target_epoch == seen_epoch {
            return Ok(());
        }

        let mut guard = self
            .inner
            .lock()
            .map_err(|_| napi_err("database mutex poisoned"))?;

        if self.last_refresh_epoch.load(Ordering::SeqCst) == target_epoch {
            return Ok(());
        }

        if let Some(inner) = guard.take() {
            inner.close().map_err(napi_err_v2)?;
        }

        let reopened = RustDb::open_paths(&self.ndb_path, &self.wal_path).map_err(napi_err_v2)?;
        *guard = Some(reopened);
        self.last_refresh_epoch.store(target_epoch, Ordering::SeqCst);
        Ok(())
    }

    fn with_db<T>(&self, f: impl FnOnce(&RustDb) -> Result<T>) -> Result<T> {
        self.refresh_if_needed()?;
        let guard = self
            .inner
            .lock()
            .map_err(|_| napi_err("database mutex poisoned"))?;
        let db = guard
            .as_ref()
            .ok_or_else(|| napi_err("database is closed"))?;
        f(db)
    }
}

#[napi]
impl Db {
    #[napi(factory)]
    pub fn open(path: String) -> Result<Self> {
        let (ndb_path, wal_path) = derive_paths(Path::new(&path));
        Self::make_open(
            ndb_path.to_string_lossy().to_string(),
            wal_path.to_string_lossy().to_string(),
            path,
        )
    }

    #[napi(factory, js_name = "openPaths")]
    pub fn open_paths_factory(ndb_path: String, wal_path: String) -> Result<Self> {
        let logical_path = ndb_path.clone();
        Self::make_open(ndb_path, wal_path, logical_path)
    }

    #[napi(getter)]
    pub fn path(&self) -> String {
        self.path.clone()
    }

    #[napi(getter, js_name = "ndbPath")]
    pub fn ndb_path(&self) -> String {
        self.ndb_path.clone()
    }

    #[napi(getter, js_name = "walPath")]
    pub fn wal_path(&self) -> String {
        self.wal_path.clone()
    }

    #[napi]
    pub fn query(&self, cypher: String, params: Option<JsonValue>) -> Result<Vec<JsonValue>> {
        self.with_db(|db| run_query(db, &cypher, params).map_err(napi_err))
    }

    #[napi]
    pub fn execute_write(&self, cypher: String, params: Option<JsonValue>) -> Result<u32> {
        self.with_db(|db| {
            let prepared = nervusdb_query::prepare(&cypher).map_err(napi_err)?;
            let params = parse_params(params).map_err(napi_err)?;
            let snapshot = db.snapshot();
            let mut txn = db.begin_write();
            let created = prepared
                .execute_write(&snapshot, &mut txn, &params)
                .map_err(napi_err)?;
            txn.commit().map_err(napi_err_v2)?;
            Ok(created)
        })
    }

    #[napi]
    pub fn begin_write(&self) -> Result<WriteTxn> {
        self.refresh_if_needed()?;
        self.active_write_txns.fetch_add(1, Ordering::SeqCst);
        Ok(WriteTxn {
            owner_inner: self.inner.clone(),
            staged_queries: Vec::new(),
            refresh_epoch: self.refresh_epoch.clone(),
            active_write_txns: self.active_write_txns.clone(),
            affected: 0,
            finished: false,
        })
    }

    #[napi]
    pub fn compact(&self) -> Result<()> {
        self.with_db(|db| db.compact().map_err(napi_err_v2))
    }

    #[napi]
    pub fn checkpoint(&self) -> Result<()> {
        self.with_db(|db| db.checkpoint().map_err(napi_err_v2))
    }

    #[napi(js_name = "createIndex")]
    pub fn create_index(&self, label: String, property: String) -> Result<()> {
        self.with_db(|db| db.create_index(&label, &property).map_err(napi_err_v2))
    }

    #[napi(js_name = "searchVector")]
    pub fn search_vector(&self, query: Vec<f64>, k: u32) -> Result<Vec<JsonValue>> {
        self.with_db(|db| {
            let query: Vec<f32> = query.into_iter().map(|v| v as f32).collect();
            let rows = db.search_vector(&query, k as usize).map_err(napi_err_v2)?;
            Ok(rows
                .into_iter()
                .map(|(node_id, distance)| json!({"nodeId": node_id, "distance": distance}))
                .collect())
        })
    }

    #[napi]
    pub fn close(&self) -> Result<()> {
        if self.active_write_txns.load(Ordering::SeqCst) != 0 {
            return Err(napi_err("cannot close database: write transaction in progress"));
        }
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| napi_err("database mutex poisoned"))?;
        if let Some(inner) = guard.take() {
            inner.close().map_err(napi_err_v2)?;
        }
        Ok(())
    }
}

#[napi]
pub struct WriteTxn {
    owner_inner: Arc<Mutex<Option<RustDb>>>,
    staged_queries: Vec<(String, Option<JsonValue>)>,
    refresh_epoch: Arc<AtomicU64>,
    active_write_txns: Arc<AtomicU64>,
    affected: u32,
    finished: bool,
}

impl WriteTxn {
    fn run_immediate<T>(&mut self, f: impl FnOnce(&mut nervusdb::WriteTxn<'_>) -> std::result::Result<T, V2Error>) -> Result<T> {
        let guard = self
            .owner_inner
            .lock()
            .map_err(|_| napi_err("database mutex poisoned"))?;
        let db = guard
            .as_ref()
            .ok_or_else(|| napi_err("database is closed"))?;
        let mut txn = db.begin_write();
        let out = f(&mut txn).map_err(napi_err_v2)?;
        txn.commit().map_err(napi_err_v2)?;
        self.refresh_epoch.fetch_add(1, Ordering::SeqCst);
        Ok(out)
    }

    fn finish(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        self.active_write_txns.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Drop for WriteTxn {
    fn drop(&mut self) {
        self.finish();
    }
}

#[napi]
impl WriteTxn {
    #[napi]
    pub fn query(&mut self, cypher: String, params: Option<JsonValue>) -> Result<()> {
        let _ = nervusdb_query::prepare(&cypher).map_err(napi_err)?;
        if let Some(ref p) = params {
            let _ = parse_params(Some(p.clone())).map_err(napi_err)?;
        }
        self.staged_queries.push((cypher, params));
        Ok(())
    }

    #[napi(js_name = "createNode")]
    pub fn create_node(&mut self, external_id: i64, label_id: u32) -> Result<u32> {
        if external_id < 0 {
            return Err(napi_err("external_id must be >= 0"));
        }
        let node = self.run_immediate(|txn| txn.create_node(external_id as u64, label_id))?;
        self.affected = self.affected.saturating_add(1);
        Ok(node)
    }

    #[napi(js_name = "getOrCreateLabel")]
    pub fn get_or_create_label(&mut self, name: String) -> Result<u32> {
        self.run_immediate(|txn| txn.get_or_create_label(&name))
    }

    #[napi(js_name = "getOrCreateRelType")]
    pub fn get_or_create_rel_type(&mut self, name: String) -> Result<u32> {
        self.run_immediate(|txn| txn.get_or_create_rel_type(&name))
    }

    #[napi(js_name = "createEdge")]
    pub fn create_edge(&mut self, src: u32, rel: u32, dst: u32) -> Result<()> {
        self.run_immediate(|txn| {
            txn.create_edge(src, rel, dst);
            Ok(())
        })?;
        self.affected = self.affected.saturating_add(1);
        Ok(())
    }

    #[napi(js_name = "tombstoneNode")]
    pub fn tombstone_node(&mut self, node: u32) -> Result<()> {
        self.run_immediate(|txn| {
            txn.tombstone_node(node);
            Ok(())
        })?;
        self.affected = self.affected.saturating_add(1);
        Ok(())
    }

    #[napi(js_name = "tombstoneEdge")]
    pub fn tombstone_edge(&mut self, src: u32, rel: u32, dst: u32) -> Result<()> {
        self.run_immediate(|txn| {
            txn.tombstone_edge(src, rel, dst);
            Ok(())
        })?;
        self.affected = self.affected.saturating_add(1);
        Ok(())
    }

    #[napi(js_name = "setNodeProperty")]
    pub fn set_node_property(&mut self, node: u32, key: String, value: JsonValue) -> Result<()> {
        let value = json_to_property_value(&value).map_err(napi_err)?;
        self.run_immediate(|txn| txn.set_node_property(node, key, value))?;
        self.affected = self.affected.saturating_add(1);
        Ok(())
    }

    #[napi(js_name = "setEdgeProperty")]
    pub fn set_edge_property(
        &mut self,
        src: u32,
        rel: u32,
        dst: u32,
        key: String,
        value: JsonValue,
    ) -> Result<()> {
        let value = json_to_property_value(&value).map_err(napi_err)?;
        self.run_immediate(|txn| txn.set_edge_property(src, rel, dst, key, value))?;
        self.affected = self.affected.saturating_add(1);
        Ok(())
    }

    #[napi(js_name = "removeNodeProperty")]
    pub fn remove_node_property(&mut self, node: u32, key: String) -> Result<()> {
        self.run_immediate(|txn| txn.remove_node_property(node, &key))?;
        self.affected = self.affected.saturating_add(1);
        Ok(())
    }

    #[napi(js_name = "removeEdgeProperty")]
    pub fn remove_edge_property(&mut self, src: u32, rel: u32, dst: u32, key: String) -> Result<()> {
        self.run_immediate(|txn| txn.remove_edge_property(src, rel, dst, &key))?;
        self.affected = self.affected.saturating_add(1);
        Ok(())
    }

    #[napi(js_name = "setVector")]
    pub fn set_vector(&mut self, node: u32, vector: Vec<f64>) -> Result<()> {
        let vector: Vec<f32> = vector.into_iter().map(|v| v as f32).collect();
        self.run_immediate(|txn| txn.set_vector(node, vector))?;
        self.affected = self.affected.saturating_add(1);
        Ok(())
    }

    #[napi]
    pub fn rollback(&mut self) -> Result<()> {
        self.staged_queries.clear();
        self.finish();
        Ok(())
    }

    #[napi]
    pub fn commit(&mut self) -> Result<u32> {
        {
            let guard = self
                .owner_inner
                .lock()
                .map_err(|_| napi_err("database mutex poisoned"))?;
            let db = guard
                .as_ref()
                .ok_or_else(|| napi_err("database is closed"))?;
            for (cypher, params) in self.staged_queries.drain(..) {
                let prepared = nervusdb_query::prepare(&cypher).map_err(napi_err)?;
                let params = parse_params(params).map_err(napi_err)?;
                let snapshot = db.snapshot();
                let mut txn = db.begin_write();
                let created = prepared
                    .execute_write(&snapshot, &mut txn, &params)
                    .map_err(napi_err)?;
                self.affected = self.affected.saturating_add(created);
                txn.commit().map_err(napi_err_v2)?;
            }
        }
        self.refresh_epoch.fetch_add(1, Ordering::SeqCst);
        self.finish();
        Ok(self.affected)
    }
}

#[napi]
pub fn vacuum(path: String) -> Result<JsonValue> {
    let report = rust_vacuum(path).map_err(napi_err_v2)?;
    Ok(json!({
        "ndbPath": report.ndb_path,
        "backupPath": report.backup_path,
        "oldNextPageId": report.old_next_page_id,
        "newNextPageId": report.new_next_page_id,
        "copiedDataPages": report.copied_data_pages,
        "oldFilePages": report.old_file_pages,
        "newFilePages": report.new_file_pages,
    }))
}

#[napi]
pub fn backup(path: String, backup_dir: String) -> Result<JsonValue> {
    let info = rust_backup(path, backup_dir).map_err(napi_err_v2)?;
    Ok(json!({
        "id": info.id.to_string(),
        "createdAt": info.created_at.to_rfc3339(),
        "sizeBytes": info.size_bytes,
        "fileCount": info.file_count,
        "nervusdbVersion": info.nervusdb_version,
        "checkpointTxid": info.checkpoint_txid,
        "checkpointEpoch": info.checkpoint_epoch,
    }))
}

#[napi]
pub fn bulkload(path: String, nodes: Vec<BulkNodeInput>, edges: Vec<BulkEdgeInput>) -> Result<()> {
    let mut bulk_nodes = Vec::with_capacity(nodes.len());
    for n in nodes {
        if n.external_id < 0 {
            return Err(napi_err("bulk node external_id must be >= 0"));
        }
        bulk_nodes.push(BulkNode {
            external_id: n.external_id as u64,
            label: n.label,
            properties: parse_property_map(n.properties).map_err(napi_err)?,
        });
    }

    let mut bulk_edges = Vec::with_capacity(edges.len());
    for e in edges {
        if e.src_external_id < 0 || e.dst_external_id < 0 {
            return Err(napi_err("bulk edge external ids must be >= 0"));
        }
        bulk_edges.push(BulkEdge {
            src_external_id: e.src_external_id as u64,
            rel_type: e.rel_type,
            dst_external_id: e.dst_external_id as u64,
            properties: parse_property_map(e.properties).map_err(napi_err)?,
        });
    }

    rust_bulkload(path, bulk_nodes, bulk_edges).map_err(napi_err_v2)
}

#[cfg(test)]
mod tests {
    use super::{classify_err_message, napi_err};
    use serde_json::Value;

    fn parse_payload(reason: &str) -> Value {
        serde_json::from_str(reason).expect("napi reason should be valid json payload")
    }

    #[test]
    fn napi_err_uses_structured_compatibility_payload() {
        let err = napi_err("storage format mismatch: expected epoch 1, found 0");
        let reason = err.reason;
        assert!(reason.contains("\"category\":\"compatibility\""));
        assert!(reason.contains("\"code\":\"NERVUS_COMPATIBILITY\""));
    }

    #[test]
    fn napi_err_maps_syntax_messages_to_syntax_category() {
        let err = napi_err("syntax error: unexpected token");
        let payload = parse_payload(&err.reason);
        assert_eq!(payload["code"], "NERVUS_SYNTAX");
        assert_eq!(payload["category"], "syntax");
        assert_eq!(payload["message"], "syntax error: unexpected token");
    }

    #[test]
    fn napi_err_maps_expected_prefix_to_syntax_category() {
        let err = napi_err("Expected ')'");
        let payload = parse_payload(&err.reason);
        assert_eq!(payload["code"], "NERVUS_SYNTAX");
        assert_eq!(payload["category"], "syntax");
    }

    #[test]
    fn napi_err_maps_storage_messages_for_fs_failures() {
        let err = napi_err("permission denied while opening wal");
        let payload = parse_payload(&err.reason);
        assert_eq!(payload["code"], "NERVUS_STORAGE");
        assert_eq!(payload["category"], "storage");
    }

    #[test]
    fn napi_err_falls_back_to_execution_category() {
        let err = napi_err("not implemented: expression");
        let payload = parse_payload(&err.reason);
        assert_eq!(payload["code"], "NERVUS_EXECUTION");
        assert_eq!(payload["category"], "execution");
    }

    #[test]
    fn classify_err_message_keeps_compatibility_priority() {
        let (code, category) =
            classify_err_message("compatibility warning with parse token details");
        assert_eq!(code, "NERVUS_COMPATIBILITY");
        assert_eq!(category, "compatibility");
    }

    #[test]
    fn classify_err_message_detects_resource_limit() {
        let (code, category) = classify_err_message(
            "execution error: ResourceLimitExceeded(kind=Timeout, limit=1, observed=10, stage=ReturnOne)",
        );
        assert_eq!(code, "NERVUS_RESOURCE_LIMIT");
        assert_eq!(category, "execution");
    }
}
