use crate::classify_nervus_error;
use crate::db::Db;
use nervusdb::PropertyValue;
use nervusdb::WriteTxn as RustWriteTxn;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};
use std::mem::transmute;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

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
        let mut out = std::collections::BTreeMap::new();
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

/// Write transaction for NervusDB.
///
/// All modifications are buffered until commit() is called.
#[pyclass(unsendable)]
pub struct WriteTxn {
    inner: Option<RustWriteTxn<'static>>,
    db: Py<Db>,
    active_write_txns: Arc<AtomicUsize>,
    active: bool,
}

impl WriteTxn {
    pub fn new(txn: RustWriteTxn<'_>, db: Py<Db>, active_write_txns: Arc<AtomicUsize>) -> Self {
        // SAFETY: We hold a strong reference to `db` in the struct, ensuring the owner
        // stays alive as long as this transaction exists. The 'static lifetime is
        // a lie to the compiler, but it's safe because we enforce the lifetime relationship manually.
        let extended_txn = unsafe { transmute::<RustWriteTxn<'_>, RustWriteTxn<'static>>(txn) };
        Self {
            inner: Some(extended_txn),
            db,
            active_write_txns,
            active: true,
        }
    }

    fn finish(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        self.inner = None;
        self.active_write_txns.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Drop for WriteTxn {
    fn drop(&mut self) {
        self.finish();
    }
}

#[pymethods]
impl WriteTxn {
    /// Execute a Cypher write query.
    fn query(&mut self, py: Python<'_>, query: &str) -> PyResult<()> {
        let txn = self
            .inner
            .as_mut()
            .ok_or_else(|| classify_nervus_error("Transaction already finished"))?;

        // Get snapshot from the parent Db
        let db_ref = self.db.borrow(py);
        let inner_db = db_ref
            .inner
            .as_ref()
            .ok_or_else(|| classify_nervus_error("Database is closed"))?;
        let snapshot = inner_db.snapshot();

        let prepared = nervusdb_query::prepare(query).map_err(classify_nervus_error)?;

        prepared
            .execute_write(&snapshot, txn, &nervusdb_query::Params::new())
            .map_err(classify_nervus_error)?;

        Ok(())
    }

    /// Commit the transaction.
    fn commit(&mut self) -> PyResult<()> {
        let txn = self
            .inner
            .take()
            .ok_or_else(|| classify_nervus_error("Transaction already finished"))?;

        let res = txn.commit().map_err(classify_nervus_error);
        self.finish();
        res
    }

    /// Set vector embedding for a node.
    ///
    /// Args:
    ///     node_id: Internal Node ID (u32)
    ///     vector: List of floats
    fn set_vector(&mut self, node_id: u32, vector: Vec<f32>) -> PyResult<()> {
        let txn = self
            .inner
            .as_mut()
            .ok_or_else(|| classify_nervus_error("Transaction already finished"))?;
        txn.set_vector(node_id, vector)
            .map_err(classify_nervus_error)
    }

    /// Create node with (external_id, label_id), returns internal node id.
    fn create_node(&mut self, external_id: u64, label_id: u32) -> PyResult<u32> {
        let txn = self
            .inner
            .as_mut()
            .ok_or_else(|| classify_nervus_error("Transaction already finished"))?;
        txn.create_node(external_id, label_id)
            .map_err(classify_nervus_error)
    }

    /// Get or create label id.
    fn get_or_create_label(&mut self, name: &str) -> PyResult<u32> {
        let txn = self
            .inner
            .as_mut()
            .ok_or_else(|| classify_nervus_error("Transaction already finished"))?;
        txn.get_or_create_label(name).map_err(classify_nervus_error)
    }

    /// Get or create relationship type id.
    fn get_or_create_rel_type(&mut self, name: &str) -> PyResult<u32> {
        let txn = self
            .inner
            .as_mut()
            .ok_or_else(|| classify_nervus_error("Transaction already finished"))?;
        txn.get_or_create_rel_type(name)
            .map_err(classify_nervus_error)
    }

    /// Create directed edge.
    fn create_edge(&mut self, src: u32, rel: u32, dst: u32) -> PyResult<()> {
        let txn = self
            .inner
            .as_mut()
            .ok_or_else(|| classify_nervus_error("Transaction already finished"))?;
        txn.create_edge(src, rel, dst);
        Ok(())
    }

    /// Tombstone node.
    fn tombstone_node(&mut self, node: u32) -> PyResult<()> {
        let txn = self
            .inner
            .as_mut()
            .ok_or_else(|| classify_nervus_error("Transaction already finished"))?;
        txn.tombstone_node(node);
        Ok(())
    }

    /// Tombstone edge.
    fn tombstone_edge(&mut self, src: u32, rel: u32, dst: u32) -> PyResult<()> {
        let txn = self
            .inner
            .as_mut()
            .ok_or_else(|| classify_nervus_error("Transaction already finished"))?;
        txn.tombstone_edge(src, rel, dst);
        Ok(())
    }

    /// Set node property.
    fn set_node_property(
        &mut self,
        py: Python<'_>,
        node: u32,
        key: String,
        value: Py<PyAny>,
    ) -> PyResult<()> {
        let txn = self
            .inner
            .as_mut()
            .ok_or_else(|| classify_nervus_error("Transaction already finished"))?;
        let value = py_to_property_value(value.bind(py))?;
        txn.set_node_property(node, key, value)
            .map_err(classify_nervus_error)
    }

    /// Set edge property.
    fn set_edge_property(
        &mut self,
        py: Python<'_>,
        src: u32,
        rel: u32,
        dst: u32,
        key: String,
        value: Py<PyAny>,
    ) -> PyResult<()> {
        let txn = self
            .inner
            .as_mut()
            .ok_or_else(|| classify_nervus_error("Transaction already finished"))?;
        let value = py_to_property_value(value.bind(py))?;
        txn.set_edge_property(src, rel, dst, key, value)
            .map_err(classify_nervus_error)
    }

    /// Remove node property.
    fn remove_node_property(&mut self, node: u32, key: &str) -> PyResult<()> {
        let txn = self
            .inner
            .as_mut()
            .ok_or_else(|| classify_nervus_error("Transaction already finished"))?;
        txn.remove_node_property(node, key)
            .map_err(classify_nervus_error)
    }

    /// Remove edge property.
    fn remove_edge_property(&mut self, src: u32, rel: u32, dst: u32, key: &str) -> PyResult<()> {
        let txn = self
            .inner
            .as_mut()
            .ok_or_else(|| classify_nervus_error("Transaction already finished"))?;
        txn.remove_edge_property(src, rel, dst, key)
            .map_err(classify_nervus_error)
    }

    /// Rollback the transaction.
    pub(crate) fn rollback(&mut self) {
        self.finish();
    }
}
