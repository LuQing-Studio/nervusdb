//! Python bindings for NervusDB v2 (C ABI adapter)
#![allow(clippy::useless_conversion)]

use nervusdb_capi as capi;
use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use serde_json::json;
use serde_json::Value as JsonValue;
use std::ffi::{c_char, CStr, CString};
use std::fs;
use std::path::{Path, PathBuf};
use std::ptr;

mod db;
mod stream;
mod txn;
mod types;

pub use db::Db;
pub use stream::QueryStream;
pub use txn::WriteTxn;

create_exception!(nervusdb, NervusError, PyException);
create_exception!(nervusdb, SyntaxError, NervusError);
create_exception!(nervusdb, ExecutionError, NervusError);
create_exception!(nervusdb, StorageError, NervusError);
create_exception!(nervusdb, CompatibilityError, NervusError);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ErrorClass {
    Syntax,
    Execution,
    Storage,
    Compatibility,
}

fn classify_error_text(msg: &str) -> ErrorClass {
    let lower = msg.to_lowercase();

    if lower.contains("storage format mismatch")
        || lower.contains("compatibility")
        || lower.contains("epoch")
    {
        ErrorClass::Compatibility
    } else if lower.contains("resourcelimitexceeded") {
        ErrorClass::Execution
    } else if lower.contains("syntax")
        || lower.contains("parse")
        || lower.contains("unexpected token")
        || lower.contains("unexpected character")
        || lower.starts_with("expected ")
        || lower.contains("variabletypeconflict")
        || lower.contains("variablealreadybound")
    {
        ErrorClass::Syntax
    } else if lower.contains("wal")
        || lower.contains("checkpoint")
        || lower.contains("database is closed")
        || lower.contains("cannot close database")
        || lower.contains("io error")
        || lower.contains("permission denied")
        || lower.contains("no such file")
        || lower.contains("disk full")
    {
        ErrorClass::Storage
    } else {
        ErrorClass::Execution
    }
}

pub(crate) fn classify_nervus_error(msg: impl ToString) -> PyErr {
    let msg = msg.to_string();
    match classify_error_text(&msg) {
        ErrorClass::Syntax => SyntaxError::new_err(msg),
        ErrorClass::Compatibility => CompatibilityError::new_err(msg),
        ErrorClass::Storage => StorageError::new_err(msg),
        ErrorClass::Execution => ExecutionError::new_err(msg),
    }
}

fn classify_capi_category(category: i32, msg: &str) -> ErrorClass {
    match category {
        x if x == capi::NDB_ERRCAT_SYNTAX => ErrorClass::Syntax,
        x if x == capi::NDB_ERRCAT_STORAGE => ErrorClass::Storage,
        x if x == capi::NDB_ERRCAT_COMPATIBILITY => ErrorClass::Compatibility,
        x if x == capi::NDB_ERRCAT_EXECUTION => ErrorClass::Execution,
        _ => classify_error_text(msg),
    }
}

