use crate::atomic::read_file_optional;
use crate::paths;
use crate::types::{McpError, McpServerEntry};
use crate::winshim;
use std::collections::HashMap;

use super::Adapter;

pub struct CodexAdapter;

impl Adapter for CodexAdapter {
    fn id(&self) -> &'static str {
        "codex"
    }

    fn read_servers(&self) -> Result<Vec<McpServerEntry>, McpError> {
        let path = paths::codex_config();
        let Some(content) = read_file_optional(&path)? else {
            return Ok(Vec::new());
        };

        #[derive(serde::Deserialize)]
        struct CodexConfig {
            #[serde(default)]
            mcp_servers: HashMap<String, toml::Value>,
        }

        let config: CodexConfig = toml::from_str(&content).map_err(|e| {
            McpError::InvalidConfig(format!("Codex config.toml: {e}"))
        })?;
        let servers = config
            .mcp_servers
            .into_iter()
            .filter_map(|(name, spec)| match entry_from_toml(&name, &spec, self.id()) {
                Ok(entry) => Some(entry),
                Err(e) => {
                    eprintln!("Skipping invalid Codex MCP server '{name}': {e}");
                    None
                }
            })
            .collect();

        Ok(servers)
    }

    fn write_server(&self, name: &str, entry: Option<&McpServerEntry>) -> Result<(), McpError> {
        let path = paths::codex_config();
        let content = read_file_optional(&path)?.unwrap_or_else(String::new);

        #[derive(serde::Deserialize, serde::Serialize)]
        struct CodexConfig {
            #[serde(default, skip_serializing_if = "HashMap::is_empty")]
            mcp_servers: HashMap<String, toml::Value>,
            // Preserve unrelated top-level keys (model_provider, projects, windows, ...).
            #[serde(flatten)]
            extra: HashMap<String, toml::Value>,
        }

        let mut config: CodexConfig = match toml::from_str(&content) {
            Ok(c) => c,
            Err(_) => CodexConfig {
                mcp_servers: HashMap::new(),
                extra: HashMap::new(),
            },
        };

        match entry {
            Some(e) => {
                config.mcp_servers.insert(name.to_string(), toml_from_entry(e));
            }
            None => {
                config.mcp_servers.remove(name);
            }
        }

        let output = toml::to_string_pretty(&config).map_err(|e| {
            McpError::InvalidConfig(format!("TOML serialization error: {e}"))
        })?;
        crate::atomic::atomic_write(&path, &output)
    }
}

/// Parses one `[mcp_servers.*]` entry. Codex requires an explicit `type`
/// ("stdio", "http", or "sse"); stdio carries `command`/`args`/`env`, while
/// http/sse carry `url`/`http_headers`.
fn entry_from_toml(name: &str, value: &toml::Value, app: &str) -> Result<McpServerEntry, String> {
    let table = value.as_table().ok_or("not a TOML table")?;
    let transport = table.get("type").and_then(|v| v.as_str()).unwrap_or("stdio");

    match transport {
        "http" | "sse" => {
            let url = table
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or("missing 'url' field")?;
            let headers = table
                .get("http_headers")
                .and_then(|v| v.as_table())
                .map(toml_table_to_string_map);
            Ok(McpServerEntry {
                name: name.to_string(),
                app: app.to_string(),
                transport: transport.to_string(),
                command: None,
                args: None,
                env: None,
                url: Some(url.to_string()),
                headers,
                enabled: true,
                deleted: false,
            })
        }
        "stdio" => {
            let command = table
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or("missing 'command' field")?;
            let args = table.get("args").and_then(|v| v.as_array()).map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            });
            let env = table
                .get("env")
                .and_then(|v| v.as_table())
                .map(toml_table_to_string_map);
            Ok(McpServerEntry {
                name: name.to_string(),
                app: app.to_string(),
                transport: "stdio".to_string(),
                command: Some(command.to_string()),
                args,
                env,
                url: None,
                headers: None,
                enabled: true,
                deleted: false,
            })
        }
        other => Err(format!("unsupported type '{other}'")),
    }
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

/// Builds the `[mcp_servers.<name>]` table to write. Stdio commands get the
/// Windows `cmd /c` shim wrapper applied.
fn toml_from_entry(entry: &McpServerEntry) -> toml::Value {
    let mut table = toml::value::Table::new();

    if entry.transport == "http" || entry.transport == "sse" {
        table.insert("type".into(), toml::Value::String(entry.transport.clone()));
        table.insert(
            "url".into(),
            toml::Value::String(entry.url.clone().unwrap_or_default()),
        );
        if let Some(headers) = &entry.headers {
            if !headers.is_empty() {
                table.insert(
                    "http_headers".into(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stdio_entry_defaults_to_stdio_when_type_absent() {
        let value: toml::Value = toml::from_str(
            r#"
            command = "npx"
            args = ["-y", "foo"]
            "#,
        )
        .unwrap();
        let entry = entry_from_toml("foo", &value, "codex").unwrap();
        assert_eq!(entry.transport, "stdio");
        assert_eq!(entry.command, Some("npx".to_string()));
    }

    #[test]
    fn http_entry_uses_http_headers_field() {
        let value: toml::Value = toml::from_str(
            r#"
            type = "http"
            url = "https://example.com/mcp"

            [http_headers]
            Authorization = "Bearer xyz"
            "#,
        )
        .unwrap();
        let entry = entry_from_toml("remote", &value, "codex").unwrap();
        assert_eq!(entry.transport, "http");
        assert_eq!(entry.url, Some("https://example.com/mcp".to_string()));
        assert_eq!(
            entry.headers.as_ref().unwrap().get("Authorization"),
            Some(&"Bearer xyz".to_string())
        );

        let written = toml_from_entry(&entry);
        let table = written.as_table().unwrap();
        assert_eq!(table["type"].as_str(), Some("http"));
        assert!(table.contains_key("http_headers"));
        assert!(!table.contains_key("headers"));
    }

    #[test]
    fn sse_entry_without_url_is_rejected() {
        let value: toml::Value = toml::from_str(r#"type = "sse""#).unwrap();
        assert!(entry_from_toml("bad", &value, "codex").is_err());
    }
}
