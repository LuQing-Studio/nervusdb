#![allow(clippy::not_unsafe_ptr_arg_deref)]

use nervusdb_core as core;
use nervusdb_query::{Params, Row, Value, ast, prepare};
use serde_json::{Map as JsonMap, Value as JsonValue, json};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::{CStr, CString, c_char, c_int};
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

pub const NDB_OK: c_int = 0;
pub const NDB_ERR_INVALID_ARGUMENT: c_int = 1;
pub const NDB_ERR_NULL_POINTER: c_int = 2;
pub const NDB_ERR_SYNTAX: c_int = 1001;
pub const NDB_ERR_EXECUTION: c_int = 1002;
pub const NDB_ERR_STORAGE: c_int = 1003;
pub const NDB_ERR_COMPATIBILITY: c_int = 1004;
pub const NDB_ERR_BUSY: c_int = 1005;
pub const NDB_ERR_UNSUPPORTED: c_int = 1006;
pub const NDB_ERR_INTERNAL: c_int = 1099;

pub const NDB_ERRCAT_NONE: c_int = 0;
pub const NDB_ERRCAT_SYNTAX: c_int = 1;
pub const NDB_ERRCAT_EXECUTION: c_int = 2;
pub const NDB_ERRCAT_STORAGE: c_int = 3;
pub const NDB_ERRCAT_COMPATIBILITY: c_int = 4;

pub const NDB_STEP_ROW: c_int = 1;
pub const NDB_STEP_DONE: c_int = 2;
pub const NDB_STEP_ERROR: c_int = 3;

pub const NDB_COL_NULL: c_int = 0;
pub const NDB_COL_BOOL: c_int = 1;
pub const NDB_COL_INT64: c_int = 2;
pub const NDB_COL_DOUBLE: c_int = 3;
pub const NDB_COL_STRING: c_int = 4;
pub const NDB_COL_LIST: c_int = 5;
pub const NDB_COL_MAP: c_int = 6;
pub const NDB_COL_NODE: c_int = 7;
pub const NDB_COL_RELATIONSHIP: c_int = 8;
pub const NDB_COL_PATH: c_int = 9;
pub const NDB_COL_OTHER: c_int = 10;

#[repr(C)]
pub struct ndb_db_t {
    _private: [u8; 0],
}

#[repr(C)]
pub struct ndb_stmt_t {
    _private: [u8; 0],
}

#[repr(C)]
pub struct ndb_txn_t {
    _private: [u8; 0],
}

#[repr(C)]
pub struct ndb_result_t {
    _private: [u8; 0],
}

struct DbHandle {
    db: Option<core::Db>,
    active_txn_count: AtomicUsize,
}

struct TxnHandle {
    db: *mut ndb_db_t,
    txn: Option<core::WriteTxn<'static>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StmtMode {
    Read,
    Write,
}

struct StmtHandle {
    db: *mut ndb_db_t,
    mode: StmtMode,
    cypher: String,
    params: BTreeMap<String, Value>,
    executed: bool,
    rows: Vec<Row>,
    cursor: usize,
    current: Option<Row>,
    write_count: u32,
}

struct ResultHandle {
    json: CString,
}

#[derive(Clone, Default)]
struct LastError {
    code: c_int,
    category: c_int,
    message: String,
}

#[derive(Debug)]
struct ApiError {
    code: c_int,
    category: c_int,
    message: String,
}

type ApiResult<T> = Result<T, ApiError>;

thread_local! {
    static LAST_ERROR: RefCell<LastError> = RefCell::new(LastError::default());
}

impl ApiError {
    fn new(code: c_int, category: c_int, message: impl Into<String>) -> Self {
        Self {
            code,
            category,
            message: message.into(),
        }
    }

    fn invalid(message: impl Into<String>) -> Self {
        Self::new(
            NDB_ERR_INVALID_ARGUMENT,
            NDB_ERRCAT_EXECUTION,
            message.into(),
        )
    }

    fn null_pointer(name: &str) -> Self {
        Self::new(
            NDB_ERR_NULL_POINTER,
            NDB_ERRCAT_EXECUTION,
            format!("{name} is null"),
        )
    }

    fn syntax(message: impl Into<String>) -> Self {
        Self::new(NDB_ERR_SYNTAX, NDB_ERRCAT_SYNTAX, message.into())
    }

    fn execution(message: impl Into<String>) -> Self {
        Self::new(NDB_ERR_EXECUTION, NDB_ERRCAT_EXECUTION, message.into())
    }

    fn storage(message: impl Into<String>) -> Self {
        Self::new(NDB_ERR_STORAGE, NDB_ERRCAT_STORAGE, message.into())
    }

    fn compatibility(message: impl Into<String>) -> Self {
        Self::new(
            NDB_ERR_COMPATIBILITY,
            NDB_ERRCAT_COMPATIBILITY,
            message.into(),
        )
    }

    fn busy(message: impl Into<String>) -> Self {
        Self::new(NDB_ERR_BUSY, NDB_ERRCAT_EXECUTION, message.into())
    }

    fn internal(message: impl Into<String>) -> Self {
        Self::new(NDB_ERR_INTERNAL, NDB_ERRCAT_EXECUTION, message.into())
    }

    fn from_core(err: core::Error) -> Self {
        match err {
            core::Error::Compatibility(msg) => Self::compatibility(msg),
            core::Error::Storage(msg) => Self::storage(msg),
            core::Error::Query(msg) => Self::from_query_message(&msg),
            core::Error::Other(msg) => Self::from_query_message(&msg),
            core::Error::Io(io_err) => Self::storage(io_err.to_string()),
        }
    }

    fn from_query_message(msg: &str) -> Self {
        let lower = msg.to_lowercase();
        if lower.contains("syntax")
            || lower.contains("parse")
            || lower.contains("unexpected token")
            || lower.contains("unexpected character")
            || lower.starts_with("expected ")
            || lower.contains("variablealreadybound")
            || lower.contains("variabletypeconflict")
        {
            return Self::syntax(msg.to_string());
        }

        if lower.contains("storage format mismatch") || lower.contains("compatibility") {
            return Self::compatibility(msg.to_string());
        }

        if lower.contains("wal")
            || lower.contains("checkpoint")
            || lower.contains("io error")
            || lower.contains("permission denied")
            || lower.contains("disk full")
            || lower.contains("no such file")
            || lower.contains("database is closed")
        {
            return Self::storage(msg.to_string());
        }

        Self::execution(msg.to_string())
    }
}

fn clear_last_error() {
    LAST_ERROR.with(|slot| {
        *slot.borrow_mut() = LastError::default();
    });
}

