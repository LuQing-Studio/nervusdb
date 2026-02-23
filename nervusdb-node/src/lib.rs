use napi::bindgen_prelude::Result;
use napi::Error;
use napi_derive::napi;
use nervusdb_capi as capi;
use serde_json::{json, Value as JsonValue};
use std::ffi::{c_char, CStr, CString};
use std::fs;
use std::path::{Path, PathBuf};
use std::ptr;
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
        || lower.contains("unexpected character")
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

fn to_cstring(value: &str, field: &str) -> Result<CString> {
    CString::new(value).map_err(|_| napi_err(format!("{field} contains interior NUL")))
}

fn derive_paths(path: &Path) -> (PathBuf, PathBuf) {
    match path.extension().and_then(|e| e.to_str()) {
        Some("ndb") => (path.to_path_buf(), path.with_extension("wal")),
        Some("wal") => (path.with_extension("ndb"), path.to_path_buf()),
        _ => (path.with_extension("ndb"), path.with_extension("wal")),
    }
}

fn read_last_error_message() -> String {
    let needed = capi::ndb_last_error_message(ptr::null_mut(), 0);
    if needed == 0 {
        return "unknown error".to_string();
    }

    let mut buf = vec![0 as c_char; needed.saturating_add(1).max(64)];
    let _ = capi::ndb_last_error_message(buf.as_mut_ptr(), buf.len());
    unsafe {
        // SAFETY: C API guarantees null-terminated output when len > 0.
        CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned()
    }
}

fn map_error_payload(category: i32, message: &str) -> (&'static str, &'static str) {
    match category {
        x if x == capi::NDB_ERRCAT_SYNTAX => ("NERVUS_SYNTAX", "syntax"),
        x if x == capi::NDB_ERRCAT_STORAGE => ("NERVUS_STORAGE", "storage"),
        x if x == capi::NDB_ERRCAT_COMPATIBILITY => ("NERVUS_COMPATIBILITY", "compatibility"),
        x if x == capi::NDB_ERRCAT_EXECUTION => {
            let (code, _cat) = classify_err_message(message);
            if code == "NERVUS_SYNTAX" || code == "NERVUS_STORAGE" || code == "NERVUS_COMPATIBILITY"
            {
                ("NERVUS_EXECUTION", "execution")
            } else {
                (code, "execution")
            }
        }
        _ => classify_err_message(message),
    }
}

fn napi_last_error() -> Error {
    let category = capi::ndb_last_error_category();
    let message = read_last_error_message();
    let (code, category) = map_error_payload(category, &message);
    Error::from_reason(error_payload(code, category, message))
}

fn capi_status(rc: i32) -> Result<()> {
    if rc == capi::NDB_OK {
        Ok(())
    } else {
        Err(napi_last_error())
    }
}

fn encode_params(params: Option<JsonValue>) -> Result<Option<CString>> {
    let Some(params) = params else {
        return Ok(None);
    };
    if !params.is_object() {
        return Err(napi_err("params must be an object"));
    }
    let encoded = serde_json::to_string(&params).map_err(napi_err)?;
    Ok(Some(to_cstring(&encoded, "params")?))
}

fn parse_json_array(json_text: &str) -> Result<Vec<JsonValue>> {
    let value: JsonValue = serde_json::from_str(json_text).map_err(napi_err)?;
    let arr = value
        .as_array()
        .ok_or_else(|| napi_err("C ABI query result must be a JSON array"))?;
    Ok(arr.clone())
}

fn result_to_json_rows(result_ptr: *mut capi::ndb_result_t) -> Result<Vec<JsonValue>> {
    let mut json_ptr: *mut c_char = ptr::null_mut();
    let rc = capi::ndb_result_to_json(result_ptr, &mut json_ptr);
    let _ = capi::ndb_result_free(result_ptr);
    capi_status(rc)?;
    if json_ptr.is_null() {
        return Err(napi_err("ndb_result_to_json returned null"));
    }

    let json_text = unsafe {
        // SAFETY: pointer returned by C API is valid until freed by `ndb_string_free`.
        CStr::from_ptr(json_ptr).to_string_lossy().into_owned()
    };
    capi::ndb_string_free(json_ptr);
    parse_json_array(&json_text)
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
    raw: Arc<Mutex<Option<*mut capi::ndb_db_t>>>,
    path: String,
    ndb_path: String,
    wal_path: String,
    active_write_txns: Arc<AtomicU64>,
}