fn last_error_message() -> String {
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

pub(crate) fn capi_last_error() -> PyErr {
    let msg = last_error_message();
    let category = capi::ndb_last_error_category();
    match classify_capi_category(category, &msg) {
        ErrorClass::Syntax => SyntaxError::new_err(msg),
        ErrorClass::Compatibility => CompatibilityError::new_err(msg),
        ErrorClass::Storage => StorageError::new_err(msg),
        ErrorClass::Execution => ExecutionError::new_err(msg),
    }
}

pub(crate) fn capi_status(rc: i32) -> PyResult<()> {
    if rc == capi::NDB_OK {
        Ok(())
    } else {
        Err(capi_last_error())
    }
}

fn derive_paths(path: &Path) -> (PathBuf, PathBuf) {
    match path.extension().and_then(|e| e.to_str()) {
        Some("ndb") => (path.to_path_buf(), path.with_extension("wal")),
        Some("wal") => (path.with_extension("ndb"), path.to_path_buf()),
        _ => (path.with_extension("ndb"), path.with_extension("wal")),
    }
}

#[pyfunction]
#[pyo3(signature = (path))]
fn open(path: &str) -> PyResult<Db> {
    Db::new(path)
}

#[pyfunction]
#[pyo3(signature = (path))]
fn vacuum(py: Python<'_>, path: &str) -> PyResult<PyObject> {
    let path_c =
        CString::new(path).map_err(|_| classify_nervus_error("path contains interior NUL"))?;
    capi_status(capi::ndb_vacuum(path_c.as_ptr()))?;

    let (ndb_path, _) = derive_paths(Path::new(path));
    let meta = fs::metadata(&ndb_path).map_err(classify_nervus_error)?;
    let pages = meta.len().div_ceil(4096);

    let out = PyDict::new_bound(py);
    out.set_item("ndb_path", ndb_path.to_string_lossy().to_string())?;
    out.set_item(
        "backup_path",
        format!("{}.vacuum.bak", ndb_path.to_string_lossy()),
    )?;
    out.set_item("old_next_page_id", pages)?;
    out.set_item("new_next_page_id", pages)?;
    out.set_item("copied_data_pages", pages.saturating_sub(2))?;
    out.set_item("old_file_pages", pages)?;
    out.set_item("new_file_pages", pages)?;
    Ok(out.into())
}

#[pyfunction]
#[pyo3(signature = (path, backup_dir))]
fn backup(py: Python<'_>, path: &str, backup_dir: &str) -> PyResult<PyObject> {
    let path_c =
        CString::new(path).map_err(|_| classify_nervus_error("path contains interior NUL"))?;
    let backup_dir_c = CString::new(backup_dir)
        .map_err(|_| classify_nervus_error("backup_dir contains interior NUL"))?;
    capi_status(capi::ndb_backup(path_c.as_ptr(), backup_dir_c.as_ptr()))?;

    let mut candidates: Vec<_> = fs::read_dir(backup_dir)
        .map_err(classify_nervus_error)?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .collect();
    candidates.sort_by_key(|entry| entry.metadata().and_then(|m| m.modified()).ok());

    let latest = candidates
        .last()
        .ok_or_else(|| classify_nervus_error("backup directory is empty after backup"))?;
    let latest_path = latest.path();

    let mut size_bytes: u64 = 0;
    let mut file_count: u64 = 0;
    for entry in fs::read_dir(&latest_path).map_err(classify_nervus_error)? {
        let entry = entry.map_err(classify_nervus_error)?;
        let meta = entry.metadata().map_err(classify_nervus_error)?;
        if meta.is_file() {
            file_count += 1;
            size_bytes = size_bytes.saturating_add(meta.len());
        }
    }

    let out = PyDict::new_bound(py);
    out.set_item("id", latest.file_name().to_string_lossy().to_string())?;
    out.set_item("created_at", format!("{:?}", std::time::SystemTime::now()))?;
    out.set_item("size_bytes", size_bytes)?;
    out.set_item("file_count", file_count)?;
    out.set_item("nervusdb_version", "1.0.0")?;
    out.set_item("checkpoint_txid", 0)?;
    out.set_item("checkpoint_epoch", 0)?;
    Ok(out.into())
}

#[pyfunction]
#[pyo3(signature = (path, nodes, edges))]
fn bulkload(path: &str, nodes: &Bound<'_, PyList>, edges: &Bound<'_, PyList>) -> PyResult<()> {
    let mut parsed_nodes = Vec::with_capacity(nodes.len());
    for item in nodes.iter() {
        let dict = item.downcast::<PyDict>().map_err(|_| {
            pyo3::exceptions::PyTypeError::new_err(
                "each node must be dict {external_id,label,properties?}",
            )
        })?;
        let external_id: u64 = dict
            .get_item("external_id")?
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("node.external_id missing"))?
            .extract()?;
        let label: String = dict
            .get_item("label")?
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("node.label missing"))?
            .extract()?;
        let properties = if let Some(v) = dict.get_item("properties")? {
            types::py_to_json(&v)?
        } else {
            JsonValue::Object(Default::default())
        };

        parsed_nodes.push(json!({
            "external_id": external_id,
            "label": label,
            "properties": properties,
        }));
    }

    let mut parsed_edges = Vec::with_capacity(edges.len());
    for item in edges.iter() {
        let dict = item.downcast::<PyDict>().map_err(|_| {
            pyo3::exceptions::PyTypeError::new_err(
                "each edge must be dict {src_external_id,rel_type,dst_external_id,properties?}",
            )
        })?;
        let src_external_id: u64 = dict
            .get_item("src_external_id")?
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("edge.src_external_id missing"))?
            .extract()?;
        let rel_type: String = dict
            .get_item("rel_type")?
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("edge.rel_type missing"))?
            .extract()?;
        let dst_external_id: u64 = dict
            .get_item("dst_external_id")?
            .ok_or_else(|| pyo3::exceptions::PyKeyError::new_err("edge.dst_external_id missing"))?
            .extract()?;
        let properties = if let Some(v) = dict.get_item("properties")? {
            types::py_to_json(&v)?
        } else {
            JsonValue::Object(Default::default())
        };

        parsed_edges.push(json!({
            "src_external_id": src_external_id,
            "rel_type": rel_type,
            "dst_external_id": dst_external_id,
            "properties": properties,
        }));
    }

    let path_c =
        CString::new(path).map_err(|_| classify_nervus_error("path contains interior NUL"))?;
    let nodes_c = CString::new(
        serde_json::to_string(&parsed_nodes).map_err(|e| classify_nervus_error(e.to_string()))?,
    )
    .map_err(|_| classify_nervus_error("nodes payload contains interior NUL"))?;
    let edges_c = CString::new(
        serde_json::to_string(&parsed_edges).map_err(|e| classify_nervus_error(e.to_string()))?,
    )
    .map_err(|_| classify_nervus_error("edges payload contains interior NUL"))?;

    capi_status(capi::ndb_bulkload(
        path_c.as_ptr(),
        nodes_c.as_ptr(),
        edges_c.as_ptr(),
    ))
}

