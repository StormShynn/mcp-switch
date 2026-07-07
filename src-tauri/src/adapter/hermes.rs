use std::collections::HashMap;

use crate::atomic::read_file_optional;
use crate::mcp_json;
use crate::paths;
use crate::types::{McpError, McpServerEntry};
use crate::winshim;
use serde_json::{json, Map, Value};

use super::Adapter;

pub struct HermesAdapter;

impl Adapter for HermesAdapter {
    fn id(&self) -> &'static str {
        "hermes"
    }

    fn read_servers(&self) -> Result<Vec<McpServerEntry>, McpError> {
        // Try TOML first, fall back to JSON
        let toml_path = paths::hermes_config();
        let json_path = paths::hermes_config_json();

        if toml_path.exists() {
            self.read_toml(&toml_path)
        } else if json_path.exists() {
            self.read_json(&json_path)
        } else {
            Ok(Vec::new())
        }
    }

    fn write_server(&self, name: &str, entry: Option<&McpServerEntry>) -> Result<(), McpError> {
        let toml_path = paths::hermes_config();
        let json_path = paths::hermes_config_json();

        if toml_path.exists() {
            self.write_toml(&toml_path, name, entry)
        } else {
            self.write_json(&json_path, name, entry)
        }
    }
}

impl HermesAdapter {
    fn read_toml(&self, path: &std::path::Path) -> Result<Vec<McpServerEntry>, McpError> {
        let Some(content) = read_file_optional(path)? else {
            return Ok(Vec::new());
        };

        #[derive(serde::Deserialize)]
        struct HermesConfig {
            #[serde(default)]
            mcp_servers: Option<Vec<toml::Value>>,
        }

        let config: HermesConfig = toml::from_str(&content)?;
        let servers = config
            .mcp_servers
            .unwrap_or_default()
            .into_iter()
            .filter_map(|spec| match entry_from_toml(&spec) {
                Ok(entry) => Some(entry),
                Err(e) => {
                    eprintln!("Skipping invalid Hermes MCP server: {e}");
                    None
                }
            })
            .collect();

        Ok(servers)
    }

    fn read_json(&self, path: &std::path::Path) -> Result<Vec<McpServerEntry>, McpError> {
        let Some(content) = read_file_optional(path)? else {
            return Ok(Vec::new());
        };

        #[derive(serde::Deserialize)]
        struct HermesConfig {
            #[serde(default)]
            mcp_servers: Option<Vec<Value>>,
        }

        let config: HermesConfig = serde_json::from_str(&content)?;
        let servers = config
            .mcp_servers
            .unwrap_or_default()
            .into_iter()
            .filter_map(|spec| match entry_from_json(&spec) {
                Ok(entry) => Some(entry),
                Err(e) => {
                    eprintln!("Skipping invalid Hermes MCP server: {e}");
                    None
                }
            })
            .collect();

        Ok(servers)
    }

    fn write_toml(
        &self,
        path: &std::path::Path,
        name: &str,
        entry: Option<&McpServerEntry>,
    ) -> Result<(), McpError> {
        let content = read_file_optional(path)?.unwrap_or_default();

        #[derive(serde::Deserialize, serde::Serialize)]
        struct HermesConfig {
            #[serde(default)]
            mcp_servers: Option<Vec<toml::Value>>,
            // Preserve unrelated top-level keys.
            #[serde(flatten)]
            extra: HashMap<String, toml::Value>,
        }

        let mut config: HermesConfig = toml::from_str(&content).unwrap_or(HermesConfig {
            mcp_servers: None,
            extra: HashMap::new(),
        });

        let mut servers = config.mcp_servers.unwrap_or_default();
        upsert_or_remove_by_name(
            &mut servers,
            name,
            |v| v.as_table().and_then(|t| t.get("name")).and_then(|v| v.as_str()),
            entry.map(toml_from_entry),
        );

        config.mcp_servers = if servers.is_empty() {
            None
        } else {
            Some(servers)
        };

        let output = toml::to_string_pretty(&config)
            .map_err(|e| McpError::InvalidConfig(format!("TOML serialization: {e}")))?;
        crate::atomic::atomic_write(path, &output)
    }

    fn write_json(
        &self,
        path: &std::path::Path,
        name: &str,
        entry: Option<&McpServerEntry>,
    ) -> Result<(), McpError> {
        let content = read_file_optional(path)?.unwrap_or_else(|| "{}".to_string());

        #[derive(serde::Deserialize, serde::Serialize)]
        struct HermesConfig {
            #[serde(default, skip_serializing_if = "Option::is_none")]
            mcp_servers: Option<Vec<Value>>,
            // Preserve unrelated top-level keys.
            #[serde(flatten)]
            extra: serde_json::Map<String, Value>,
        }

        let mut config: HermesConfig = serde_json::from_str(&content)?;
        let mut servers = config.mcp_servers.unwrap_or_default();
        upsert_or_remove_by_name(
            &mut servers,
            name,
            |v| v.as_object().and_then(|o| o.get("name")).and_then(|v| v.as_str()),
            entry.map(json_from_entry),
        );

        config.mcp_servers = if servers.is_empty() {
            None
        } else {
            Some(servers)
        };

        let output = serde_json::to_string_pretty(&config)?;
        crate::atomic::atomic_write(path, &output)
    }
}