impl Db {
    fn with_db_ptr<T>(&self, f: impl FnOnce(*mut capi::ndb_db_t) -> Result<T>) -> Result<T> {
        let guard = self
            .raw
            .lock()
            .map_err(|_| napi_err("database mutex poisoned"))?;
        let raw = guard
            .as_ref()
            .copied()
            .ok_or_else(|| napi_err("database is closed"))?;
        f(raw)
    }

    fn make_open(ndb_path: String, wal_path: String, logical_path: String) -> Result<Self> {
        let ndb_c = to_cstring(&ndb_path, "ndb_path")?;
        let wal_c = to_cstring(&wal_path, "wal_path")?;
        let mut raw: *mut capi::ndb_db_t = ptr::null_mut();
        capi_status(capi::ndb_open_paths(
            ndb_c.as_ptr(),
            wal_c.as_ptr(),
            &mut raw,
        ))?;
        if raw.is_null() {
            return Err(napi_err("ndb_open_paths returned null db handle"));
        }

        Ok(Self {
            raw: Arc::new(Mutex::new(Some(raw))),
            path: logical_path,
            ndb_path,
            wal_path,
            active_write_txns: Arc::new(AtomicU64::new(0)),
        })
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
        self.with_db_ptr(|raw| {
            let cypher_c = to_cstring(&cypher, "cypher")?;
            let params_c = encode_params(params)?;
            let params_ptr = params_c.as_ref().map_or(ptr::null(), |s| s.as_ptr());

            let mut result_ptr: *mut capi::ndb_result_t = ptr::null_mut();
            capi_status(capi::ndb_query(
                raw,
                cypher_c.as_ptr(),
                params_ptr,
                &mut result_ptr,
            ))?;
            if result_ptr.is_null() {
                return Err(napi_err("ndb_query returned null result handle"));
            }
            result_to_json_rows(result_ptr)
        })
    }

    #[napi]
    pub fn execute_write(&self, cypher: String, params: Option<JsonValue>) -> Result<u32> {
        self.with_db_ptr(|raw| {
            let cypher_c = to_cstring(&cypher, "cypher")?;
            let params_c = encode_params(params)?;
            let params_ptr = params_c.as_ref().map_or(ptr::null(), |s| s.as_ptr());

            let mut affected: u32 = 0;
            capi_status(capi::ndb_execute_write(
                raw,
                cypher_c.as_ptr(),
                params_ptr,
                &mut affected,
            ))?;
            Ok(affected)
        })
    }

    #[napi]
    pub fn begin_write(&self) -> Result<WriteTxn> {
        self.with_db_ptr(|raw| {
            let mut txn_raw: *mut capi::ndb_txn_t = ptr::null_mut();
            self.active_write_txns.fetch_add(1, Ordering::SeqCst);
            if let Err(err) = capi_status(capi::ndb_begin_write(raw, &mut txn_raw)) {
                self.active_write_txns.fetch_sub(1, Ordering::SeqCst);
                return Err(err);
            }
            if txn_raw.is_null() {
                self.active_write_txns.fetch_sub(1, Ordering::SeqCst);
                return Err(napi_err("ndb_begin_write returned null transaction"));
            }
            Ok(WriteTxn {
                raw: Some(txn_raw),
                active_write_txns: self.active_write_txns.clone(),
                affected: 0,
                finished: false,
            })
        })
    }

    #[napi]
    pub fn compact(&self) -> Result<()> {
        self.with_db_ptr(|raw| capi_status(capi::ndb_compact(raw)))
    }

    #[napi]
    pub fn checkpoint(&self) -> Result<()> {
        self.with_db_ptr(|raw| capi_status(capi::ndb_checkpoint(raw)))
    }

    #[napi(js_name = "createIndex")]
    pub fn create_index(&self, label: String, property: String) -> Result<()> {
        self.with_db_ptr(|raw| {
            let label_c = to_cstring(&label, "label")?;
            let property_c = to_cstring(&property, "property")?;
            capi_status(capi::ndb_create_index(
                raw,
                label_c.as_ptr(),
                property_c.as_ptr(),
            ))
        })
    }

