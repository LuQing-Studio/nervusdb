use crate::classify_nervus_error;
use crate::db::Db;
use crate::types::py_to_json;
use nervusdb_capi as capi;
use pyo3::prelude::*;
use std::ffi::CString;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[pyclass(unsendable)]
pub struct WriteTxn {
    raw: Option<*mut capi::ndb_txn_t>,
    _db: Py<Db>,
    active_write_txns: Arc<AtomicUsize>,
    active: bool,
}

impl WriteTxn {
    pub fn new(txn: *mut capi::ndb_txn_t, db: Py<Db>, active_write_txns: Arc<AtomicUsize>) -> Self {
        Self {
            raw: Some(txn),
            _db: db,
            active_write_txns,
            active: true,
        }
    }

    fn with_txn_ptr<T>(
        &mut self,
        f: impl FnOnce(*mut capi::ndb_txn_t) -> PyResult<T>,
    ) -> PyResult<T> {
        let raw = self
            .raw
            .ok_or_else(|| classify_nervus_error("Transaction already finished"))?;
        f(raw)
    }

    fn finish(&mut self) {
        if !self.active {
            return;
        }
        self.active = false;
        self.raw = None;
        self.active_write_txns.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Drop for WriteTxn {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        if let Some(raw) = self.raw.take() {
            let _ = capi::ndb_txn_rollback(raw);
        }
        self.finish();
    }
}

#[pymethods]
impl WriteTxn {
    fn query(&mut self, _py: Python<'_>, query: &str) -> PyResult<()> {
        let query_c = CString::new(query)
            .map_err(|_| classify_nervus_error("query contains interior NUL"))?;
        self.with_txn_ptr(|raw| {
            let rc = capi::ndb_txn_query(raw, query_c.as_ptr(), ptr::null());
            if rc == capi::NDB_OK {
                Ok(())
            } else {
                Err(crate::capi_last_error())
            }
        })
    }

    fn commit(&mut self) -> PyResult<()> {
        let raw = self
            .raw
            .take()
            .ok_or_else(|| classify_nervus_error("Transaction already finished"))?;
        let rc = capi::ndb_txn_commit(raw);
        if rc != capi::NDB_OK {
            return Err(crate::capi_last_error());
        }
        self.finish();
        Ok(())
    }

    fn set_vector(&mut self, node_id: u32, vector: Vec<f32>) -> PyResult<()> {
        self.with_txn_ptr(|raw| {
            let rc = capi::ndb_txn_set_vector(raw, node_id, vector.as_ptr(), vector.len());
            if rc == capi::NDB_OK {
                Ok(())
            } else {
                Err(crate::capi_last_error())
            }
        })
    }

    fn create_node(&mut self, external_id: u64, label_id: u32) -> PyResult<u32> {
        let mut node_id: u32 = 0;
        self.with_txn_ptr(|raw| {
            let rc = capi::ndb_txn_create_node(raw, external_id, label_id, &mut node_id);
            if rc == capi::NDB_OK {
                Ok(())
            } else {
                Err(crate::capi_last_error())
            }
        })?;
        Ok(node_id)
    }

    fn get_or_create_label(&mut self, name: &str) -> PyResult<u32> {
        let name_c = CString::new(name)
            .map_err(|_| classify_nervus_error("label name contains interior NUL"))?;
        let mut label: u32 = 0;
        self.with_txn_ptr(|raw| {
            let rc = capi::ndb_txn_get_or_create_label(raw, name_c.as_ptr(), &mut label);
            if rc == capi::NDB_OK {
                Ok(())
            } else {
                Err(crate::capi_last_error())
            }
        })?;
        Ok(label)
    }

    fn get_or_create_rel_type(&mut self, name: &str) -> PyResult<u32> {
        let name_c = CString::new(name)
            .map_err(|_| classify_nervus_error("rel type name contains interior NUL"))?;
        let mut rel: u32 = 0;
        self.with_txn_ptr(|raw| {
            let rc = capi::ndb_txn_get_or_create_rel_type(raw, name_c.as_ptr(), &mut rel);
            if rc == capi::NDB_OK {
                Ok(())
            } else {
                Err(crate::capi_last_error())
            }
        })?;
        Ok(rel)
    }

