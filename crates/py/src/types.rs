//! Convert between Python dicts and engine option structs by routing
//! through `serde_json`. This avoids hand-writing a `FromPyObject` impl
//! for every engine option, and matches the engine's existing `serde`
//! contract used by the HTTP server.

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyString};
use serde::de::DeserializeOwned;

pub fn from_py<T: DeserializeOwned + Default>(opts: Option<&Bound<'_, PyDict>>) -> PyResult<T> {
    let Some(d) = opts else {
        return Ok(T::default());
    };
    let json = pyobject_to_json(d.as_any())?;
    serde_json::from_value(json).map_err(|e| {
        super::errors::ValidationError::new_err(format!("invalid options: {e}"))
    })
}

fn pyobject_to_json(obj: &Bound<'_, PyAny>) -> PyResult<serde_json::Value> {
    if obj.is_none() {
        return Ok(serde_json::Value::Null);
    }
    if let Ok(b) = obj.extract::<bool>() {
        return Ok(b.into());
    }
    if let Ok(i) = obj.extract::<i64>() {
        return Ok(i.into());
    }
    if let Ok(f) = obj.extract::<f64>() {
        return Ok(f.into());
    }
    if let Ok(s) = obj.downcast::<PyString>() {
        return Ok(s.to_string_lossy().to_string().into());
    }
    if let Ok(seq) = obj.downcast::<pyo3::types::PyList>() {
        let mut out = Vec::with_capacity(seq.len());
        for item in seq.iter() {
            out.push(pyobject_to_json(&item)?);
        }
        return Ok(serde_json::Value::Array(out));
    }
    if let Ok(d) = obj.downcast::<PyDict>() {
        let mut map = serde_json::Map::new();
        for (k, v) in d.iter() {
            let k: String = k.extract()?;
            map.insert(k, pyobject_to_json(&v)?);
        }
        return Ok(serde_json::Value::Object(map));
    }
    let type_name = obj.get_type().name()?.to_string_lossy().to_string();
    Err(super::errors::ValidationError::new_err(format!(
        "unsupported python type: {type_name}"
    )))
}