    #[napi(js_name = "searchVector")]
    pub fn search_vector(&self, query: Vec<f64>, k: u32) -> Result<Vec<JsonValue>> {
        self.with_db_ptr(|raw| {
            let query_f32: Vec<f32> = query.into_iter().map(|v| v as f32).collect();
            let mut result_ptr: *mut capi::ndb_result_t = ptr::null_mut();
            capi_status(capi::ndb_search_vector(
                raw,
                query_f32.as_ptr(),
                query_f32.len(),
                k,
                &mut result_ptr,
            ))?;
            if result_ptr.is_null() {
                return Err(napi_err("ndb_search_vector returned null result"));
            }
            let rows = result_to_json_rows(result_ptr)?;
            Ok(rows
                .into_iter()
                .map(|r| {
                    let node_id = r.get("node_id").cloned().unwrap_or(JsonValue::Null);
                    let distance = r.get("distance").cloned().unwrap_or(JsonValue::Null);
                    json!({"nodeId": node_id, "distance": distance})
                })
                .collect())
        })
    }

    #[napi]
    pub fn close(&self) -> Result<()> {
        if self.active_write_txns.load(Ordering::SeqCst) != 0 {
            return Err(napi_err(
                "cannot close database: write transaction in progress",
            ));
        }

        let mut guard = self
            .raw
            .lock()
            .map_err(|_| napi_err("database mutex poisoned"))?;
        if let Some(raw) = guard.take() {
            capi_status(capi::ndb_close(raw))?;
        }
        Ok(())
    }
}

#[napi]
pub struct WriteTxn {
    raw: Option<*mut capi::ndb_txn_t>,
    active_write_txns: Arc<AtomicU64>,
    affected: u32,
    finished: bool,
}

impl WriteTxn {
    fn with_txn_ptr<T>(&mut self, f: impl FnOnce(*mut capi::ndb_txn_t) -> Result<T>) -> Result<T> {
        let raw = self
            .raw
            .ok_or_else(|| napi_err("transaction is no longer active"))?;
        f(raw)
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
        if self.finished {
            return;
        }
        if let Some(raw) = self.raw.take() {
            let _ = capi::ndb_txn_rollback(raw);
        }
        self.affected = 0;
        self.finish();
    }
}

#[napi]
impl WriteTxn {
    #[napi]
    pub fn query(&mut self, cypher: String, params: Option<JsonValue>) -> Result<()> {
        let cypher_c = to_cstring(&cypher, "cypher")?;
        let params_c = encode_params(params)?;
        let params_ptr = params_c.as_ref().map_or(ptr::null(), |s| s.as_ptr());

        self.with_txn_ptr(|raw| {
            capi_status(capi::ndb_txn_query(raw, cypher_c.as_ptr(), params_ptr))
        })?;
        self.affected = self.affected.saturating_add(1);
        Ok(())
    }

    #[napi(js_name = "createNode")]
    pub fn create_node(&mut self, external_id: i64, label_id: u32) -> Result<u32> {
        if external_id < 0 {
            return Err(napi_err("external_id must be >= 0"));
        }
        let mut out: u32 = 0;
        self.with_txn_ptr(|raw| {
            capi_status(capi::ndb_txn_create_node(
                raw,
                external_id as u64,
                label_id,
                &mut out,
            ))
        })?;
        self.affected = self.affected.saturating_add(1);
        Ok(out)
    }

    #[napi(js_name = "getOrCreateLabel")]
    pub fn get_or_create_label(&mut self, name: String) -> Result<u32> {
        let name_c = to_cstring(&name, "name")?;
        let mut out: u32 = 0;
        self.with_txn_ptr(|raw| {
            capi_status(capi::ndb_txn_get_or_create_label(
                raw,
                name_c.as_ptr(),
                &mut out,
            ))
        })?;
        Ok(out)
    }

