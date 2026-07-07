use crate::atomic::read_file_optional;
use crate::mcp_json;
use crate::paths;
use crate::types::{McpError, McpServerEntry};
use crate::winshim;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

use super::Adapter;

pub struct AntigravityAdapter;

impl Adapter for AntigravityAdapter {
    fn id(&self) -> &'static str {
        "antigravity"
    }

    fn read_servers(&self) -> Result<Vec<McpServerEntry>, McpError> {
        let path = paths::antigravity_config();
        let Some(content) = read_file_optional(&path)? else {
            return Ok(Vec::new());
        };

        #[derive(serde::Deserialize)]
        struct AntigravityConfig {
            #[serde(default, rename = "mcpServers")]
            mcp_servers: Option<HashMap<String, Value>>,
        }

        let config: AntigravityConfig = serde_json::from_str(&content)?;
        let servers = config
            .mcp_servers
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(name, spec)| match entry_from_spec(&name, &spec) {
                Ok(entry) => Some(entry),
                Err(e) => {
                    eprintln!("Skipping invalid Antigravity MCP server '{name}': {e}");
                    None
                }
            })
            .collect();

        Ok(servers)
    }

    fn write_server(&self, name: &str, entry: Option<&McpServerEntry>) -> Result<(), McpError> {
        let path = paths::antigravity_config();
        let content = read_file_optional(&path)?.unwrap_or_else(|| "{}".to_string());

        #[derive(serde::Deserialize, serde::Serialize)]
        struct AntigravityConfig {
            #[serde(default, skip_serializing_if = "Option::is_none", rename = "mcpServers")]
            mcp_servers: Option<HashMap<String, Value>>,
            // Preserve unrelated top-level keys.
            #[serde(flatten)]
            extra: serde_json::Map<String, Value>,
        }

        let mut config: AntigravityConfig = serde_json::from_str(&content)?;
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

/// Antigravity has no `type` field. Local servers carry `command`/`args`/
/// `env` (e.g. a `docker run ...` entry). Remote servers use `serverUrl`
/// (NOT `url`/`httpUrl` like other tools) + `headers`; Antigravity doesn't
/// distinguish http vs sse, so remote entries import as "sse".
fn entry_from_spec(name: &str, spec: &Value) -> Result<McpServerEntry, String> {
    let obj = spec.as_object().ok_or("not a JSON object")?;

    if let Some(server_url) = obj.get("serverUrl").and_then(|v| v.as_str()) {
        return Ok(McpServerEntry {
            name: name.to_string(),
            transport: "sse".to_string(),
            command: None,
            args: None,
            env: None,
            url: Some(server_url.to_string()),
            headers: mcp_json::string_map(obj, "headers"),
            enabled: HashMap::new(),
            sources: Vec::new(),
        });
    }

    let command = obj
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or("missing 'command'/'serverUrl' field")?;
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

/// Writes back in Antigravity's own shape: `serverUrl` (not `url`) for
/// remote servers, regardless of whether the unified transport is "http" or
/// "sse". Stdio commands get the Windows `cmd /c` shim wrapper applied.
fn spec_from_entry(entry: &McpServerEntry) -> Value {
    let mut obj = Map::new();

    if entry.transport == "http" || entry.transport == "sse" {
        obj.insert(
            "serverUrl".into(),
            json!(entry.url.clone().unwrap_or_default()),
        );
        if let Some(headers) = &entry.headers {
            if !headers.is_empty() {
                obj.insert("headers".into(), json!(headers));
            }
        }
        return Value::Object(obj);
    }

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
    Value::Object(obj)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdio_entry_infers_from_command_field() {
        let spec = json!({
            "command": "docker",
            "args": ["run", "-i", "--rm"],
            "env": {"KEY": "val"}
        });
        let entry = entry_from_spec("github", &spec).unwrap();
        assert_eq!(entry.transport, "stdio");
        assert_eq!(entry.command, Some("docker".to_string()));
        assert_eq!(entry.env.unwrap().get("KEY"), Some(&"val".to_string()));
    }

    #[test]
    fn server_url_field_maps_to_sse_transport() {
        let spec = json!({
            "serverUrl": "https://api.githubcopilot.com/mcp/",
            "headers": {"Authorization": "Bearer xyz"}
        });
        let entry = entry_from_spec("github", &spec).unwrap();
        assert_eq!(entry.transport, "sse");
        assert_eq!(
            entry.url,
            Some("https://api.githubcopilot.com/mcp/".to_string())
        );

        let written = spec_from_entry(&entry);
        assert_eq!(written["serverUrl"], "https://api.githubcopilot.com/mcp/");
        assert!(written.get("url").is_none());
        assert!(written.get("httpUrl").is_none());
    }

    #[test]
    fn entry_without_command_or_server_url_is_rejected() {
        let spec = json!({"foo": "bar"});
        assert!(entry_from_spec("bad", &spec).is_err());
    }
}