fn set_last_error(err: &ApiError) {
    LAST_ERROR.with(|slot| {
        *slot.borrow_mut() = LastError {
            code: err.code,
            category: err.category,
            message: err.message.clone(),
        };
    });
}

fn ok_status() -> c_int {
    clear_last_error();
    NDB_OK
}

fn err_status(err: ApiError) -> c_int {
    set_last_error(&err);
    err.code
}

fn cstr_to_string(ptr: *const c_char, name: &str) -> ApiResult<String> {
    if ptr.is_null() {
        return Err(ApiError::null_pointer(name));
    }
    let c = unsafe {
        // SAFETY: caller passed a non-null pointer and C ABI contract requires a valid C string.
        CStr::from_ptr(ptr)
    };
    c.to_str()
        .map(|s| s.to_string())
        .map_err(|_| ApiError::invalid(format!("{name} must be valid UTF-8")))
}

fn cstr_to_json_value(ptr: *const c_char, name: &str) -> ApiResult<JsonValue> {
    let raw = cstr_to_string(ptr, name)?;
    serde_json::from_str(&raw)
        .map_err(|e| ApiError::invalid(format!("{name} must be valid JSON: {e}")))
}

fn parse_params_json(params: *const c_char) -> ApiResult<Params> {
    let mut out = Params::new();
    if params.is_null() {
        return Ok(out);
    }

    let root = cstr_to_json_value(params, "params")?;
    let map = root
        .as_object()
        .ok_or_else(|| ApiError::invalid("params must be a JSON object"))?;

    for (k, v) in map {
        out.insert(k.clone(), json_to_query_value(v)?);
    }

    Ok(out)
}

fn params_from_map(map: &BTreeMap<String, Value>) -> Params {
    let mut params = Params::new();
    for (k, v) in map {
        params.insert(k.clone(), v.clone());
    }
    params
}

fn json_to_query_value(v: &JsonValue) -> ApiResult<Value> {
    match v {
        JsonValue::Null => Ok(Value::Null),
        JsonValue::Bool(b) => Ok(Value::Bool(*b)),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Int(i))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Float(f))
            } else {
                Err(ApiError::invalid("unsupported numeric value"))
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

fn json_to_property_value(v: &JsonValue) -> ApiResult<core::PropertyValue> {
    match v {
        JsonValue::Null => Ok(core::PropertyValue::Null),
        JsonValue::Bool(b) => Ok(core::PropertyValue::Bool(*b)),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(core::PropertyValue::Int(i))
            } else if let Some(f) = n.as_f64() {
                Ok(core::PropertyValue::Float(f))
            } else {
                Err(ApiError::invalid("unsupported numeric value"))
            }
        }
        JsonValue::String(s) => Ok(core::PropertyValue::String(s.clone())),
        JsonValue::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(json_to_property_value(item)?);
            }
            Ok(core::PropertyValue::List(out))
        }
        JsonValue::Object(map) => {
            let mut out = BTreeMap::new();
            for (k, v) in map {
                out.insert(k.clone(), json_to_property_value(v)?);
            }
            Ok(core::PropertyValue::Map(out))
        }
    }
}

fn value_to_json(v: Value) -> JsonValue {
    match v {
        Value::Null => JsonValue::Null,
        Value::Bool(b) => json!(b),
        Value::Int(i) => json!(i),
        Value::Float(f) => json!(f),
        Value::String(s) => json!(s),
        Value::DateTime(ts) => json!({ "type": "datetime", "value": ts }),
        Value::Blob(bytes) => json!({ "type": "blob", "len": bytes.len() }),
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
            json!({ "type": "path", "nodes": nodes, "relationships": rels })
        }
        Value::NodeId(id) => json!({ "type": "node_id", "value": id }),
        Value::ExternalId(id) => json!({ "type": "external_id", "value": id }),
        Value::EdgeKey(k) => {
            json!({ "type": "edge_key", "src": k.src, "rel": k.rel, "dst": k.dst })
        }
        Value::Path(p) => {
            let edges = p
                .edges
                .into_iter()
                .map(|e| json!({ "src": e.src, "rel": e.rel, "dst": e.dst }))
                .collect::<Vec<_>>();
            json!({ "type": "path_legacy", "nodes": p.nodes, "edges": edges })
        }
    }
}

fn row_to_json(row: Row) -> JsonValue {
    let mut obj = JsonMap::new();
    for (k, v) in row.columns().iter().cloned() {
        obj.insert(k, value_to_json(v));
    }
    JsonValue::Object(obj)
}

fn write_query_contains_write(cypher: &str) -> ApiResult<bool> {
    let trimmed = cypher.trim_start();
    if trimmed.len() >= 7 && trimmed[..7].eq_ignore_ascii_case("EXPLAIN") {
        return Ok(false);
    }
    let parsed =
        nervusdb_query::parse(cypher).map_err(|e| ApiError::from_query_message(&e.to_string()))?;
    Ok(query_contains_write(&parsed))
}

fn query_contains_write(query: &ast::Query) -> bool {
    query.clauses.iter().any(clause_contains_write)
}

fn clause_contains_write(clause: &ast::Clause) -> bool {
    match clause {
        ast::Clause::Create(_)
        | ast::Clause::Merge(_)
        | ast::Clause::Set(_)
        | ast::Clause::Remove(_)
        | ast::Clause::Delete(_)
        | ast::Clause::Foreach(_) => true,
        ast::Clause::Call(ast::CallClause::Subquery(q)) => query_contains_write(q),
        ast::Clause::Union(union_clause) => query_contains_write(&union_clause.query),
        _ => false,
    }
}

unsafe fn db_handle_mut<'a>(db: *mut ndb_db_t) -> ApiResult<&'a mut DbHandle> {
    if db.is_null() {
        return Err(ApiError::null_pointer("db"));
    }
    Ok(unsafe {
        // SAFETY: pointer validity is ensured by FFI lifecycle; all handles are allocated by this crate.
        &mut *db.cast::<DbHandle>()
    })
}

unsafe fn db_handle_ref<'a>(db: *mut ndb_db_t) -> ApiResult<&'a DbHandle> {
    if db.is_null() {
        return Err(ApiError::null_pointer("db"));
    }
    Ok(unsafe {
        // SAFETY: pointer validity is ensured by FFI lifecycle; all handles are allocated by this crate.
        &*db.cast::<DbHandle>()
    })
}

fn db_ref_from_handle(handle: &DbHandle) -> ApiResult<&core::Db> {
    handle
        .db
        .as_ref()
        .ok_or_else(|| ApiError::execution("database handle has been closed"))
}