    #[napi(js_name = "getOrCreateRelType")]
    pub fn get_or_create_rel_type(&mut self, name: String) -> Result<u32> {
        let name_c = to_cstring(&name, "name")?;
        let mut out: u32 = 0;
        self.with_txn_ptr(|raw| {
            capi_status(capi::ndb_txn_get_or_create_rel_type(
                raw,
                name_c.as_ptr(),
                &mut out,
            ))
        })?;
        Ok(out)
    }

    #[napi(js_name = "createEdge")]
    pub fn create_edge(&mut self, src: u32, rel: u32, dst: u32) -> Result<()> {
        self.with_txn_ptr(|raw| capi_status(capi::ndb_txn_create_edge(raw, src, rel, dst)))?;
        self.affected = self.affected.saturating_add(1);
        Ok(())
    }

    #[napi(js_name = "tombstoneNode")]
    pub fn tombstone_node(&mut self, node: u32) -> Result<()> {
        self.with_txn_ptr(|raw| capi_status(capi::ndb_txn_tombstone_node(raw, node)))?;
        self.affected = self.affected.saturating_add(1);
        Ok(())
    }

    #[napi(js_name = "tombstoneEdge")]
    pub fn tombstone_edge(&mut self, src: u32, rel: u32, dst: u32) -> Result<()> {
        self.with_txn_ptr(|raw| capi_status(capi::ndb_txn_tombstone_edge(raw, src, rel, dst)))?;
        self.affected = self.affected.saturating_add(1);
        Ok(())
    }

    #[napi(js_name = "setNodeProperty")]
    pub fn set_node_property(&mut self, node: u32, key: String, value: JsonValue) -> Result<()> {
        let key_c = to_cstring(&key, "key")?;
        let value_c = to_cstring(
            &serde_json::to_string(&value).map_err(napi_err)?,
            "value_json",
        )?;
        self.with_txn_ptr(|raw| {
            capi_status(capi::ndb_txn_set_node_property(
                raw,
                node,
                key_c.as_ptr(),
                value_c.as_ptr(),
            ))
        })?;
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
        let key_c = to_cstring(&key, "key")?;
        let value_c = to_cstring(
            &serde_json::to_string(&value).map_err(napi_err)?,
            "value_json",
        )?;
        self.with_txn_ptr(|raw| {
            capi_status(capi::ndb_txn_set_edge_property(
                raw,
                src,
                rel,
                dst,
                key_c.as_ptr(),
                value_c.as_ptr(),
            ))
        })?;
        self.affected = self.affected.saturating_add(1);
        Ok(())
    }

    #[napi(js_name = "removeNodeProperty")]
    pub fn remove_node_property(&mut self, node: u32, key: String) -> Result<()> {
        let key_c = to_cstring(&key, "key")?;
        self.with_txn_ptr(|raw| {
            capi_status(capi::ndb_txn_remove_node_property(
                raw,
                node,
                key_c.as_ptr(),
            ))
        })?;
        self.affected = self.affected.saturating_add(1);
        Ok(())
    }

    #[napi(js_name = "removeEdgeProperty")]
    pub fn remove_edge_property(
        &mut self,
        src: u32,
        rel: u32,
        dst: u32,
        key: String,
    ) -> Result<()> {
        let key_c = to_cstring(&key, "key")?;
        self.with_txn_ptr(|raw| {
            capi_status(capi::ndb_txn_remove_edge_property(
                raw,
                src,
                rel,
                dst,
                key_c.as_ptr(),
            ))
        })?;
        self.affected = self.affected.saturating_add(1);
        Ok(())
    }

    #[napi(js_name = "setVector")]
    pub fn set_vector(&mut self, node: u32, vector: Vec<f64>) -> Result<()> {
        let vector: Vec<f32> = vector.into_iter().map(|v| v as f32).collect();
        self.with_txn_ptr(|raw| {
            capi_status(capi::ndb_txn_set_vector(
                raw,
                node,
                vector.as_ptr(),
                vector.len(),
            ))
        })?;
        self.affected = self.affected.saturating_add(1);
        Ok(())
    }

    #[napi]
    pub fn rollback(&mut self) -> Result<()> {
        if self.finished {
            return Ok(());
        }
        if let Some(raw) = self.raw.take() {
            capi_status(capi::ndb_txn_rollback(raw))?;
        }
        self.affected = 0;
        self.finish();
        Ok(())
    }

