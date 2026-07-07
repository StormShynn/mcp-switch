use crate::atomic::read_file_optional;
use crate::mcp_json;
use crate::paths;
use crate::types::{McpError, McpServerEntry};
use crate::winshim;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

use super::Adapter;

pub struct ClaudeDesktopAdapter;

impl Adapter for ClaudeDesktopAdapter {
    fn id(&self) -> &'static str {
        "claude-desktop"
    }

    fn read_servers(&self) -> Result<Vec<McpServerEntry>, McpError> {
        let path = paths::claude_desktop_config();
        let Some(content) = read_file_optional(&path)? else {
            return Ok(Vec::new());
        };

        #[derive(serde::Deserialize)]
        struct ClaudeDesktopConfig {
            #[serde(default, rename = "mcpServers")]
            mcp_servers: Option<HashMap<String, Value>>,
        }

        let config: ClaudeDesktopConfig = serde_json::from_str(&content)?;
        let servers = config
            .mcp_servers
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(name, spec)| match entry_from_spec(&name, &spec) {
                Ok(entry) => Some(entry),
                Err(e) => {
                    eprintln!("Skipping invalid Claude Desktop MCP server '{name}': {e}");
                    None
                }
            })
            .collect();

        Ok(servers)
    }

    fn write_server(&self, name: &str, entry: Option<&McpServerEntry>) -> Result<(), McpError> {
        let path = paths::claude_desktop_config();
        let content = read_file_optional(&path)?.unwrap_or_else(|| "{}".to_string());

        #[derive(serde::Deserialize, serde::Serialize)]
        struct ClaudeDesktopConfig {
            #[serde(default, skip_serializing_if = "Option::is_none", rename = "mcpServers")]
            mcp_servers: Option<HashMap<String, Value>>,
            // Preserve unrelated top-level keys (globalShortcut, theme, ...).
            #[serde(flatten)]
            extra: serde_json::Map<String, Value>,
        }

        let mut config: ClaudeDesktopConfig = serde_json::from_str(&content)?;
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

/// Claude Desktop uses the same `mcpServers` shape as Claude Code: `type`
/// defaults to "stdio" when absent; "http"/"sse" carry `url`/`headers`
/// instead of `command`/`args`/`env`.
fn entry_from_spec(name: &str, spec: &Value) -> Result<McpServerEntry, String> {
    let obj = spec.as_object().ok_or("not a JSON object")?;
    let transport = obj.get("type").and_then(|v| v.as_str()).unwrap_or("stdio");

    match transport {
        "http" | "sse" => {
            let url = obj
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or("missing 'url' field")?;
            Ok(McpServerEntry {
                name: name.to_string(),
                transport: transport.to_string(),
                command: None,
                args: None,
                env: None,
                url: Some(url.to_string()),
                headers: mcp_json::string_map(obj, "headers"),
                enabled: HashMap::new(),
                sources: Vec::new(),
                deleted: false,
            })
        }
        "stdio" => {
            let command = obj
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or("missing 'command' field")?;
            Ok(McpServerEntry {
                name: name.to_string(),
                transport: "stdio".to_string(),
                command: Some(command.to_string()),
                args: mcp_json::string_array(obj, "args"),
                env: mcp_json::string_map(obj, "env"),
                url: None,
                headers: None,
                enabled: HashMap::new(),
                sources: Vec::new(),
                deleted: false,
            })
        }
        other => Err(format!("unsupported type '{other}'")),
    }
}

fn spec_from_entry(entry: &McpServerEntry) -> Value {
    if entry.transport == "http" || entry.transport == "sse" {
        let mut obj = Map::new();
        obj.insert("type".into(), json!(entry.transport));
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
    let mut obj = Map::new();
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
