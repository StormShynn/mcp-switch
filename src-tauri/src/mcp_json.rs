//! Small helpers for pulling typed fields out of a loosely-typed
//! `serde_json::Value` object. Adapters parse `mcpServers` entries this way
//! (rather than a strongly-typed struct) so one malformed or unrecognized
//! entry can be skipped without failing the whole file's import.

use serde_json::{Map, Value};
use std::collections::HashMap;

pub fn string_array(obj: &Map<String, Value>, key: &str) -> Option<Vec<String>> {
    obj.get(key)?.as_array().map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    })
}

pub fn string_map(obj: &Map<String, Value>, key: &str) -> Option<HashMap<String, String>> {
    obj.get(key)?.as_object().map(|m| {
        m.iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
            .collect()
    })
}