fn db_ref_from_handle_mut(handle: &mut DbHandle) -> ApiResult<&mut core::Db> {
    handle
        .db
        .as_mut()
        .ok_or_else(|| ApiError::execution("database handle has been closed"))
}

unsafe fn txn_handle_mut<'a>(txn: *mut ndb_txn_t) -> ApiResult<&'a mut TxnHandle> {
    if txn.is_null() {
        return Err(ApiError::null_pointer("txn"));
    }
    Ok(unsafe {
        // SAFETY: pointer validity is ensured by FFI lifecycle; all handles are allocated by this crate.
        &mut *txn.cast::<TxnHandle>()
    })
}

unsafe fn stmt_handle_mut<'a>(stmt: *mut ndb_stmt_t) -> ApiResult<&'a mut StmtHandle> {
    if stmt.is_null() {
        return Err(ApiError::null_pointer("stmt"));
    }
    Ok(unsafe {
        // SAFETY: pointer validity is ensured by FFI lifecycle; all handles are allocated by this crate.
        &mut *stmt.cast::<StmtHandle>()
    })
}

unsafe fn result_handle_ref<'a>(result: *mut ndb_result_t) -> ApiResult<&'a ResultHandle> {
    if result.is_null() {
        return Err(ApiError::null_pointer("result"));
    }
    Ok(unsafe {
        // SAFETY: pointer validity is ensured by FFI lifecycle; all handles are allocated by this crate.
        &*result.cast::<ResultHandle>()
    })
}

fn decrement_active_txn_count(db: *mut ndb_db_t) {
    if db.is_null() {
        return;
    }
    let handle = unsafe {
        // SAFETY: this is best-effort bookkeeping; only pointers created by this crate are passed here.
        db.cast::<DbHandle>().as_ref()
    };
    if let Some(handle) = handle {
        let current = handle.active_txn_count.load(Ordering::SeqCst);
        if current > 0 {
            handle.active_txn_count.fetch_sub(1, Ordering::SeqCst);
        }
    }
}

fn make_result_handle_from_json(value: JsonValue) -> ApiResult<*mut ndb_result_t> {
    let text = serde_json::to_string(&value)
        .map_err(|e| ApiError::internal(format!("json encode failed: {e}")))?;
    let json =
        CString::new(text).map_err(|_| ApiError::internal("json text contains interior NUL"))?;
    let handle = Box::new(ResultHandle { json });
    Ok(Box::into_raw(handle).cast::<ndb_result_t>())
}

fn make_result_handle_from_rows(rows: Vec<Row>) -> ApiResult<*mut ndb_result_t> {
    let value = JsonValue::Array(rows.into_iter().map(row_to_json).collect());
    make_result_handle_from_json(value)
}

fn execute_read_rows(db: &core::Db, cypher: &str, params: &Params) -> ApiResult<Vec<Row>> {
    if write_query_contains_write(cypher)? {
        return Err(ApiError::execution(
            "ndb_query/read API does not accept write statements",
        ));
    }
    let prepared = prepare(cypher).map_err(|e| ApiError::from_query_message(&e.to_string()))?;
    let snapshot = db.snapshot();
    let rows = prepared
        .execute_streaming(&snapshot, params)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ApiError::from_query_message(&e.to_string()))?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let mut reified = Vec::with_capacity(row.columns().len());
        for (k, v) in row.columns().iter().cloned() {
            let rv = v
                .reify(&snapshot)
                .map_err(|e| ApiError::from_query_message(&e.to_string()))?;
            reified.push((k, rv));
        }
        out.push(Row::new(reified));
    }
    Ok(out)
}

fn execute_write_count(db: &core::Db, cypher: &str, params: &Params) -> ApiResult<u32> {
    if !write_query_contains_write(cypher)? {
        return Err(ApiError::execution(
            "ndb_execute_write API expects a write statement",
        ));
    }
    let prepared = prepare(cypher).map_err(|e| ApiError::from_query_message(&e.to_string()))?;
    let snapshot = db.snapshot();
    let mut txn = db.begin_write();
    let (_rows, write_count) = prepared
        .execute_mixed(&snapshot, &mut txn, params)
        .map_err(|e| ApiError::from_query_message(&e.to_string()))?;
    txn.commit().map_err(ApiError::from_core)?;
    Ok(write_count)
}

fn execute_write_in_txn(
    db: &core::Db,
    txn: &mut core::WriteTxn<'static>,
    cypher: &str,
    params: &Params,
) -> ApiResult<u32> {
    if !write_query_contains_write(cypher)? {
        return Err(ApiError::execution(
            "ndb_txn_query API expects a write statement",
        ));
    }
    let prepared = prepare(cypher).map_err(|e| ApiError::from_query_message(&e.to_string()))?;
    let snapshot = db.snapshot();
    let (_rows, write_count) = prepared
        .execute_mixed(&snapshot, txn, params)
        .map_err(|e| ApiError::from_query_message(&e.to_string()))?;
    Ok(write_count)
}

fn stmt_execute_if_needed(stmt: &mut StmtHandle) -> ApiResult<()> {
    if stmt.executed {
        return Ok(());
    }

    let db_handle = unsafe { db_handle_ref(stmt.db)? };
    let db = db_ref_from_handle(db_handle)?;
    let params = params_from_map(&stmt.params);
    match stmt.mode {
        StmtMode::Read => {
            stmt.rows = execute_read_rows(db, &stmt.cypher, &params)?;
            stmt.cursor = 0;
            stmt.current = None;
            stmt.write_count = 0;
        }
        StmtMode::Write => {
            stmt.write_count = execute_write_count(db, &stmt.cypher, &params)?;
            stmt.rows.clear();
            stmt.cursor = 0;
            stmt.current = None;
        }
    }
    stmt.executed = true;
    Ok(())
}

fn stmt_current_value(stmt: &StmtHandle, col: usize) -> ApiResult<&Value> {
    let row = stmt
        .current
        .as_ref()
        .ok_or_else(|| ApiError::execution("no current row; call ndb_stmt_step first"))?;
    row.columns()
        .get(col)
        .map(|(_, v)| v)
        .ok_or_else(|| ApiError::invalid("column index out of range"))
}

