use crate::atomic::read_file_optional;
use crate::mcp_json;
use crate::paths;
use crate::types::{McpError, McpServerEntry};
use crate::winshim;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

use super::Adapter;

pub struct OpenCodeAdapter;

impl Adapter for OpenCodeAdapter {
    fn id(&self) -> &'static str {
        "opencode"
    }

    fn read_servers(&self) -> Result<Vec<McpServerEntry>, McpError> {
        let path = paths::opencode_config();
        let Some(content) = read_file_optional(&path)? else {
            return Ok(Vec::new());
        };

        #[derive(serde::Deserialize)]
        struct OpenCodeConfig {
            // OpenCode's own top-level key is `mcp`, not `mcpServers`.
            #[serde(default)]
            mcp: Option<HashMap<String, Value>>,
        }

        let config: OpenCodeConfig = serde_json::from_str(&content)?;
        let servers = config
            .mcp
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(name, spec)| match entry_from_spec(&name, &spec, self.id()) {
                Ok(entry) => Some(entry),
                Err(e) => {
                    eprintln!("Skipping invalid OpenCode MCP server '{name}': {e}");
                    None
                }
            })
            .collect();

        Ok(servers)
    }

    fn write_server(&self, name: &str, entry: Option<&McpServerEntry>) -> Result<(), McpError> {
        let path = paths::opencode_config();
        let content = read_file_optional(&path)?.unwrap_or_else(|| "{}".to_string());

        #[derive(serde::Deserialize, serde::Serialize)]
        struct OpenCodeConfig {
            #[serde(default, skip_serializing_if = "Option::is_none")]
            mcp: Option<HashMap<String, Value>>,
            // Preserve unrelated top-level keys (theme, provider, ...).
            #[serde(flatten)]
            extra: serde_json::Map<String, Value>,
        }

        let mut config: OpenCodeConfig = serde_json::from_str(&content)?;
        let mut servers = config.mcp.unwrap_or_default();
        match entry {
            Some(e) => {
                servers.insert(name.to_string(), spec_from_entry(e));
            }
            None => {
                servers.remove(name);
            }
        }
        config.mcp = if servers.is_empty() {
            None
        } else {
            Some(servers)
        };

        let output = serde_json::to_string_pretty(&config)?;
        crate::atomic::atomic_write(&path, &output)
    }
}

/// OpenCode's own shape: `type: "local"` bundles the executable and its args
/// into a single `command` array (`environment` holds env vars); `type:
/// "remote"` carries `url`/`headers`. OpenCode doesn't distinguish http vs
/// sse, so remote entries import as "sse" (mirrors Hermes).
fn entry_from_spec(name: &str, spec: &Value, app: &str) -> Result<McpServerEntry, String> {
    let obj = spec.as_object().ok_or("not a JSON object")?;
    let transport = obj.get("type").and_then(|v| v.as_str()).unwrap_or("local");

    match transport {
        "remote" => {
            let url = obj
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or("missing 'url' field")?;
            Ok(McpServerEntry {
                name: name.to_string(),
                app: app.to_string(),
                transport: "sse".to_string(),
                command: None,
                args: None,
                env: None,
                url: Some(url.to_string()),
                headers: mcp_json::string_map(obj, "headers"),
                enabled: true,
                deleted: false,
            })
        }
        "local" => {
            let cmd_arr = obj
                .get("command")
                .and_then(|v| v.as_array())
                .ok_or("missing 'command' array")?;
            let mut parts = cmd_arr.iter().filter_map(|v| v.as_str().map(str::to_string));
            let command = parts.next().ok_or("'command' array is empty")?;
            let args: Vec<String> = parts.collect();
            Ok(McpServerEntry {
                name: name.to_string(),
                app: app.to_string(),
                transport: "stdio".to_string(),
                command: Some(command),
                args: if args.is_empty() { None } else { Some(args) },
                env: mcp_json::string_map(obj, "environment"),
                url: None,
                headers: None,
                enabled: true,
                deleted: false,
            })
        }
        other => Err(format!("unsupported type '{other}'")),
    }
}

