use crate::atomic::read_file_optional;
use crate::mcp_json;
use crate::paths;
use crate::types::{McpError, McpServerEntry};
use crate::winshim;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

use super::Adapter;

pub struct ClaudeAdapter;

impl Adapter for ClaudeAdapter {
    fn id(&self) -> &'static str {
        "claude"
    }

    fn read_servers(&self) -> Result<Vec<McpServerEntry>, McpError> {
        let path = paths::claude_config();
        let Some(content) = read_file_optional(&path)? else {
            return Ok(Vec::new());
        };

        #[derive(serde::Deserialize)]
        struct ClaudeConfig {
            #[serde(default, rename = "mcpServers")]
            mcp_servers: Option<HashMap<String, Value>>,
        }

        let config: ClaudeConfig = serde_json::from_str(&content)?;
        let servers = config
            .mcp_servers
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(name, spec)| match entry_from_spec(&name, &spec) {
                Ok(entry) => Some(entry),
                Err(e) => {
                    eprintln!("Skipping invalid Claude MCP server '{name}': {e}");
                    None
                }
            })
            .collect();

        Ok(servers)
    }

    fn write_server(&self, name: &str, entry: Option<&McpServerEntry>) -> Result<(), McpError> {
        let path = paths::claude_config();
        let content = read_file_optional(&path)?.unwrap_or_else(|| "{}".to_string());

        #[derive(serde::Deserialize, serde::Serialize)]
        struct ClaudeConfig {
            #[serde(default, skip_serializing_if = "Option::is_none", rename = "mcpServers")]
            mcp_servers: Option<HashMap<String, Value>>,
            // Preserve unrelated top-level keys (projects, theme, onboarding flags, ...).
            #[serde(flatten)]
            extra: serde_json::Map<String, Value>,
        }

        let mut config: ClaudeConfig = serde_json::from_str(&content)?;
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

/// Parses one raw `mcpServers` entry. Claude's native shape defaults `type`
/// to "stdio" when absent; "http"/"sse" carry `url`/`headers` instead of
/// `command`/`args`/`env`.
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
            })
        }
        other => Err(format!("unsupported type '{other}'")),
    }
}

/// Builds the raw `mcpServers` entry to write. Stdio commands get the
/// Windows `cmd /c` shim wrapper applied; `type` is omitted for stdio to
/// match Claude's own convention (absent type = stdio) and keep existing
/// on-disk entries unchanged.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdio_without_type_field_defaults_to_stdio() {
        let spec = json!({"command": "node", "args": ["server.js"]});
        let entry = entry_from_spec("fs", &spec).unwrap();
        assert_eq!(entry.transport, "stdio");
        assert_eq!(entry.command, Some("node".to_string()));
        assert_eq!(entry.args, Some(vec!["server.js".to_string()]));
        assert_eq!(entry.url, None);
    }

    #[test]
    fn http_entry_requires_url() {
        let spec = json!({"type": "http"});
        assert!(entry_from_spec("bad", &spec).is_err());
    }

    #[test]
    fn sse_entry_round_trips_url_and_headers() {
        let spec = json!({
            "type": "sse",
            "url": "https://example.com/mcp",
            "headers": {"Authorization": "Bearer xyz"}
        });
        let entry = entry_from_spec("remote", &spec).unwrap();
        assert_eq!(entry.transport, "sse");
        assert_eq!(entry.url, Some("https://example.com/mcp".to_string()));
        assert_eq!(
            entry.headers.as_ref().unwrap().get("Authorization"),
            Some(&"Bearer xyz".to_string())
        );

        let written = spec_from_entry(&entry);
        assert_eq!(written["type"], "sse");
        assert_eq!(written["url"], "https://example.com/mcp");
        assert_eq!(written["headers"]["Authorization"], "Bearer xyz");
        assert!(written.get("command").is_none());
    }

    #[test]
    fn stdio_write_omits_type_field() {
        let entry = McpServerEntry {
            name: "fs".to_string(),
            transport: "stdio".to_string(),
            command: Some("node".to_string()),
            args: None,
            env: None,
            url: None,
            headers: None,
            enabled: HashMap::new(),
            sources: Vec::new(),
        };
        let written = spec_from_entry(&entry);
        assert!(written.get("type").is_none());
    }
}