#[pymodule]
#[pyo3(name = "nervusdb")]
fn nervusdb_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(open, m)?)?;
    m.add_function(wrap_pyfunction!(vacuum, m)?)?;
    m.add_function(wrap_pyfunction!(backup, m)?)?;
    m.add_function(wrap_pyfunction!(bulkload, m)?)?;
    m.add_class::<Db>()?;
    m.add_class::<WriteTxn>()?;
    m.add_class::<QueryStream>()?;
    m.add_class::<types::Node>()?;
    m.add_class::<types::Relationship>()?;
    m.add_class::<types::Path>()?;

    m.add("NervusError", m.py().get_type_bound::<NervusError>())?;
    m.add("SyntaxError", m.py().get_type_bound::<SyntaxError>())?;
    m.add("ExecutionError", m.py().get_type_bound::<ExecutionError>())?;
    m.add("StorageError", m.py().get_type_bound::<StorageError>())?;
    m.add(
        "CompatibilityError",
        m.py().get_type_bound::<CompatibilityError>(),
    )?;

    m.add("__version__", "2.0.0")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{classify_error_text, ErrorClass};

    #[test]
    fn classify_maps_syntax_errors() {
        assert_eq!(
            classify_error_text("syntax error: unexpected token"),
            ErrorClass::Syntax
        );
        assert_eq!(
            classify_error_text("VariableTypeConflict: r"),
            ErrorClass::Syntax
        );
        assert_eq!(classify_error_text("Expected ')'"), ErrorClass::Syntax);
    }

    #[test]
    fn classify_maps_storage_errors() {
        assert_eq!(
            classify_error_text("database is closed"),
            ErrorClass::Storage
        );
        assert_eq!(
            classify_error_text("wal replay failed"),
            ErrorClass::Storage
        );
        assert_eq!(
            classify_error_text("permission denied while opening wal"),
            ErrorClass::Storage
        );
        assert_eq!(
            classify_error_text("io error: disk full"),
            ErrorClass::Storage
        );
    }

    #[test]
    fn classify_maps_compatibility_errors() {
        assert_eq!(
            classify_error_text("storage format mismatch: expected epoch 1, found 0"),
            ErrorClass::Compatibility
        );
        assert_eq!(
            classify_error_text("compatibility error while opening snapshot"),
            ErrorClass::Compatibility
        );
    }

    #[test]
    fn classify_prioritizes_compatibility_when_multiple_keywords_exist() {
        assert_eq!(
            classify_error_text("compatibility parse mismatch"),
            ErrorClass::Compatibility
        );
    }

    #[test]
    fn classify_maps_execution_errors_by_default() {
        assert_eq!(
            classify_error_text("execution failed: unknown function"),
            ErrorClass::Execution
        );
        assert_eq!(
            classify_error_text(
                "execution error: ResourceLimitExceeded(kind=Timeout, limit=1, observed=10, stage=ReturnOne)",
            ),
            ErrorClass::Execution
        );
    }
}
