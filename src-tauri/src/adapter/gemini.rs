use crate::atomic::read_file_optional;
use crate::mcp_json;
use crate::paths;
use crate::types::{McpError, McpServerEntry};
use crate::winshim;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

use super::Adapter;

pub struct GeminiAdapter;

impl Adapter for GeminiAdapter {
    fn id(&self) -> &'static str {
        "gemini"
    }

    fn read_servers(&self) -> Result<Vec<McpServerEntry>, McpError> {
        let path = paths::gemini_config();
        let Some(content) = read_file_optional(&path)? else {
            return Ok(Vec::new());
        };

        #[derive(serde::Deserialize)]
        struct GeminiConfig {
            #[serde(default, rename = "mcpServers")]
            mcp_servers: Option<HashMap<String, Value>>,
        }

        let config: GeminiConfig = serde_json::from_str(&content)?;
        let servers = config
            .mcp_servers
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(name, spec)| match entry_from_spec(&name, &spec, self.id()) {
                Ok(entry) => Some(entry),
                Err(e) => {
                    eprintln!("Skipping invalid Gemini MCP server '{name}': {e}");
                    None
                }
            })
            .collect();

        Ok(servers)
    }

    fn write_server(&self, name: &str, entry: Option<&McpServerEntry>) -> Result<(), McpError> {
        let path = paths::gemini_config();
        let content = read_file_optional(&path)?.unwrap_or_else(|| "{}".to_string());

        #[derive(serde::Deserialize, serde::Serialize)]
        struct GeminiConfig {
            #[serde(default, skip_serializing_if = "Option::is_none", rename = "mcpServers")]
            mcp_servers: Option<HashMap<String, Value>>,
            // Preserve unrelated top-level keys (theme, selectedAuthType, ...).
            #[serde(flatten)]
            extra: serde_json::Map<String, Value>,
        }

        let mut config: GeminiConfig = serde_json::from_str(&content)?;
        let mut servers = config.mcp_servers.unwrap_or_default();
        match entry {
            Some(e) => {
                servers.insert(name.to_string(), spec_from_entry(e));
            }
            None => {
                servers.remove(name);
            }
        }
        config.mcp_servers = if servers.is_empty() {
            None
        } else {
            Some(servers)
        };

        let output = serde_json::to_string_pretty(&config)?;
        crate::atomic::atomic_write(&path, &output)
    }
}

/// Gemini CLI has no `type` field — it infers transport from which field is
/// present: `command` -> stdio, `httpUrl` -> http, `url` (alone) -> sse.
fn entry_from_spec(name: &str, spec: &Value, app: &str) -> Result<McpServerEntry, String> {
    let obj = spec.as_object().ok_or("not a JSON object")?;

    if let Some(http_url) = obj.get("httpUrl").and_then(|v| v.as_str()) {
        return Ok(McpServerEntry {
            name: name.to_string(),
            app: app.to_string(),
            transport: "http".to_string(),
            command: None,
            args: None,
            env: None,
            url: Some(http_url.to_string()),
            headers: mcp_json::string_map(obj, "headers"),
            enabled: true,
            deleted: false,
        });
    }
    if let Some(url) = obj.get("url").and_then(|v| v.as_str()) {
        return Ok(McpServerEntry {
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
        });
    }

    let command = obj
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or("missing 'command'/'url'/'httpUrl' field")?;
    Ok(McpServerEntry {
        name: name.to_string(),
        app: app.to_string(),
        transport: "stdio".to_string(),
        command: Some(command.to_string()),
        args: mcp_json::string_array(obj, "args"),
        env: mcp_json::string_map(obj, "env"),
        url: None,
        headers: None,
        enabled: true,
        deleted: false,
    })
}

/// Writes back in Gemini's own shape: no `type` field, `httpUrl` for http,
/// plain `url` for sse. Stdio commands get the Windows `cmd /c` shim wrapper
/// applied.
fn spec_from_entry(entry: &McpServerEntry) -> Value {
    let mut obj = Map::new();

    match entry.transport.as_str() {
        "http" => {
            obj.insert("httpUrl".into(), json!(entry.url.clone().unwrap_or_default()));
            if let Some(headers) = &entry.headers {
                if !headers.is_empty() {
                    obj.insert("headers".into(), json!(headers));
                }
            }
        }
        "sse" => {
            obj.insert("url".into(), json!(entry.url.clone().unwrap_or_default()));
            if let Some(headers) = &entry.headers {
                if !headers.is_empty() {
                    obj.insert("headers".into(), json!(headers));
                }
            }
        }
        _ => {
            let (command, args) = winshim::wrap_for_windows(
                entry.command.as_deref().unwrap_or_default(),
                entry.args.clone(),
            );
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
        }
    }

    Value::Object(obj)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdio_entry_infers_from_command_field() {
        let spec = json!({"command": "npx", "args": ["-y", "foo"], "env": {"KEY": "val"}});
        let entry = entry_from_spec("foo", &spec, "gemini").unwrap();
        assert_eq!(entry.transport, "stdio");
        assert_eq!(entry.command, Some("npx".to_string()));
        assert_eq!(entry.env.unwrap().get("KEY"), Some(&"val".to_string()));
    }

    #[test]
    fn http_url_field_maps_to_http_transport() {
        let spec = json!({"httpUrl": "https://example.com/mcp"});
        let entry = entry_from_spec("remote", &spec, "gemini").unwrap();
        assert_eq!(entry.transport, "http");
        assert_eq!(entry.url, Some("https://example.com/mcp".to_string()));

        let written = spec_from_entry(&entry);
        assert_eq!(written["httpUrl"], "https://example.com/mcp");
        assert!(written.get("url").is_none());
    }

    #[test]
    fn bare_url_field_maps_to_sse_transport() {
        let spec = json!({"url": "https://example.com/sse"});
        let entry = entry_from_spec("remote", &spec, "gemini").unwrap();
        assert_eq!(entry.transport, "sse");

        let written = spec_from_entry(&entry);
        assert_eq!(written["url"], "https://example.com/sse");
        assert!(written.get("httpUrl").is_none());
    }

    #[test]
    fn entry_without_command_or_url_is_rejected() {
        let spec = json!({"foo": "bar"});
        assert!(entry_from_spec("bad", &spec, "gemini").is_err());
    }
}