/// Upserts (`Some`) or removes (`None`) the item whose `name_of` matches
/// `name` in `items`, preserving every other item and their relative order.
fn upsert_or_remove_by_name<T>(
    items: &mut Vec<T>,
    name: &str,
    name_of: impl Fn(&T) -> Option<&str>,
    new_item: Option<T>,
) {
    let index = items.iter().position(|item| name_of(item) == Some(name));
    match (new_item, index) {
        (Some(item), Some(idx)) => items[idx] = item,
        (Some(item), None) => items.push(item),
        (None, Some(idx)) => {
            items.remove(idx);
        }
        (None, None) => {}
    }
}

// ============================================================================
// TOML <-> unified entry
// ============================================================================

/// Hermes has no `type` field: `command` present -> stdio, otherwise `url` ->
/// remote (Hermes doesn't distinguish http/sse on read, so remote entries
/// always come back as "sse"; `write_*` treats "http" the same as "sse").
fn entry_from_toml(value: &toml::Value) -> Result<McpServerEntry, String> {
    let table = value.as_table().ok_or("not a TOML table")?;
    let name = table
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or("missing 'name' field")?
        .to_string();

    if let Some(command) = table.get("command").and_then(|v| v.as_str()) {
        let args = table.get("args").and_then(|v| v.as_array()).map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        });
        let env = table
            .get("env")
            .and_then(|v| v.as_table())
            .map(toml_table_to_string_map);
        return Ok(McpServerEntry {
            name,
            transport: "stdio".to_string(),
            command: Some(command.to_string()),
            args,
            env,
            url: None,
            headers: None,
            enabled: HashMap::new(),
            sources: Vec::new(),
        });
    }

    if let Some(url) = table.get("url").and_then(|v| v.as_str()) {
        let headers = table
            .get("headers")
            .and_then(|v| v.as_table())
            .map(toml_table_to_string_map);
        return Ok(McpServerEntry {
            name,
            transport: "sse".to_string(),
            command: None,
            args: None,
            env: None,
            url: Some(url.to_string()),
            headers,
            enabled: HashMap::new(),
            sources: Vec::new(),
        });
    }

    Err("neither 'command' nor 'url' field present".to_string())
}

fn toml_from_entry(entry: &McpServerEntry) -> toml::Value {
    let mut table = toml::value::Table::new();
    table.insert("name".into(), toml::Value::String(entry.name.clone()));

    if entry.transport == "http" || entry.transport == "sse" {
        table.insert(
            "url".into(),
            toml::Value::String(entry.url.clone().unwrap_or_default()),
        );
        if let Some(headers) = &entry.headers {
            if !headers.is_empty() {
                table.insert(
                    "headers".into(),
                    toml::Value::Table(string_map_to_toml_table(headers)),
                );
            }
        }
        return toml::Value::Table(table);
    }

    let (command, args) =
        winshim::wrap_for_windows(entry.command.as_deref().unwrap_or_default(), entry.args.clone());
    table.insert("command".into(), toml::Value::String(command));
    if let Some(args) = args {
        if !args.is_empty() {
            table.insert(
                "args".into(),
                toml::Value::Array(args.into_iter().map(toml::Value::String).collect()),
            );
        }
    }
    if let Some(env) = &entry.env {
        if !env.is_empty() {
            table.insert(
                "env".into(),
                toml::Value::Table(string_map_to_toml_table(env)),
            );
        }
    }
    toml::Value::Table(table)
}

fn toml_table_to_string_map(table: &toml::value::Table) -> HashMap<String, String> {
    table
        .iter()
        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
        .collect()
}

fn string_map_to_toml_table(map: &HashMap<String, String>) -> toml::value::Table {
    map.iter()
        .map(|(k, v)| (k.clone(), toml::Value::String(v.clone())))
        .collect()
}

// ============================================================================
// JSON <-> unified entry
// ============================================================================

