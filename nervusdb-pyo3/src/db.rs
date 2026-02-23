use super::types::{json_to_py, py_to_json};
use super::WriteTxn;
use crate::{capi_status, classify_nervus_error, QueryStream};
use nervusdb_capi as capi;
use pyo3::prelude::*;
use pyo3::types::PyType;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::ffi::{c_char, CStr, CString};
use std::path::{Path, PathBuf};
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[pyclass(unsendable)]
pub struct Db {
    pub(crate) raw: Option<*mut capi::ndb_db_t>,
    ndb_path: PathBuf,
    wal_path: PathBuf,
    active_write_txns: Arc<AtomicUsize>,
}

impl Db {
    fn derive_paths(path: &Path) -> (PathBuf, PathBuf) {
        match path.extension().and_then(|e| e.to_str()) {
            Some("ndb") => (path.to_path_buf(), path.with_extension("wal")),
            Some("wal") => (path.with_extension("ndb"), path.to_path_buf()),
            _ => (path.with_extension("ndb"), path.with_extension("wal")),
        }
    }

    fn raw_ptr(&self) -> PyResult<*mut capi::ndb_db_t> {
        self.raw
            .ok_or_else(|| classify_nervus_error("database is closed"))
    }

    fn encode_params(
        params: Option<HashMap<String, Py<PyAny>>>,
        py: Python<'_>,
    ) -> PyResult<Option<CString>> {
        let Some(params) = params else {
            return Ok(None);
        };

        let mut out = serde_json::Map::new();
        for (k, v) in params {
            out.insert(k, py_to_json(v.bind(py))?);
        }
        let encoded = serde_json::to_string(&JsonValue::Object(out))
            .map_err(|e| classify_nervus_error(e.to_string()))?;
        CString::new(encoded)
            .map(Some)
            .map_err(|_| classify_nervus_error("params contains interior NUL"))
    }

    fn result_json(result_ptr: *mut capi::ndb_result_t) -> PyResult<JsonValue> {
        let mut json_ptr: *mut c_char = ptr::null_mut();
        let rc = capi::ndb_result_to_json(result_ptr, &mut json_ptr);
        capi::ndb_result_free(result_ptr);
        capi_status(rc)?;
        if json_ptr.is_null() {
            return Err(classify_nervus_error("ndb_result_to_json returned null"));
        }

        let text = unsafe {
            // SAFETY: pointer comes from C API and is valid until freed by `ndb_string_free`.
            CStr::from_ptr(json_ptr).to_string_lossy().into_owned()
        };
        capi::ndb_string_free(json_ptr);

        serde_json::from_str(&text).map_err(|e| classify_nervus_error(e.to_string()))
    }

    fn execute_query_rows(
        &self,
        query: &str,
        params: Option<HashMap<String, Py<PyAny>>>,
        py: Python<'_>,
    ) -> PyResult<Vec<HashMap<String, Py<PyAny>>>> {
        let raw = self.raw_ptr()?;
        let query_c = CString::new(query)
            .map_err(|_| classify_nervus_error("query contains interior NUL"))?;
        let params_c = Self::encode_params(params, py)?;
        let params_ptr = params_c.as_ref().map_or(ptr::null(), |s| s.as_ptr());

        let mut result_ptr: *mut capi::ndb_result_t = ptr::null_mut();
        capi_status(capi::ndb_query(
            raw,
            query_c.as_ptr(),
            params_ptr,
            &mut result_ptr,
        ))?;
        if result_ptr.is_null() {
            return Err(classify_nervus_error(
                "ndb_query returned null result handle",
            ));
        }

        let value = Self::result_json(result_ptr)?;
        let rows = value
            .as_array()
            .ok_or_else(|| classify_nervus_error("query result must be array"))?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let obj = row
                .as_object()
                .ok_or_else(|| classify_nervus_error("query row must be object"))?;
            let mut mapped = HashMap::with_capacity(obj.len());
            for (k, v) in obj {
                mapped.insert(k.clone(), json_to_py(v.clone(), py));
            }
            out.push(mapped);
        }
        Ok(out)
    }
}

#[pymethods]
impl Db {
    #[new]
    pub(crate) fn new(path: &str) -> PyResult<Self> {
        let (ndb_path, wal_path) = Self::derive_paths(Path::new(path));
        let ndb_c = CString::new(ndb_path.to_string_lossy().to_string())
            .map_err(|_| classify_nervus_error("ndb_path contains interior NUL"))?;
        let wal_c = CString::new(wal_path.to_string_lossy().to_string())
            .map_err(|_| classify_nervus_error("wal_path contains interior NUL"))?;

        let mut raw: *mut capi::ndb_db_t = ptr::null_mut();
        capi_status(capi::ndb_open_paths(
            ndb_c.as_ptr(),
            wal_c.as_ptr(),
            &mut raw,
        ))?;
        if raw.is_null() {
            return Err(classify_nervus_error(
                "ndb_open_paths returned null db handle",
            ));
        }

        Ok(Self {
            raw: Some(raw),
            ndb_path,
            wal_path,
            active_write_txns: Arc::new(AtomicUsize::new(0)),
        })
    }

