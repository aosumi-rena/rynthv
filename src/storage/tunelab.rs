mod cbor;
mod controls;
mod read;
mod write;
use crate::model::*;
use serde_json::{Value, json};
type Result<T> = std::result::Result<T, String>;
const SOURCE: &str = "rynthvTuneLab";
const SCALE: f64 = QUARTER as f64 / 480.0;
fn number(v: &Value, key: &str, default: f64) -> Result<f64> {
    match v.get(key) {
        None => Ok(default),
        Some(x) => x
            .as_f64()
            .filter(|x| x.is_finite())
            .ok_or_else(|| format!("Invalid TuneLab {key}")),
    }
}
fn clock(v: f64) -> Result<i64> {
    let b = v * SCALE;
    if !b.is_finite() || b.abs() >= (i64::MAX / 4) as f64 {
        return Err("TuneLab position overflow".into());
    }
    Ok(b.round() as i64)
}
fn tick(v: i64) -> f64 {
    v as f64 / SCALE
}
fn array<'a>(v: &'a Value, key: &str) -> Result<&'a [Value]> {
    v.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| format!("TuneLab {key} must be an array"))
}
fn text<'a>(v: &'a Value, key: &str, default: &'a str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or(default)
}
fn object(v: &Value) -> Value {
    if v.is_object() { v.clone() } else { json!({}) }
}
fn retained(v: &Value, omit: &[&str]) -> Value {
    let mut v = object(v);
    for k in omit {
        v.as_object_mut().unwrap().remove(*k);
    }
    v
}
pub(super) fn decode(bytes: &[u8], binary: bool) -> Result<Project> {
    let value = if binary {
        cbor::decode(bytes)?
    } else {
        serde_json::from_slice(bytes).map_err(|e| format!("TuneLab JSON: {e}"))?
    };
    read::project(value)
}
pub(super) fn encode(project: &Project, binary: bool) -> Result<Vec<u8>> {
    let value = write::project(project)?;
    if binary {
        cbor::encode(&value)
    } else {
        serde_json::to_vec_pretty(&value).map_err(|e| e.to_string())
    }
}