fn entry_from_json(spec: &Value) -> Result<McpServerEntry, String> {
    let obj = spec.as_object().ok_or("not a JSON object")?;
    let name = obj
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or("missing 'name' field")?
        .to_string();

    if let Some(command) = obj.get("command").and_then(|v| v.as_str()) {
        return Ok(McpServerEntry {
            name,
            transport: "stdio".to_string(),
            command: Some(command.to_string()),
            args: mcp_json::string_array(obj, "args"),
            env: mcp_json::string_map(obj, "env"),
            url: None,
            headers: None,
            enabled: HashMap::new(),
            sources: Vec::new(),
        });
    }

    if let Some(url) = obj.get("url").and_then(|v| v.as_str()) {
        return Ok(McpServerEntry {
            name,
            transport: "sse".to_string(),
            command: None,
            args: None,
            env: None,
            url: Some(url.to_string()),
            headers: mcp_json::string_map(obj, "headers"),
            enabled: HashMap::new(),
            sources: Vec::new(),
        });
    }

    Err("neither 'command' nor 'url' field present".to_string())
}

fn json_from_entry(entry: &McpServerEntry) -> Value {
    let mut obj = Map::new();
    obj.insert("name".into(), json!(entry.name));

    if entry.transport == "http" || entry.transport == "sse" {
        obj.insert("url".into(), json!(entry.url.clone().unwrap_or_default()));
        if let Some(headers) = &entry.headers {
            if !headers.is_empty() {
                obj.insert("headers".into(), json!(headers));
            }
        }
        return Value::Object(obj);
    }

    let (command, args) =
        winshim::wrap_for_windows(entry.command.as_deref().unwrap_or_default(), entry.args.clone());
    obj.insert("command".into(), json!(command));
    if let Some(args) = args {
        if !args.is_empty() {
            obj.insert("args".into(), json!(args));
        }
    }
    if let Some(env) = &entry.env {
        if !env.is_empty() {
            obj.insert("env".into(), json!(env));
        }
    }
    Value::Object(obj)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toml_stdio_entry_infers_from_command_field() {
        let value: toml::Value = toml::from_str(
            r#"
            name = "fs"
            command = "npx"
            args = ["-y", "foo"]

            [env]
            KEY = "val"
            "#,
        )
        .unwrap();
        let entry = entry_from_toml(&value).unwrap();
        assert_eq!(entry.name, "fs");
        assert_eq!(entry.transport, "stdio");
        assert_eq!(entry.env.unwrap().get("KEY"), Some(&"val".to_string()));
    }

    #[test]
    fn toml_remote_entry_infers_sse_from_url_field() {
        let value: toml::Value = toml::from_str(
            r#"
            name = "remote"
            url = "https://example.com/mcp"
            "#,
        )
        .unwrap();
        let entry = entry_from_toml(&value).unwrap();
        assert_eq!(entry.transport, "sse");
        assert_eq!(entry.url, Some("https://example.com/mcp".to_string()));
    }

    #[test]
    fn json_entry_missing_name_is_rejected() {
        let spec = json!({"command": "npx"});
        assert!(entry_from_json(&spec).is_err());
    }

    #[test]
    fn json_http_transport_writes_plain_url_no_type() {
        let entry = McpServerEntry {
            name: "remote".to_string(),
            transport: "http".to_string(),
            command: None,
            args: None,
            env: None,
            url: Some("https://example.com/mcp".to_string()),
            headers: None,
            enabled: HashMap::new(),
            sources: Vec::new(),
        };
        let written = json_from_entry(&entry);
        assert_eq!(written["url"], "https://example.com/mcp");
        assert!(written.get("type").is_none());
        assert!(written.get("command").is_none());
    }

    // ---- upsert_or_remove_by_name tests ----

    #[test]
    fn upsert_replaces_matching_item_in_place() {
        let mut items = vec!["a:1".to_string(), "b:2".to_string(), "c:3".to_string()];
        upsert_or_remove_by_name(
            &mut items,
            "b",
            |s| s.split(':').next(),
            Some("b:99".to_string()),
        );
        assert_eq!(items, vec!["a:1", "b:99", "c:3"]);
    }

    #[test]
    fn upsert_appends_when_not_found() {
        let mut items = vec!["a:1".to_string()];
        upsert_or_remove_by_name(
            &mut items,
            "new",
            |s| s.split(':').next(),
            Some("new:1".to_string()),
        );
        assert_eq!(items, vec!["a:1", "new:1"]);
    }

    #[test]
    fn remove_drops_matching_item_and_leaves_others_untouched() {
        let mut items = vec!["a:1".to_string(), "b:2".to_string(), "c:3".to_string()];
        upsert_or_remove_by_name(&mut items, "b", |s| s.split(':').next(), None);
        assert_eq!(items, vec!["a:1", "c:3"]);
    }

    #[test]
    fn remove_of_untracked_name_is_a_no_op() {
        // This is the core guarantee the toggle refactor depends on: a name
        // MCP Switch never imported (added/edited outside MCP Switch) must
        // survive being written to when an unrelated server is toggled.
        let mut items = vec!["foreign:1".to_string(), "tracked:2".to_string()];
        upsert_or_remove_by_name(&mut items, "tracked", |s| s.split(':').next(), None);
        assert_eq!(items, vec!["foreign:1"]);
    }
}
