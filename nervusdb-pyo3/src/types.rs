use pyo3::prelude::*;
use pyo3::types::{PyAny, PyDict, PyList};
use serde_json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue};
use std::collections::BTreeMap;

#[pyclass]
#[derive(Debug)]
pub struct Node {
    #[pyo3(get)]
    pub id: u64,
    #[pyo3(get)]
    pub labels: Vec<String>,
    pub properties: BTreeMap<String, PyObject>,
}

#[pymethods]
impl Node {
    #[getter]
    fn properties(&self, py: Python<'_>) -> PyResult<PyObject> {
        let dict = PyDict::new_bound(py);
        for (k, v) in &self.properties {
            dict.set_item(k, v.clone_ref(py))?;
        }
        Ok(dict.into())
    }
}

#[pyclass]
#[derive(Debug)]
pub struct Relationship {
    #[pyo3(get)]
    pub id: Option<u64>,
    #[pyo3(get)]
    pub start_node_id: u64,
    #[pyo3(get)]
    pub end_node_id: u64,
    #[pyo3(get)]
    pub rel_type: String,
    pub properties: BTreeMap<String, PyObject>,
}

#[pymethods]
impl Relationship {
    #[getter]
    fn properties(&self, py: Python<'_>) -> PyResult<PyObject> {
        let dict = PyDict::new_bound(py);
        for (k, v) in &self.properties {
            dict.set_item(k, v.clone_ref(py))?;
        }
        Ok(dict.into())
    }
}

#[pyclass]
#[derive(Debug)]
pub struct Path {
    pub nodes: Vec<Node>,
    pub relationships: Vec<Relationship>,
}

#[pymethods]
impl Path {
    #[getter]
    fn nodes(&self, py: Python<'_>) -> PyResult<Vec<Node>> {
        let mut out = Vec::new();
        for n in &self.nodes {
            let mut props = BTreeMap::new();
            for (k, v) in &n.properties {
                props.insert(k.clone(), v.clone_ref(py));
            }
            out.push(Node {
                id: n.id,
                labels: n.labels.clone(),
                properties: props,
            });
        }
        Ok(out)
    }

    #[getter]
    fn relationships(&self, py: Python<'_>) -> PyResult<Vec<Relationship>> {
        let mut out = Vec::new();
        for r in &self.relationships {
            let mut props = BTreeMap::new();
            for (k, v) in &r.properties {
                props.insert(k.clone(), v.clone_ref(py));
            }
            out.push(Relationship {
                id: r.id,
                start_node_id: r.start_node_id,
                end_node_id: r.end_node_id,
                rel_type: r.rel_type.clone(),
                properties: props,
            });
        }
        Ok(out)
    }
}

pub fn py_to_json(obj: &Bound<'_, PyAny>) -> PyResult<JsonValue> {
    if obj.is_none() {
        return Ok(JsonValue::Null);
    }

    if let Ok(b) = obj.extract::<bool>() {
        return Ok(JsonValue::Bool(b));
    }

    if let Ok(i) = obj.extract::<i64>() {
        return Ok(JsonValue::Number(JsonNumber::from(i)));
    }

    if let Ok(f) = obj.extract::<f64>() {
        return JsonNumber::from_f64(f)
            .map(JsonValue::Number)
            .ok_or_else(|| pyo3::exceptions::PyTypeError::new_err("float parameter is NaN/inf"));
    }

    if let Ok(s) = obj.extract::<String>() {
        return Ok(JsonValue::String(s));
    }

    if let Ok(list) = obj.downcast::<PyList>() {
        let mut out = Vec::with_capacity(list.len());
        for item in list.iter() {
            out.push(py_to_json(&item)?);
        }
        return Ok(JsonValue::Array(out));
    }

    if let Ok(dict) = obj.downcast::<PyDict>() {
        let mut out = JsonMap::new();
        for (k, v) in dict.iter() {
            let key = k.extract::<String>().map_err(|_| {
                pyo3::exceptions::PyTypeError::new_err("Dictionary keys must be strings")
            })?;
            out.insert(key, py_to_json(&v)?);
        }
        return Ok(JsonValue::Object(out));
    }

    Err(pyo3::exceptions::PyTypeError::new_err(
        "Unsupported type for parameter/property value",
    ))
}