    #[classmethod]
    #[pyo3(signature = (ndb_path, wal_path))]
    fn open_paths(_cls: &Bound<'_, PyType>, ndb_path: &str, wal_path: &str) -> PyResult<Self> {
        let ndb_c = CString::new(ndb_path)
            .map_err(|_| classify_nervus_error("ndb_path contains interior NUL"))?;
        let wal_c = CString::new(wal_path)
            .map_err(|_| classify_nervus_error("wal_path contains interior NUL"))?;

        let mut raw: *mut capi::ndb_db_t = ptr::null_mut();
        capi_status(capi::ndb_open_paths(
            ndb_c.as_ptr(),
            wal_c.as_ptr(),
            &mut raw,
        ))?;
        if raw.is_null() {
            return Err(classify_nervus_error(
                "ndb_open_paths returned null db handle",
            ));
        }

        Ok(Self {
            raw: Some(raw),
            ndb_path: PathBuf::from(ndb_path),
            wal_path: PathBuf::from(wal_path),
            active_write_txns: Arc::new(AtomicUsize::new(0)),
        })
    }

    #[pyo3(signature = (query, params=None))]
    fn query(
        &self,
        query: &str,
        params: Option<HashMap<String, Py<PyAny>>>,
        py: Python<'_>,
    ) -> PyResult<Vec<HashMap<String, Py<PyAny>>>> {
        self.execute_query_rows(query, params, py)
    }

    #[pyo3(signature = (query, params=None))]
    fn query_stream(
        &self,
        query: &str,
        params: Option<HashMap<String, Py<PyAny>>>,
        py: Python<'_>,
    ) -> PyResult<QueryStream> {
        let rows = self.execute_query_rows(query, params, py)?;
        Ok(QueryStream::new(rows))
    }

    #[pyo3(signature = (query, params=None))]
    fn execute_write(
        &self,
        query: &str,
        params: Option<HashMap<String, Py<PyAny>>>,
        py: Python<'_>,
    ) -> PyResult<u32> {
        let raw = self.raw_ptr()?;
        let query_c = CString::new(query)
            .map_err(|_| classify_nervus_error("query contains interior NUL"))?;
        let params_c = Self::encode_params(params, py)?;
        let params_ptr = params_c.as_ref().map_or(ptr::null(), |s| s.as_ptr());

        let mut affected: u32 = 0;
        capi_status(capi::ndb_execute_write(
            raw,
            query_c.as_ptr(),
            params_ptr,
            &mut affected,
        ))?;
        Ok(affected)
    }

    fn search_vector(&self, query: Vec<f32>, k: usize) -> PyResult<Vec<(u32, f32)>> {
        let raw = self.raw_ptr()?;
        let mut result_ptr: *mut capi::ndb_result_t = ptr::null_mut();
        capi_status(capi::ndb_search_vector(
            raw,
            query.as_ptr(),
            query.len(),
            k as u32,
            &mut result_ptr,
        ))?;
        if result_ptr.is_null() {
            return Err(classify_nervus_error(
                "ndb_search_vector returned null result handle",
            ));
        }

        let value = Self::result_json(result_ptr)?;
        let rows = value
            .as_array()
            .ok_or_else(|| classify_nervus_error("vector result must be array"))?;

        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let obj = row
                .as_object()
                .ok_or_else(|| classify_nervus_error("vector row must be object"))?;
            let node_id = obj
                .get("node_id")
                .and_then(JsonValue::as_u64)
                .ok_or_else(|| classify_nervus_error("vector row.node_id missing"))?
                as u32;
            let distance = obj
                .get("distance")
                .and_then(JsonValue::as_f64)
                .ok_or_else(|| classify_nervus_error("vector row.distance missing"))?
                as f32;
            out.push((node_id, distance));
        }
        Ok(out)
    }

    fn compact(&self) -> PyResult<()> {
        let raw = self.raw_ptr()?;
        capi_status(capi::ndb_compact(raw))
    }

    fn checkpoint(&self) -> PyResult<()> {
        let raw = self.raw_ptr()?;
        capi_status(capi::ndb_checkpoint(raw))
    }

    fn create_index(&self, label: &str, property: &str) -> PyResult<()> {
        let raw = self.raw_ptr()?;
        let label_c = CString::new(label)
            .map_err(|_| classify_nervus_error("label contains interior NUL"))?;
        let property_c = CString::new(property)
            .map_err(|_| classify_nervus_error("property contains interior NUL"))?;
        capi_status(capi::ndb_create_index(
            raw,
            label_c.as_ptr(),
            property_c.as_ptr(),
        ))
    }

    pub(crate) fn begin_write(slf: Py<Db>, py: Python<'_>) -> PyResult<WriteTxn> {
        let db_ref = slf.borrow_mut(py);
        let raw = db_ref
            .raw
            .ok_or_else(|| classify_nervus_error("database is closed"))?;

        let mut txn_raw: *mut capi::ndb_txn_t = ptr::null_mut();
        capi_status(capi::ndb_begin_write(raw, &mut txn_raw))?;
        if txn_raw.is_null() {
            return Err(classify_nervus_error(
                "ndb_begin_write returned null transaction handle",
            ));
        }

        let counter = db_ref.active_write_txns.clone();
        counter.fetch_add(1, Ordering::SeqCst);
        Ok(WriteTxn::new(txn_raw, slf.clone_ref(py), counter))
    }

    pub(crate) fn close(&mut self) -> PyResult<()> {
        if self.active_write_txns.load(Ordering::SeqCst) != 0 {
            return Err(classify_nervus_error(
                "Cannot close database: write transaction in progress",
            ));
        }
        if let Some(raw) = self.raw.take() {
            capi_status(capi::ndb_close(raw))?;
        }
        Ok(())
    }

    #[getter]
    fn path(&self) -> String {
        self.ndb_path.to_string_lossy().to_string()
    }

    #[getter]
    fn ndb_path(&self) -> String {
        self.ndb_path.to_string_lossy().to_string()
    }

    #[getter]
    fn wal_path(&self) -> String {
        self.wal_path.to_string_lossy().to_string()
    }
}