    fn create_edge(&mut self, src: u32, rel: u32, dst: u32) -> PyResult<()> {
        self.with_txn_ptr(|raw| {
            let rc = capi::ndb_txn_create_edge(raw, src, rel, dst);
            if rc == capi::NDB_OK {
                Ok(())
            } else {
                Err(crate::capi_last_error())
            }
        })
    }

    fn tombstone_node(&mut self, node: u32) -> PyResult<()> {
        self.with_txn_ptr(|raw| {
            let rc = capi::ndb_txn_tombstone_node(raw, node);
            if rc == capi::NDB_OK {
                Ok(())
            } else {
                Err(crate::capi_last_error())
            }
        })
    }

    fn tombstone_edge(&mut self, src: u32, rel: u32, dst: u32) -> PyResult<()> {
        self.with_txn_ptr(|raw| {
            let rc = capi::ndb_txn_tombstone_edge(raw, src, rel, dst);
            if rc == capi::NDB_OK {
                Ok(())
            } else {
                Err(crate::capi_last_error())
            }
        })
    }

    fn set_node_property(
        &mut self,
        py: Python<'_>,
        node: u32,
        key: String,
        value: Py<PyAny>,
    ) -> PyResult<()> {
        let key_c = CString::new(key)
            .map_err(|_| classify_nervus_error("property key contains interior NUL"))?;
        let value_json = serde_json::to_string(&py_to_json(value.bind(py))?)
            .map_err(|e| classify_nervus_error(e.to_string()))?;
        let value_c = CString::new(value_json)
            .map_err(|_| classify_nervus_error("property value contains interior NUL"))?;

        self.with_txn_ptr(|raw| {
            let rc = capi::ndb_txn_set_node_property(raw, node, key_c.as_ptr(), value_c.as_ptr());
            if rc == capi::NDB_OK {
                Ok(())
            } else {
                Err(crate::capi_last_error())
            }
        })
    }

    fn set_edge_property(
        &mut self,
        py: Python<'_>,
        src: u32,
        rel: u32,
        dst: u32,
        key: String,
        value: Py<PyAny>,
    ) -> PyResult<()> {
        let key_c = CString::new(key)
            .map_err(|_| classify_nervus_error("property key contains interior NUL"))?;
        let value_json = serde_json::to_string(&py_to_json(value.bind(py))?)
            .map_err(|e| classify_nervus_error(e.to_string()))?;
        let value_c = CString::new(value_json)
            .map_err(|_| classify_nervus_error("property value contains interior NUL"))?;

        self.with_txn_ptr(|raw| {
            let rc = capi::ndb_txn_set_edge_property(
                raw,
                src,
                rel,
                dst,
                key_c.as_ptr(),
                value_c.as_ptr(),
            );
            if rc == capi::NDB_OK {
                Ok(())
            } else {
                Err(crate::capi_last_error())
            }
        })
    }

    fn remove_node_property(&mut self, node: u32, key: &str) -> PyResult<()> {
        let key_c = CString::new(key)
            .map_err(|_| classify_nervus_error("property key contains interior NUL"))?;
        self.with_txn_ptr(|raw| {
            let rc = capi::ndb_txn_remove_node_property(raw, node, key_c.as_ptr());
            if rc == capi::NDB_OK {
                Ok(())
            } else {
                Err(crate::capi_last_error())
            }
        })
    }

    fn remove_edge_property(&mut self, src: u32, rel: u32, dst: u32, key: &str) -> PyResult<()> {
        let key_c = CString::new(key)
            .map_err(|_| classify_nervus_error("property key contains interior NUL"))?;
        self.with_txn_ptr(|raw| {
            let rc = capi::ndb_txn_remove_edge_property(raw, src, rel, dst, key_c.as_ptr());
            if rc == capi::NDB_OK {
                Ok(())
            } else {
                Err(crate::capi_last_error())
            }
        })
    }

    pub(crate) fn rollback(&mut self) -> PyResult<()> {
        if !self.active {
            return Ok(());
        }
        if let Some(raw) = self.raw.take() {
            let rc = capi::ndb_txn_rollback(raw);
            if rc != capi::NDB_OK {
                return Err(crate::capi_last_error());
            }
        }
        self.finish();
        Ok(())
    }
}