fn json_to_py_map(map: JsonMap<String, JsonValue>, py: Python<'_>) -> Py<PyAny> {
    let dict = PyDict::new_bound(py);
    for (k, v) in map {
        let _ = dict.set_item(k, json_to_py(v, py));
    }
    dict.into()
}

fn node_from_json(obj: &JsonMap<String, JsonValue>, py: Python<'_>) -> Node {
    let id = obj
        .get("id")
        .and_then(JsonValue::as_u64)
        .unwrap_or_default();
    let labels = obj
        .get("labels")
        .and_then(JsonValue::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(JsonValue::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let properties = obj
        .get("properties")
        .and_then(JsonValue::as_object)
        .map(|props| {
            let mut out = BTreeMap::new();
            for (k, v) in props {
                out.insert(k.clone(), json_to_py(v.clone(), py));
            }
            out
        })
        .unwrap_or_default();

    Node {
        id,
        labels,
        properties,
    }
}

fn relationship_from_json(obj: &JsonMap<String, JsonValue>, py: Python<'_>) -> Relationship {
    let src = obj
        .get("src")
        .and_then(JsonValue::as_u64)
        .unwrap_or_default();
    let dst = obj
        .get("dst")
        .and_then(JsonValue::as_u64)
        .unwrap_or_default();
    let rel_type = obj
        .get("rel_type")
        .and_then(JsonValue::as_str)
        .unwrap_or("")
        .to_string();

    let properties = obj
        .get("properties")
        .and_then(JsonValue::as_object)
        .map(|props| {
            let mut out = BTreeMap::new();
            for (k, v) in props {
                out.insert(k.clone(), json_to_py(v.clone(), py));
            }
            out
        })
        .unwrap_or_default();

    Relationship {
        id: Some(src ^ dst ^ 0x0102_0304_0506_0708),
        start_node_id: src,
        end_node_id: dst,
        rel_type,
        properties,
    }
}

fn path_from_json(obj: &JsonMap<String, JsonValue>, py: Python<'_>) -> Path {
    let nodes = obj
        .get("nodes")
        .and_then(JsonValue::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(JsonValue::as_object)
                .map(|o| node_from_json(o, py))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let relationships = obj
        .get("relationships")
        .and_then(JsonValue::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(JsonValue::as_object)
                .map(|o| relationship_from_json(o, py))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Path {
        nodes,
        relationships,
    }
}

pub fn json_to_py(val: JsonValue, py: Python<'_>) -> Py<PyAny> {
    match val {
        JsonValue::Null => py.None(),
        JsonValue::Bool(b) => b.into_py(py),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.into_py(py)
            } else if let Some(u) = n.as_u64() {
                u.into_py(py)
            } else if let Some(f) = n.as_f64() {
                f.into_py(py)
            } else {
                py.None()
            }
        }
        JsonValue::String(s) => s.into_py(py),
        JsonValue::Array(arr) => {
            let py_list = PyList::new_bound(py, arr.into_iter().map(|v| json_to_py(v, py)));
            py_list.into()
        }
        JsonValue::Object(obj) => {
            if let Some(kind) = obj.get("type").and_then(JsonValue::as_str) {
                match kind {
                    "node" => return node_from_json(&obj, py).into_py(py),
                    "relationship" => return relationship_from_json(&obj, py).into_py(py),
                    "path" => return path_from_json(&obj, py).into_py(py),
                    "node_id" | "external_id" => {
                        let value = obj.get("value").cloned().unwrap_or(JsonValue::Null);
                        return json_to_py(value, py);
                    }
                    _ => {}
                }
            }
            json_to_py_map(obj, py)
        }
    }
}