fn value_kind(v: &Value) -> c_int {
    match v {
        Value::Null => NDB_COL_NULL,
        Value::Bool(_) => NDB_COL_BOOL,
        Value::Int(_) | Value::DateTime(_) | Value::NodeId(_) => NDB_COL_INT64,
        Value::ExternalId(_) => NDB_COL_INT64,
        Value::Float(_) => NDB_COL_DOUBLE,
        Value::String(_) => NDB_COL_STRING,
        Value::List(_) => NDB_COL_LIST,
        Value::Map(_) => NDB_COL_MAP,
        Value::Node(_) => NDB_COL_NODE,
        Value::Relationship(_) => NDB_COL_RELATIONSHIP,
        Value::Path(_) | Value::ReifiedPath(_) | Value::EdgeKey(_) => NDB_COL_PATH,
        Value::Blob(_) => NDB_COL_OTHER,
    }
}

fn write_out_c_string(out: *mut *mut c_char, text: &str) -> ApiResult<()> {
    if out.is_null() {
        return Err(ApiError::null_pointer("out"));
    }
    let c = CString::new(text).map_err(|_| ApiError::internal("string contains interior NUL"))?;
    unsafe {
        // SAFETY: caller provided a valid output pointer.
        *out = c.into_raw();
    }
    Ok(())
}

fn parse_bulk_nodes(nodes_json: *const c_char) -> ApiResult<Vec<core::BulkNode>> {
    if nodes_json.is_null() {
        return Ok(Vec::new());
    }
    let root = cstr_to_json_value(nodes_json, "nodes")?;
    let arr = root
        .as_array()
        .ok_or_else(|| ApiError::invalid("nodes must be a JSON array"))?;

    let mut out = Vec::with_capacity(arr.len());
    for node in arr {
        let obj = node
            .as_object()
            .ok_or_else(|| ApiError::invalid("node entry must be an object"))?;
        let external_id = obj
            .get("external_id")
            .and_then(JsonValue::as_u64)
            .ok_or_else(|| ApiError::invalid("node.external_id must be an unsigned integer"))?;
        let label = obj
            .get("label")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| ApiError::invalid("node.label must be a string"))?;
        let mut properties = BTreeMap::new();
        if let Some(props) = obj.get("properties") {
            let props_obj = props
                .as_object()
                .ok_or_else(|| ApiError::invalid("node.properties must be an object"))?;
            for (k, v) in props_obj {
                properties.insert(k.clone(), json_to_property_value(v)?);
            }
        }
        out.push(core::BulkNode {
            external_id,
            label: label.to_string(),
            properties,
        });
    }
    Ok(out)
}

