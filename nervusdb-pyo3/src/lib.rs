//! Python bindings for NervusDB v2
#![allow(clippy::useless_conversion)]
//!
//! ```python
//! import nervusdb
//!
//! db = nervusdb.Db("my_graph.ndb")
//! rows = db.query("MATCH (n) RETURN n")
//! for row in db.query_stream("MATCH (n) RETURN n"):
//!     print(row)
//! ```

use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};
use std::collections::BTreeMap;

use nervusdb::{
    backup as rust_backup, bulkload as rust_bulkload, vacuum as rust_vacuum, BulkEdge, BulkNode,
    PropertyValue,
};

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

fn py_to_property_value(obj: &Bound<'_, PyAny>) -> PyResult<PropertyValue> {
    if obj.is_none() {
        return Ok(PropertyValue::Null);
    }

    if let Ok(b) = obj.extract::<bool>() {
        return Ok(PropertyValue::Bool(b));
    }

    if let Ok(i) = obj.extract::<i64>() {
        return Ok(PropertyValue::Int(i));
    }

    if let Ok(f) = obj.extract::<f64>() {
        return Ok(PropertyValue::Float(f));
    }

    if let Ok(s) = obj.extract::<String>() {
        return Ok(PropertyValue::String(s));
    }

    if let Ok(list) = obj.downcast::<PyList>() {
        let mut out = Vec::with_capacity(list.len());
        for item in list.iter() {
            out.push(py_to_property_value(&item)?);
        }
        return Ok(PropertyValue::List(out));
    }

    if let Ok(dict) = obj.downcast::<PyDict>() {
        let mut out = BTreeMap::new();
        for (k, v) in dict.iter() {
            let key = k.extract::<String>().map_err(|_| {
                pyo3::exceptions::PyTypeError::new_err("Dictionary keys must be strings")
            })?;
            out.insert(key, py_to_property_value(&v)?);
        }
        return Ok(PropertyValue::Map(out));
    }

    Err(pyo3::exceptions::PyTypeError::new_err(
        "Unsupported type for PropertyValue",
    ))
}

fn py_to_property_map(obj: Option<Bound<'_, PyAny>>) -> PyResult<BTreeMap<String, PropertyValue>> {
    let Some(obj) = obj else {
        return Ok(BTreeMap::new());
    };
    if obj.is_none() {
        return Ok(BTreeMap::new());
    }
    let dict = obj.downcast::<PyDict>().map_err(|_| {
        pyo3::exceptions::PyTypeError::new_err("properties must be a dict[str, Any]")
    })?;
    let mut out = BTreeMap::new();
    for (k, v) in dict.iter() {
        let key = k.extract::<String>().map_err(|_| {
            pyo3::exceptions::PyTypeError::new_err("properties keys must be strings")
        })?;
        out.insert(key, py_to_property_value(&v)?);
    }
    Ok(out)
}

/// Open a NervusDB database.
/// This is a convenience function that aliases Db constructor.
#[pyfunction]
#[pyo3(signature = (path))]
fn open(path: &str) -> PyResult<Db> {
    Db::new(path)
}

/// Vacuum database in-place.
#[pyfunction]
#[pyo3(signature = (path))]
fn vacuum(py: Python<'_>, path: &str) -> PyResult<PyObject> {
    let report = rust_vacuum(path).map_err(classify_nervus_error)?;
    let out = PyDict::new_bound(py);
    out.set_item("ndb_path", report.ndb_path.to_string_lossy().to_string())?;
    out.set_item(
        "backup_path",
        report.backup_path.to_string_lossy().to_string(),
    )?;
    out.set_item("old_next_page_id", report.old_next_page_id)?;
    out.set_item("new_next_page_id", report.new_next_page_id)?;
    out.set_item("copied_data_pages", report.copied_data_pages)?;
    out.set_item("old_file_pages", report.old_file_pages)?;
    out.set_item("new_file_pages", report.new_file_pages)?;
    Ok(out.into())
}

/// Create online backup snapshot.
#[pyfunction]
#[pyo3(signature = (path, backup_dir))]
fn backup(py: Python<'_>, path: &str, backup_dir: &str) -> PyResult<PyObject> {
    let info = rust_backup(path, backup_dir).map_err(classify_nervus_error)?;
    let out = PyDict::new_bound(py);
    out.set_item("id", info.id.to_string())?;
    out.set_item("created_at", info.created_at.to_rfc3339())?;
    out.set_item("size_bytes", info.size_bytes)?;
    out.set_item("file_count", info.file_count)?;
    out.set_item("nervusdb_version", info.nervusdb_version)?;
    out.set_item("checkpoint_txid", info.checkpoint_txid)?;
    out.set_item("checkpoint_epoch", info.checkpoint_epoch)?;
    Ok(out.into())
}

/// Offline bulk load to a new database.
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
        let properties = py_to_property_map(dict.get_item("properties")?)?;
        parsed_nodes.push(BulkNode {
            external_id,
            label,
            properties,
        });
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
        let properties = py_to_property_map(dict.get_item("properties")?)?;
        parsed_edges.push(BulkEdge {
            src_external_id,
            rel_type,
            dst_external_id,
            properties,
        });
    }

    rust_bulkload(path, parsed_nodes, parsed_edges).map_err(classify_nervus_error)
}

/// Initialize the Python module.
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
    }

    #[test]
    fn classify_maps_execution_errors_by_default() {
        assert_eq!(
            classify_error_text("not implemented: expression"),
            ErrorClass::Execution
        );
        assert_eq!(
            classify_error_text(
                "execution error: ResourceLimitExceeded(kind=Timeout, limit=1, observed=10, stage=ReturnOne)"
            ),
            ErrorClass::Execution
        );
    }

    #[test]
    fn classify_prioritizes_compatibility_when_multiple_keywords_exist() {
        assert_eq!(
            classify_error_text("compatibility failure after parse step"),
            ErrorClass::Compatibility
        );
    }
}
