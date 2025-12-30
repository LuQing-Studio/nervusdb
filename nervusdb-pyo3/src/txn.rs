use crate::db::Db;
use crate::types::py_to_value;
use nervusdb_v2::WriteTxn as RustWriteTxn;
use pyo3::prelude::*;
use std::mem::transmute;

/// Write transaction for NervusDB.
///
/// All modifications are buffered until commit() is called.
#[pyclass(unsendable)]
pub struct WriteTxn {
    inner: Option<RustWriteTxn<'static>>,
    db: Py<Db>,
}

impl WriteTxn {
    pub fn new(txn: RustWriteTxn<'_>, db: Py<Db>) -> Self {
        // SAFETY: We hold a strong reference to `db` in the struct, ensuring the owner
        // stays alive as long as this transaction exists. The 'static lifetime is
        // a lie to the compiler, but it's safe because we enforce the lifetime relationship manually.
        let extended_txn = unsafe { transmute::<RustWriteTxn<'_>, RustWriteTxn<'static>>(txn) };
        Self {
            inner: Some(extended_txn),
            db,
        }
    }
}

#[pymethods]
impl WriteTxn {
    /// Execute a Cypher write query.
    fn query(&mut self, py: Python<'_>, query: &str) -> PyResult<()> {
        let txn = self.inner.as_mut().ok_or_else(|| {
            pyo3::exceptions::PyRuntimeError::new_err("Transaction already finished")
        })?;

        // Get snapshot from the parent Db
        let db_ref = self.db.borrow(py);
        let inner_db = db_ref
            .inner
            .as_ref()
            .ok_or_else(|| pyo3::exceptions::PyRuntimeError::new_err("Database is closed"))?;
        let snapshot = inner_db.snapshot();

        let prepared = nervusdb_v2_query::prepare(query)
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        prepared
            .execute_write(&snapshot, txn, &nervusdb_v2_query::Params::new())
            .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;

        Ok(())
    }

    /// Commit the transaction.
    fn commit(&mut self) -> PyResult<()> {
        if let Some(txn) = self.inner.take() {
            txn.commit()
                .map_err(|e| pyo3::exceptions::PyRuntimeError::new_err(e.to_string()))?;
        }
        Ok(())
    }

    /// Rollback the transaction.
    fn rollback(&mut self) {
        self.inner = None;
    }
}