fn parse_bulk_edges(edges_json: *const c_char) -> ApiResult<Vec<core::BulkEdge>> {
    if edges_json.is_null() {
        return Ok(Vec::new());
    }
    let root = cstr_to_json_value(edges_json, "edges")?;
    let arr = root
        .as_array()
        .ok_or_else(|| ApiError::invalid("edges must be a JSON array"))?;

    let mut out = Vec::with_capacity(arr.len());
    for edge in arr {
        let obj = edge
            .as_object()
            .ok_or_else(|| ApiError::invalid("edge entry must be an object"))?;
        let src_external_id = obj
            .get("src_external_id")
            .and_then(JsonValue::as_u64)
            .ok_or_else(|| ApiError::invalid("edge.src_external_id must be an unsigned integer"))?;
        let dst_external_id = obj
            .get("dst_external_id")
            .and_then(JsonValue::as_u64)
            .ok_or_else(|| ApiError::invalid("edge.dst_external_id must be an unsigned integer"))?;
        let rel_type = obj
            .get("rel_type")
            .and_then(JsonValue::as_str)
            .ok_or_else(|| ApiError::invalid("edge.rel_type must be a string"))?;
        let mut properties = BTreeMap::new();
        if let Some(props) = obj.get("properties") {
            let props_obj = props
                .as_object()
                .ok_or_else(|| ApiError::invalid("edge.properties must be an object"))?;
            for (k, v) in props_obj {
                properties.insert(k.clone(), json_to_property_value(v)?);
            }
        }
        out.push(core::BulkEdge {
            src_external_id,
            rel_type: rel_type.to_string(),
            dst_external_id,
            properties,
        });
    }
    Ok(out)
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_last_error_code() -> c_int {
    LAST_ERROR.with(|slot| slot.borrow().code)
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_last_error_category() -> c_int {
    LAST_ERROR.with(|slot| slot.borrow().category)
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_last_error_message(buf: *mut c_char, len: usize) -> usize {
    let message = LAST_ERROR.with(|slot| slot.borrow().message.clone());
    let bytes = message.as_bytes();
    if buf.is_null() || len == 0 {
        return bytes.len();
    }

    let copy_len = bytes.len().min(len.saturating_sub(1));
    unsafe {
        // SAFETY: caller provided writable buffer of at least `len` bytes.
        ptr::copy_nonoverlapping(bytes.as_ptr().cast::<c_char>(), buf, copy_len);
        *buf.add(copy_len) = 0;
    }
    bytes.len()
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_open(path: *const c_char, out_db: *mut *mut ndb_db_t) -> c_int {
    let result = (|| -> ApiResult<()> {
        if out_db.is_null() {
            return Err(ApiError::null_pointer("out_db"));
        }
        let path = cstr_to_string(path, "path")?;
        let db = core::Db::open(path).map_err(ApiError::from_core)?;
        let handle = Box::new(DbHandle {
            db: Some(db),
            active_txn_count: AtomicUsize::new(0),
        });
        unsafe {
            // SAFETY: out pointer validated above.
            *out_db = Box::into_raw(handle).cast::<ndb_db_t>();
        }
        Ok(())
    })();

    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_open_paths(
    ndb_path: *const c_char,
    wal_path: *const c_char,
    out_db: *mut *mut ndb_db_t,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        if out_db.is_null() {
            return Err(ApiError::null_pointer("out_db"));
        }
        let ndb_path = cstr_to_string(ndb_path, "ndb_path")?;
        let wal_path = cstr_to_string(wal_path, "wal_path")?;
        let db = core::Db::open_paths(ndb_path, wal_path).map_err(ApiError::from_core)?;
        let handle = Box::new(DbHandle {
            db: Some(db),
            active_txn_count: AtomicUsize::new(0),
        });
        unsafe {
            // SAFETY: out pointer validated above.
            *out_db = Box::into_raw(handle).cast::<ndb_db_t>();
        }
        Ok(())
    })();

    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_close(db: *mut ndb_db_t) -> c_int {
    let result = (|| -> ApiResult<()> {
        if db.is_null() {
            return Err(ApiError::null_pointer("db"));
        }
        let boxed = unsafe {
            // SAFETY: pointer validity is guaranteed by lifecycle; function takes ownership.
            Box::from_raw(db.cast::<DbHandle>())
        };
        if boxed.active_txn_count.load(Ordering::SeqCst) > 0 {
            let raw = Box::into_raw(boxed);
            let _ = raw;
            return Err(ApiError::busy(
                "cannot close database while write transaction is active",
            ));
        }
        if let Some(real_db) = boxed.db {
            real_db.close().map_err(ApiError::from_core)?;
        }
        Ok(())
    })();

    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_query(
    db: *mut ndb_db_t,
    cypher: *const c_char,
    params_json: *const c_char,
    out_result: *mut *mut ndb_result_t,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        if out_result.is_null() {
            return Err(ApiError::null_pointer("out_result"));
        }
        let cypher = cstr_to_string(cypher, "cypher")?;
        let params = parse_params_json(params_json)?;
        let handle = unsafe { db_handle_ref(db)? };
        let db_ref = db_ref_from_handle(handle)?;
        let rows = execute_read_rows(db_ref, &cypher, &params)?;
        let result_ptr = make_result_handle_from_rows(rows)?;
        unsafe {
            // SAFETY: out pointer validated above.
            *out_result = result_ptr;
        }
        Ok(())
    })();

    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_execute_write(
    db: *mut ndb_db_t,
    cypher: *const c_char,
    params_json: *const c_char,
    out_summary: *mut u32,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        let cypher = cstr_to_string(cypher, "cypher")?;
        let params = parse_params_json(params_json)?;
        let handle = unsafe { db_handle_ref(db)? };
        let db_ref = db_ref_from_handle(handle)?;
        let affected = execute_write_count(db_ref, &cypher, &params)?;
        if !out_summary.is_null() {
            unsafe {
                // SAFETY: output pointer is optional and only written when non-null.
                *out_summary = affected;
            }
        }
        Ok(())
    })();

    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_result_to_json(
    result: *mut ndb_result_t,
    out_json: *mut *mut c_char,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        if out_json.is_null() {
            return Err(ApiError::null_pointer("out_json"));
        }
        let handle = unsafe { result_handle_ref(result)? };
        let text = handle
            .json
            .to_str()
            .map_err(|_| ApiError::internal("stored JSON is not UTF-8"))?;
        let out = CString::new(text)
            .map_err(|_| ApiError::internal("stored JSON contains interior NUL"))?;
        unsafe {
            // SAFETY: out pointer validated above.
            *out_json = out.into_raw();
        }
        Ok(())
    })();

    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_result_free(result: *mut ndb_result_t) {
    if result.is_null() {
        return;
    }
    unsafe {
        // SAFETY: pointer was allocated by this crate; ownership transferred to caller.
        drop(Box::from_raw(result.cast::<ResultHandle>()));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_string_free(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    unsafe {
        // SAFETY: pointer was allocated via CString::into_raw by this crate.
        drop(CString::from_raw(s));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_begin_write(db: *mut ndb_db_t, out_txn: *mut *mut ndb_txn_t) -> c_int {
    let result = (|| -> ApiResult<()> {
        if out_txn.is_null() {
            return Err(ApiError::null_pointer("out_txn"));
        }
        let handle = unsafe { db_handle_mut(db)? };
        let db_ref = db_ref_from_handle_mut(handle)?;
        let txn = db_ref.begin_write();
        let txn_static: core::WriteTxn<'static> = unsafe {
            // SAFETY: lifecycle is enforced by retaining parent DB handle and active-txn gate on close.
            std::mem::transmute::<core::WriteTxn<'_>, core::WriteTxn<'static>>(txn)
        };
        handle.active_txn_count.fetch_add(1, Ordering::SeqCst);
        let txn_handle = Box::new(TxnHandle {
            db,
            txn: Some(txn_static),
        });
        unsafe {
            // SAFETY: out pointer validated above.
            *out_txn = Box::into_raw(txn_handle).cast::<ndb_txn_t>();
        }
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_txn_query(
    txn: *mut ndb_txn_t,
    cypher: *const c_char,
    params_json: *const c_char,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        let cypher = cstr_to_string(cypher, "cypher")?;
        let params = parse_params_json(params_json)?;
        let txn_handle = unsafe { txn_handle_mut(txn)? };
        let inner = txn_handle
            .txn
            .as_mut()
            .ok_or_else(|| ApiError::execution("transaction is not active"))?;
        let db_handle = unsafe { db_handle_ref(txn_handle.db)? };
        let db_ref = db_ref_from_handle(db_handle)?;
        let _ = execute_write_in_txn(db_ref, inner, &cypher, &params)?;
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_txn_commit(txn: *mut ndb_txn_t) -> c_int {
    let result = (|| -> ApiResult<()> {
        if txn.is_null() {
            return Err(ApiError::null_pointer("txn"));
        }
        let mut boxed = unsafe {
            // SAFETY: pointer validity is guaranteed by lifecycle; function takes ownership.
            Box::from_raw(txn.cast::<TxnHandle>())
        };
        let db_ptr = boxed.db;
        let tx = boxed
            .txn
            .take()
            .ok_or_else(|| ApiError::execution("transaction is not active"))?;
        tx.commit().map_err(ApiError::from_core)?;
        decrement_active_txn_count(db_ptr);
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_txn_rollback(txn: *mut ndb_txn_t) -> c_int {
    let result = (|| -> ApiResult<()> {
        if txn.is_null() {
            return Err(ApiError::null_pointer("txn"));
        }
        let mut boxed = unsafe {
            // SAFETY: pointer validity is guaranteed by lifecycle; function takes ownership.
            Box::from_raw(txn.cast::<TxnHandle>())
        };
        let db_ptr = boxed.db;
        let _ = boxed.txn.take();
        decrement_active_txn_count(db_ptr);
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_txn_create_node(
    txn: *mut ndb_txn_t,
    external_id: u64,
    label_id: u32,
    out_node_id: *mut u32,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        if out_node_id.is_null() {
            return Err(ApiError::null_pointer("out_node_id"));
        }
        let txn_handle = unsafe { txn_handle_mut(txn)? };
        let inner = txn_handle
            .txn
            .as_mut()
            .ok_or_else(|| ApiError::execution("transaction is not active"))?;
        let node_id = inner
            .create_node(external_id, label_id)
            .map_err(ApiError::from_core)?;
        unsafe {
            // SAFETY: output pointer validated above.
            *out_node_id = node_id;
        }
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_txn_get_or_create_label(
    txn: *mut ndb_txn_t,
    name: *const c_char,
    out_label_id: *mut u32,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        if out_label_id.is_null() {
            return Err(ApiError::null_pointer("out_label_id"));
        }
        let name = cstr_to_string(name, "name")?;
        let txn_handle = unsafe { txn_handle_mut(txn)? };
        let inner = txn_handle
            .txn
            .as_mut()
            .ok_or_else(|| ApiError::execution("transaction is not active"))?;
        let label_id = inner
            .get_or_create_label(&name)
            .map_err(ApiError::from_core)?;
        unsafe {
            // SAFETY: output pointer validated above.
            *out_label_id = label_id;
        }
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_txn_get_or_create_rel_type(
    txn: *mut ndb_txn_t,
    name: *const c_char,
    out_rel_type_id: *mut u32,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        if out_rel_type_id.is_null() {
            return Err(ApiError::null_pointer("out_rel_type_id"));
        }
        let name = cstr_to_string(name, "name")?;
        let txn_handle = unsafe { txn_handle_mut(txn)? };
        let inner = txn_handle
            .txn
            .as_mut()
            .ok_or_else(|| ApiError::execution("transaction is not active"))?;
        let rel_type_id = inner
            .get_or_create_rel_type(&name)
            .map_err(ApiError::from_core)?;
        unsafe {
            // SAFETY: output pointer validated above.
            *out_rel_type_id = rel_type_id;
        }
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_txn_create_edge(txn: *mut ndb_txn_t, src: u32, rel: u32, dst: u32) -> c_int {
    let result = (|| -> ApiResult<()> {
        let txn_handle = unsafe { txn_handle_mut(txn)? };
        let inner = txn_handle
            .txn
            .as_mut()
            .ok_or_else(|| ApiError::execution("transaction is not active"))?;
        inner.create_edge(src, rel, dst);
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_txn_tombstone_node(txn: *mut ndb_txn_t, node: u32) -> c_int {
    let result = (|| -> ApiResult<()> {
        let txn_handle = unsafe { txn_handle_mut(txn)? };
        let inner = txn_handle
            .txn
            .as_mut()
            .ok_or_else(|| ApiError::execution("transaction is not active"))?;
        inner.tombstone_node(node);
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_txn_tombstone_edge(
    txn: *mut ndb_txn_t,
    src: u32,
    rel: u32,
    dst: u32,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        let txn_handle = unsafe { txn_handle_mut(txn)? };
        let inner = txn_handle
            .txn
            .as_mut()
            .ok_or_else(|| ApiError::execution("transaction is not active"))?;
        inner.tombstone_edge(src, rel, dst);
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_txn_set_node_property(
    txn: *mut ndb_txn_t,
    node: u32,
    key: *const c_char,
    value_json: *const c_char,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        let key = cstr_to_string(key, "key")?;
        let value = cstr_to_json_value(value_json, "value_json")?;
        let prop = json_to_property_value(&value)?;
        let txn_handle = unsafe { txn_handle_mut(txn)? };
        let inner = txn_handle
            .txn
            .as_mut()
            .ok_or_else(|| ApiError::execution("transaction is not active"))?;
        inner
            .set_node_property(node, key, prop)
            .map_err(ApiError::from_core)?;
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_txn_set_edge_property(
    txn: *mut ndb_txn_t,
    src: u32,
    rel: u32,
    dst: u32,
    key: *const c_char,
    value_json: *const c_char,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        let key = cstr_to_string(key, "key")?;
        let value = cstr_to_json_value(value_json, "value_json")?;
        let prop = json_to_property_value(&value)?;
        let txn_handle = unsafe { txn_handle_mut(txn)? };
        let inner = txn_handle
            .txn
            .as_mut()
            .ok_or_else(|| ApiError::execution("transaction is not active"))?;
        inner
            .set_edge_property(src, rel, dst, key, prop)
            .map_err(ApiError::from_core)?;
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_txn_remove_node_property(
    txn: *mut ndb_txn_t,
    node: u32,
    key: *const c_char,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        let key = cstr_to_string(key, "key")?;
        let txn_handle = unsafe { txn_handle_mut(txn)? };
        let inner = txn_handle
            .txn
            .as_mut()
            .ok_or_else(|| ApiError::execution("transaction is not active"))?;
        inner
            .remove_node_property(node, &key)
            .map_err(ApiError::from_core)?;
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_txn_remove_edge_property(
    txn: *mut ndb_txn_t,
    src: u32,
    rel: u32,
    dst: u32,
    key: *const c_char,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        let key = cstr_to_string(key, "key")?;
        let txn_handle = unsafe { txn_handle_mut(txn)? };
        let inner = txn_handle
            .txn
            .as_mut()
            .ok_or_else(|| ApiError::execution("transaction is not active"))?;
        inner
            .remove_edge_property(src, rel, dst, &key)
            .map_err(ApiError::from_core)?;
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_txn_set_vector(
    txn: *mut ndb_txn_t,
    node: u32,
    vector: *const f32,
    len: usize,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        if vector.is_null() && len > 0 {
            return Err(ApiError::null_pointer("vector"));
        }
        let slice = if len == 0 {
            &[][..]
        } else {
            unsafe {
                // SAFETY: pointer validated above and caller provides `len` elements.
                std::slice::from_raw_parts(vector, len)
            }
        };
        let txn_handle = unsafe { txn_handle_mut(txn)? };
        let inner = txn_handle
            .txn
            .as_mut()
            .ok_or_else(|| ApiError::execution("transaction is not active"))?;
        inner
            .set_vector(node, slice.to_vec())
            .map_err(ApiError::from_core)?;
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_compact(db: *mut ndb_db_t) -> c_int {
    let result = (|| -> ApiResult<()> {
        let handle = unsafe { db_handle_ref(db)? };
        let db_ref = db_ref_from_handle(handle)?;
        db_ref.compact().map_err(ApiError::from_core)
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_checkpoint(db: *mut ndb_db_t) -> c_int {
    let result = (|| -> ApiResult<()> {
        let handle = unsafe { db_handle_ref(db)? };
        let db_ref = db_ref_from_handle(handle)?;
        db_ref.checkpoint().map_err(ApiError::from_core)
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_create_index(
    db: *mut ndb_db_t,
    label: *const c_char,
    property: *const c_char,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        let label = cstr_to_string(label, "label")?;
        let property = cstr_to_string(property, "property")?;
        let handle = unsafe { db_handle_ref(db)? };
        let db_ref = db_ref_from_handle(handle)?;
        db_ref
            .create_index(&label, &property)
            .map_err(ApiError::from_core)
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_search_vector(
    db: *mut ndb_db_t,
    query: *const f32,
    query_len: usize,
    k: u32,
    out_result: *mut *mut ndb_result_t,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        if out_result.is_null() {
            return Err(ApiError::null_pointer("out_result"));
        }
        if query.is_null() && query_len > 0 {
            return Err(ApiError::null_pointer("query"));
        }
        let query_slice = if query_len == 0 {
            &[][..]
        } else {
            unsafe {
                // SAFETY: pointer validated above and caller provides `query_len` items.
                std::slice::from_raw_parts(query, query_len)
            }
        };
        let handle = unsafe { db_handle_ref(db)? };
        let db_ref = db_ref_from_handle(handle)?;
        let rows = db_ref
            .search_vector(query_slice, k as usize)
            .map_err(ApiError::from_core)?;
        let json_rows = JsonValue::Array(
            rows.into_iter()
                .map(|(node_id, distance)| json!({ "node_id": node_id, "distance": distance }))
                .collect(),
        );
        let result_ptr = make_result_handle_from_json(json_rows)?;
        unsafe {
            // SAFETY: out pointer validated above.
            *out_result = result_ptr;
        }
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_vacuum(path: *const c_char) -> c_int {
    let result = (|| -> ApiResult<()> {
        let path = cstr_to_string(path, "path")?;
        core::vacuum(path).map_err(ApiError::from_core)?;
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_backup(path: *const c_char, backup_dir: *const c_char) -> c_int {
    let result = (|| -> ApiResult<()> {
        let path = cstr_to_string(path, "path")?;
        let backup_dir = cstr_to_string(backup_dir, "backup_dir")?;
        core::backup(path, backup_dir).map_err(ApiError::from_core)?;
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_bulkload(
    path: *const c_char,
    nodes_json: *const c_char,
    edges_json: *const c_char,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        let path = cstr_to_string(path, "path")?;
        let nodes = parse_bulk_nodes(nodes_json)?;
        let edges = parse_bulk_edges(edges_json)?;
        core::bulkload(path, nodes, edges).map_err(ApiError::from_core)
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_prepare_read(
    db: *mut ndb_db_t,
    cypher: *const c_char,
    out_stmt: *mut *mut ndb_stmt_t,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        if out_stmt.is_null() {
            return Err(ApiError::null_pointer("out_stmt"));
        }
        let _ = unsafe { db_handle_ref(db)? };
        let cypher = cstr_to_string(cypher, "cypher")?;
        if write_query_contains_write(&cypher)? {
            return Err(ApiError::execution(
                "ndb_prepare_read does not accept write statements",
            ));
        }
        let stmt = Box::new(StmtHandle {
            db,
            mode: StmtMode::Read,
            cypher,
            params: BTreeMap::new(),
            executed: false,
            rows: Vec::new(),
            cursor: 0,
            current: None,
            write_count: 0,
        });
        unsafe {
            // SAFETY: output pointer validated above.
            *out_stmt = Box::into_raw(stmt).cast::<ndb_stmt_t>();
        }
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_prepare_write(
    db: *mut ndb_db_t,
    cypher: *const c_char,
    out_stmt: *mut *mut ndb_stmt_t,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        if out_stmt.is_null() {
            return Err(ApiError::null_pointer("out_stmt"));
        }
        let _ = unsafe { db_handle_ref(db)? };
        let cypher = cstr_to_string(cypher, "cypher")?;
        if !write_query_contains_write(&cypher)? {
            return Err(ApiError::execution(
                "ndb_prepare_write expects a write statement",
            ));
        }
        let stmt = Box::new(StmtHandle {
            db,
            mode: StmtMode::Write,
            cypher,
            params: BTreeMap::new(),
            executed: false,
            rows: Vec::new(),
            cursor: 0,
            current: None,
            write_count: 0,
        });
        unsafe {
            // SAFETY: output pointer validated above.
            *out_stmt = Box::into_raw(stmt).cast::<ndb_stmt_t>();
        }
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

fn stmt_bind_value(stmt: *mut ndb_stmt_t, name: *const c_char, value: Value) -> c_int {
    let result = (|| -> ApiResult<()> {
        let key = cstr_to_string(name, "name")?;
        if key.is_empty() {
            return Err(ApiError::invalid("bind name cannot be empty"));
        }
        let handle = unsafe { stmt_handle_mut(stmt)? };
        handle.params.insert(key, value);
        handle.executed = false;
        handle.current = None;
        handle.rows.clear();
        handle.cursor = 0;
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_stmt_bind_null(stmt: *mut ndb_stmt_t, name: *const c_char) -> c_int {
    stmt_bind_value(stmt, name, Value::Null)
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_stmt_bind_bool(
    stmt: *mut ndb_stmt_t,
    name: *const c_char,
    value: c_int,
) -> c_int {
    stmt_bind_value(stmt, name, Value::Bool(value != 0))
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_stmt_bind_int64(
    stmt: *mut ndb_stmt_t,
    name: *const c_char,
    value: i64,
) -> c_int {
    stmt_bind_value(stmt, name, Value::Int(value))
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_stmt_bind_double(
    stmt: *mut ndb_stmt_t,
    name: *const c_char,
    value: f64,
) -> c_int {
    stmt_bind_value(stmt, name, Value::Float(value))
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_stmt_bind_string(
    stmt: *mut ndb_stmt_t,
    name: *const c_char,
    value: *const c_char,
) -> c_int {
    let value = match cstr_to_string(value, "value") {
        Ok(v) => v,
        Err(e) => return err_status(e),
    };
    stmt_bind_value(stmt, name, Value::String(value))
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_stmt_bind_list(
    stmt: *mut ndb_stmt_t,
    name: *const c_char,
    value_json: *const c_char,
) -> c_int {
    let value = match cstr_to_json_value(value_json, "value_json") {
        Ok(v) => v,
        Err(e) => return err_status(e),
    };
    if !value.is_array() {
        return err_status(ApiError::invalid("list bind requires JSON array"));
    }
    let parsed = match json_to_query_value(&value) {
        Ok(v) => v,
        Err(e) => return err_status(e),
    };
    stmt_bind_value(stmt, name, parsed)
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_stmt_bind_map(
    stmt: *mut ndb_stmt_t,
    name: *const c_char,
    value_json: *const c_char,
) -> c_int {
    let value = match cstr_to_json_value(value_json, "value_json") {
        Ok(v) => v,
        Err(e) => return err_status(e),
    };
    if !value.is_object() {
        return err_status(ApiError::invalid("map bind requires JSON object"));
    }
    let parsed = match json_to_query_value(&value) {
        Ok(v) => v,
        Err(e) => return err_status(e),
    };
    stmt_bind_value(stmt, name, parsed)
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_stmt_step(stmt: *mut ndb_stmt_t, out_state: *mut c_int) -> c_int {
    if out_state.is_null() {
        return err_status(ApiError::null_pointer("out_state"));
    }
    let result = (|| -> ApiResult<()> {
        let stmt = unsafe { stmt_handle_mut(stmt)? };
        stmt_execute_if_needed(stmt)?;
        match stmt.mode {
            StmtMode::Read => {
                if stmt.cursor < stmt.rows.len() {
                    stmt.current = stmt.rows.get(stmt.cursor).cloned();
                    stmt.cursor += 1;
                    unsafe {
                        // SAFETY: out_state validated above.
                        *out_state = NDB_STEP_ROW;
                    }
                } else {
                    stmt.current = None;
                    unsafe {
                        // SAFETY: out_state validated above.
                        *out_state = NDB_STEP_DONE;
                    }
                }
            }
            StmtMode::Write => unsafe {
                // SAFETY: out_state validated above.
                *out_state = NDB_STEP_DONE;
            },
        }
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => {
            unsafe {
                // SAFETY: out_state validated above.
                *out_state = NDB_STEP_ERROR;
            }
            err_status(e)
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_stmt_column_count(stmt: *mut ndb_stmt_t) -> usize {
    let result = (|| -> ApiResult<usize> {
        let stmt = unsafe { stmt_handle_mut(stmt)? };
        let row = stmt
            .current
            .as_ref()
            .ok_or_else(|| ApiError::execution("no current row"))?;
        Ok(row.columns().len())
    })();
    match result {
        Ok(v) => {
            clear_last_error();
            v
        }
        Err(e) => {
            set_last_error(&e);
            0
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_stmt_column_type(stmt: *mut ndb_stmt_t, col: usize) -> c_int {
    let result = (|| -> ApiResult<c_int> {
        let stmt = unsafe { stmt_handle_mut(stmt)? };
        let value = stmt_current_value(stmt, col)?;
        Ok(value_kind(value))
    })();
    match result {
        Ok(v) => {
            clear_last_error();
            v
        }
        Err(e) => {
            set_last_error(&e);
            NDB_COL_OTHER
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_stmt_column_int64(
    stmt: *mut ndb_stmt_t,
    col: usize,
    out_value: *mut i64,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        if out_value.is_null() {
            return Err(ApiError::null_pointer("out_value"));
        }
        let stmt = unsafe { stmt_handle_mut(stmt)? };
        let value = stmt_current_value(stmt, col)?;
        let parsed = match value {
            Value::Int(v) => *v,
            Value::DateTime(v) => *v,
            Value::NodeId(v) => i64::from(*v),
            Value::ExternalId(v) => i64::try_from(*v)
                .map_err(|_| ApiError::execution("external id does not fit i64"))?,
            _ => return Err(ApiError::execution("column type is not int64-compatible")),
        };
        unsafe {
            // SAFETY: output pointer validated above.
            *out_value = parsed;
        }
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_stmt_column_double(
    stmt: *mut ndb_stmt_t,
    col: usize,
    out_value: *mut f64,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        if out_value.is_null() {
            return Err(ApiError::null_pointer("out_value"));
        }
        let stmt = unsafe { stmt_handle_mut(stmt)? };
        let value = stmt_current_value(stmt, col)?;
        let parsed = match value {
            Value::Float(v) => *v,
            Value::Int(v) => *v as f64,
            _ => return Err(ApiError::execution("column type is not double-compatible")),
        };
        unsafe {
            // SAFETY: output pointer validated above.
            *out_value = parsed;
        }
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_stmt_column_bool(
    stmt: *mut ndb_stmt_t,
    col: usize,
    out_value: *mut c_int,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        if out_value.is_null() {
            return Err(ApiError::null_pointer("out_value"));
        }
        let stmt = unsafe { stmt_handle_mut(stmt)? };
        let value = stmt_current_value(stmt, col)?;
        let parsed = match value {
            Value::Bool(v) => *v,
            _ => return Err(ApiError::execution("column type is not bool")),
        };
        unsafe {
            // SAFETY: output pointer validated above.
            *out_value = if parsed { 1 } else { 0 };
        }
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_stmt_column_string(
    stmt: *mut ndb_stmt_t,
    col: usize,
    out_value: *mut *mut c_char,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        let stmt = unsafe { stmt_handle_mut(stmt)? };
        let value = stmt_current_value(stmt, col)?;
        let text = match value {
            Value::String(v) => v.clone(),
            _ => return Err(ApiError::execution("column type is not string")),
        };
        write_out_c_string(out_value, &text)
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_stmt_column_json(
    stmt: *mut ndb_stmt_t,
    col: usize,
    out_value: *mut *mut c_char,
) -> c_int {
    let result = (|| -> ApiResult<()> {
        let stmt = unsafe { stmt_handle_mut(stmt)? };
        let value = stmt_current_value(stmt, col)?.clone();
        let text = serde_json::to_string(&value_to_json(value))
            .map_err(|e| ApiError::internal(format!("json encode failed: {e}")))?;
        write_out_c_string(out_value, &text)
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_stmt_reset(stmt: *mut ndb_stmt_t) -> c_int {
    let result = (|| -> ApiResult<()> {
        let stmt = unsafe { stmt_handle_mut(stmt)? };
        stmt.executed = false;
        stmt.rows.clear();
        stmt.cursor = 0;
        stmt.current = None;
        stmt.write_count = 0;
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_stmt_finalize(stmt: *mut ndb_stmt_t) -> c_int {
    let result = (|| -> ApiResult<()> {
        if stmt.is_null() {
            return Ok(());
        }
        unsafe {
            // SAFETY: pointer was allocated by this crate and ownership transferred to caller.
            drop(Box::from_raw(stmt.cast::<StmtHandle>()));
        }
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn ndb_stmt_write_count(stmt: *mut ndb_stmt_t, out_count: *mut u32) -> c_int {
    let result = (|| -> ApiResult<()> {
        if out_count.is_null() {
            return Err(ApiError::null_pointer("out_count"));
        }
        let stmt = unsafe { stmt_handle_mut(stmt)? };
        unsafe {
            // SAFETY: output pointer validated above.
            *out_count = stmt.write_count;
        }
        Ok(())
    })();
    match result {
        Ok(()) => ok_status(),
        Err(e) => err_status(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_write_query_detects_create() {
        assert!(write_query_contains_write("CREATE (:User)").expect("parse"));
        assert!(!write_query_contains_write("MATCH (n) RETURN n").expect("parse"));
    }

    #[test]
    fn classify_expected_prefix_as_syntax_error() {
        let err = ApiError::from_query_message("Expected ')'");
        assert_eq!(err.code, NDB_ERR_SYNTAX);
        assert_eq!(err.category, NDB_ERRCAT_SYNTAX);
    }
}