    #[napi]
    pub fn commit(&mut self) -> Result<u32> {
        if self.finished {
            return Ok(self.affected);
        }
        if let Some(raw) = self.raw.take() {
            capi_status(capi::ndb_txn_commit(raw))?;
        }
        self.finish();
        Ok(self.affected)
    }
}

#[napi]
pub fn vacuum(path: String) -> Result<JsonValue> {
    let path_c = to_cstring(&path, "path")?;
    capi_status(capi::ndb_vacuum(path_c.as_ptr()))?;

    let (ndb_path, _) = derive_paths(Path::new(&path));
    let ndb_meta = fs::metadata(&ndb_path).map_err(napi_err)?;
    let new_file_pages = ndb_meta.len().div_ceil(4096);

    Ok(json!({
        "ndbPath": ndb_path.to_string_lossy(),
        "backupPath": format!("{}.vacuum.bak", ndb_path.to_string_lossy()),
        "oldNextPageId": new_file_pages,
        "newNextPageId": new_file_pages,
        "copiedDataPages": new_file_pages.saturating_sub(2),
        "oldFilePages": new_file_pages,
        "newFilePages": new_file_pages,
    }))
}

#[napi]
pub fn backup(path: String, backup_dir: String) -> Result<JsonValue> {
    let path_c = to_cstring(&path, "path")?;
    let backup_dir_c = to_cstring(&backup_dir, "backup_dir")?;
    capi_status(capi::ndb_backup(path_c.as_ptr(), backup_dir_c.as_ptr()))?;

    let mut candidates: Vec<_> = fs::read_dir(&backup_dir)
        .map_err(napi_err)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .collect();
    candidates.sort_by_key(|entry| entry.metadata().and_then(|m| m.modified()).ok());

    let latest = candidates
        .last()
        .ok_or_else(|| napi_err("backup directory is empty after backup"))?;
    let latest_path = latest.path();
    let id = latest.file_name().to_string_lossy().to_string();

    let mut size_bytes: u64 = 0;
    let mut file_count: u64 = 0;
    for entry in fs::read_dir(&latest_path).map_err(napi_err)? {
        let entry = entry.map_err(napi_err)?;
        let meta = entry.metadata().map_err(napi_err)?;
        if meta.is_file() {
            file_count += 1;
            size_bytes = size_bytes.saturating_add(meta.len());
        }
    }

    Ok(json!({
        "id": id,
        "createdAt": format!("{:?}", std::time::SystemTime::now()),
        "sizeBytes": size_bytes,
        "fileCount": file_count,
        "nervusdbVersion": "1.0.0",
        "checkpointTxid": 0,
        "checkpointEpoch": 0,
    }))
}

#[napi]
pub fn bulkload(path: String, nodes: Vec<BulkNodeInput>, edges: Vec<BulkEdgeInput>) -> Result<()> {
    let path_c = to_cstring(&path, "path")?;

    let mut node_items = Vec::with_capacity(nodes.len());
    for node in nodes {
        if node.external_id < 0 {
            return Err(napi_err("bulk node external_id must be >= 0"));
        }
        node_items.push(json!({
            "external_id": node.external_id,
            "label": node.label,
            "properties": node.properties.unwrap_or(JsonValue::Object(Default::default())),
        }));
    }

    let mut edge_items = Vec::with_capacity(edges.len());
    for edge in edges {
        if edge.src_external_id < 0 || edge.dst_external_id < 0 {
            return Err(napi_err("bulk edge external ids must be >= 0"));
        }
        edge_items.push(json!({
            "src_external_id": edge.src_external_id,
            "rel_type": edge.rel_type,
            "dst_external_id": edge.dst_external_id,
            "properties": edge.properties.unwrap_or(JsonValue::Object(Default::default())),
        }));
    }

    let nodes_c = to_cstring(
        &serde_json::to_string(&node_items).map_err(napi_err)?,
        "nodes_json",
    )?;
    let edges_c = to_cstring(
        &serde_json::to_string(&edge_items).map_err(napi_err)?,
        "edges_json",
    )?;

    capi_status(capi::ndb_bulkload(
        path_c.as_ptr(),
        nodes_c.as_ptr(),
        edges_c.as_ptr(),
    ))
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
