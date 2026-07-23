//! Small helpers for pulling typed fields out of a loosely-typed value
//! object (`serde_json::Value`, `toml::Value`, or `serde_yaml::Value`).
//! Adapters parse `mcpServers` entries this way (rather than a
//! strongly-typed struct) so one malformed or unrecognized entry can be
//! skipped without failing the whole file's import.
//!
//! Also holds the `extra`-field capture/apply helpers every adapter uses so
//! a live config field outside `McpServerEntry`'s own shape (Codex's `cwd`,
//! Gemini's `timeout`, ...) survives being read in and written back out,
//! instead of silently vanishing the next time MCP Switch rewrites that
//! server. `extra` is always stored as JSON internally regardless of the
//! source format, so the TOML/YAML variants below convert through it.

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::collections::HashMap;

/// Parses JSON that may contain trailing commas before `}`/`]` — common in
/// hand-edited config files (OpenCode's `config.json`, VS Code-style
/// settings) even though strict JSON forbids them. One misplaced comma
/// shouldn't take down an entire app's sync, so every adapter reads and
/// writes through this instead of `serde_json::from_str` directly.
pub fn from_str_lenient<T: DeserializeOwned>(content: &str) -> serde_json::Result<T> {
    serde_json::from_str(&strip_trailing_commas(content))
}

/// Drops a `,` that (ignoring whitespace) is immediately followed by `}` or
/// `]`, unless it's inside a string literal. Leaves well-formed JSON
/// byte-for-byte unchanged.
fn strip_trailing_commas(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;

    while let Some(c) = chars.next() {
        if in_string {
            out.push(c);
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            continue;
        }

        if c == '"' {
            in_string = true;
            out.push(c);
            continue;
        }

        if c == ',' {
            let mut lookahead = chars.clone();
            let next_significant = loop {
                match lookahead.peek() {
                    Some(nc) if nc.is_whitespace() => {
                        lookahead.next();
                    }
                    other => break other.copied(),
                }
            };
            if matches!(next_significant, Some('}') | Some(']')) {
                continue; // drop the trailing comma
            }
        }

        out.push(c);
    }

    out
}

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

/// Collects every key in `obj` not in `known_keys` into a generic bucket.
pub fn capture_extra(obj: &Map<String, Value>, known_keys: &[&str]) -> HashMap<String, Value> {
    obj.iter()
        .filter(|(k, _)| !known_keys.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Applies a captured `extra` bucket onto `obj`, for keys `obj` doesn't
/// already have set — a known field the caller set explicitly always wins
/// over a stale extra value of the same name.
pub fn apply_extra(obj: &mut Map<String, Value>, extra: &HashMap<String, Value>) {
    for (k, v) in extra {
        obj.entry(k.clone()).or_insert_with(|| v.clone());
    }
}

/// TOML equivalents of `capture_extra`/`apply_extra`, converting through
/// JSON so the `extra` bucket stays one format regardless of source.
/// Lossy only for TOML's native datetime type, which has no JSON
/// equivalent and round-trips as its own string representation.
pub fn capture_extra_toml(table: &toml::value::Table, known_keys: &[&str]) -> HashMap<String, Value> {
    table
        .iter()
        .filter(|(k, _)| !known_keys.contains(&k.as_str()))
        .filter_map(|(k, v)| Some((k.clone(), serde_json::to_value(v).ok()?)))
        .collect()
}

pub fn apply_extra_toml(table: &mut toml::value::Table, extra: &HashMap<String, Value>) {
    for (k, v) in extra {
        if !table.contains_key(k) {
            if let Ok(tv) = toml::Value::try_from(v) {
                table.insert(k.clone(), tv);
            }
        }
    }
}

/// YAML equivalents of `capture_extra`/`apply_extra`.
pub fn capture_extra_yaml(mapping: &serde_yaml::Mapping, known_keys: &[&str]) -> HashMap<String, Value> {
    mapping
        .iter()
        .filter_map(|(k, v)| {
            let key = k.as_str()?;
            if known_keys.contains(&key) {
                return None;
            }
            Some((key.to_string(), serde_json::to_value(v).ok()?))
        })
        .collect()
}

pub fn apply_extra_yaml(mapping: &mut serde_yaml::Mapping, extra: &HashMap<String, Value>) {
    for (k, v) in extra {
        let key = serde_yaml::Value::String(k.clone());
        if !mapping.contains_key(&key) {
            if let Ok(yv) = serde_yaml::to_value(v) {
                mapping.insert(key, yv);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_trailing_comma_before_closing_brace() {
        let input = "{\n  \"$schema\": \"https://opencode.ai/config.json\",}";
        let value: Value = from_str_lenient(input).unwrap();
        assert_eq!(value["$schema"], "https://opencode.ai/config.json");
    }

    #[test]
    fn strips_trailing_comma_before_closing_bracket() {
        let input = r#"{"list": ["a", "b",]}"#;
        let value: Value = from_str_lenient(input).unwrap();
        assert_eq!(value["list"], serde_json::json!(["a", "b"]));
    }

    #[test]
    fn leaves_comma_inside_string_value_untouched() {
        let input = r#"{"note": "a, b,}"}"#;
        let value: Value = from_str_lenient(input).unwrap();
        assert_eq!(value["note"], "a, b,}");
    }

    #[test]
    fn leaves_well_formed_json_unchanged() {
        let input = r#"{"a": 1, "b": [1, 2, 3]}"#;
        let value: Value = from_str_lenient(input).unwrap();
        assert_eq!(value["a"], 1);
        assert_eq!(value["b"], serde_json::json!([1, 2, 3]));
    }

    #[test]
    fn genuinely_invalid_json_still_errors() {
        let result: serde_json::Result<Value> = from_str_lenient("{not json");
        assert!(result.is_err());
    }
}