/// Writes back in OpenCode's own shape. `enabled: true` is written
/// explicitly (matching OpenCode's own convention) since MCP Switch already
/// controls per-app visibility by omitting disabled entries entirely.
fn spec_from_entry(entry: &McpServerEntry) -> Value {
    let mut obj = Map::new();

    if entry.transport == "http" || entry.transport == "sse" {
        obj.insert("type".into(), json!("remote"));
        obj.insert("url".into(), json!(entry.url.clone().unwrap_or_default()));
        if let Some(headers) = &entry.headers {
            if !headers.is_empty() {
                obj.insert("headers".into(), json!(headers));
            }
        }
        obj.insert("enabled".into(), json!(true));
        return Value::Object(obj);
    }

    obj.insert("type".into(), json!("local"));
    let (command, args) = winshim::wrap_for_windows(
        entry.command.as_deref().unwrap_or_default(),
        entry.args.clone(),
    );
    let mut command_arr = vec![json!(command)];
    if let Some(args) = args {
        command_arr.extend(args.into_iter().map(|a| json!(a)));
    }
    obj.insert("command".into(), Value::Array(command_arr));
    if let Some(env) = &entry.env {
        if !env.is_empty() {
            obj.insert("environment".into(), json!(env));
        }
    }
    obj.insert("enabled".into(), json!(true));
    Value::Object(obj)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_entry_splits_command_array_into_command_and_args() {
        let spec = json!({
            "type": "local",
            "command": ["npx", "-y", "@modelcontextprotocol/server-filesystem"],
            "environment": {"HOME": "/tmp"}
        });
        let entry = entry_from_spec("fs", &spec, "opencode").unwrap();
        assert_eq!(entry.transport, "stdio");
        assert_eq!(entry.command, Some("npx".to_string()));
        assert_eq!(
            entry.args,
            Some(vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string()
            ])
        );
        assert_eq!(entry.env.unwrap().get("HOME"), Some(&"/tmp".to_string()));
    }

    #[test]
    fn local_entry_without_type_field_defaults_to_local() {
        let spec = json!({"command": ["node", "server.js"]});
        let entry = entry_from_spec("fs", &spec, "opencode").unwrap();
        assert_eq!(entry.transport, "stdio");
    }

    #[test]
    fn remote_entry_maps_to_sse_transport() {
        let spec = json!({"type": "remote", "url": "https://example.com/mcp"});
        let entry = entry_from_spec("remote", &spec, "opencode").unwrap();
        assert_eq!(entry.transport, "sse");
        assert_eq!(entry.url, Some("https://example.com/mcp".to_string()));
    }

    #[test]
    fn write_joins_command_and_args_into_single_array_with_type_and_enabled() {
        // Uses a command outside winshim's known shim list so this test
        // isolates the array-joining logic from Windows `cmd /c` wrapping
        // (covered separately by winshim's own tests).
        let entry = McpServerEntry {
            name: "fs".to_string(),
            app: "opencode".to_string(),
            transport: "stdio".to_string(),
            command: Some("python3".to_string()),
            args: Some(vec!["server.py".to_string()]),
            env: None,
            url: None,
            headers: None,
            enabled: true,
            deleted: false,
        };
        let written = spec_from_entry(&entry);
        assert_eq!(written["type"], "local");
        assert_eq!(written["command"], json!(["python3", "server.py"]));
        assert_eq!(written["enabled"], true);
    }

    #[cfg(windows)]
    #[test]
    fn write_wraps_npx_as_first_array_element_on_windows() {
        let entry = McpServerEntry {
            name: "fs".to_string(),
            app: "opencode".to_string(),
            transport: "stdio".to_string(),
            command: Some("npx".to_string()),
            args: Some(vec!["-y".to_string(), "foo".to_string()]),
            env: None,
            url: None,
            headers: None,
            enabled: true,
            deleted: false,
        };
        let written = spec_from_entry(&entry);
        assert_eq!(written["command"], json!(["cmd", "/c", "npx", "-y", "foo"]));
    }

    #[test]
    fn write_remote_entry_uses_remote_type() {
        let entry = McpServerEntry {
            name: "remote".to_string(),
            app: "opencode".to_string(),
            transport: "sse".to_string(),
            command: None,
            args: None,
            env: None,
            url: Some("https://example.com/mcp".to_string()),
            headers: None,
            enabled: true,
            deleted: false,
        };
        let written = spec_from_entry(&entry);
        assert_eq!(written["type"], "remote");
        assert_eq!(written["url"], "https://example.com/mcp");
        assert!(written.get("command").is_none());
    }
}
