use crate::atomic::read_file_optional;
use crate::paths;
use crate::types::{McpError, McpServerEntry};
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
            mcp_servers: Option<HashMap<String, GeminiMcpServer>>,
        }

        #[derive(serde::Deserialize)]
        struct GeminiMcpServer {
            command: String,
            #[serde(default)]
            args: Option<Vec<String>>,
        }

        let config: GeminiConfig = serde_json::from_str(&content)?;
        let servers = config
            .mcp_servers
            .unwrap_or_default()
            .into_iter()
            .map(|(name, s)| McpServerEntry {
                name,
                command: s.command,
                args: s.args,
                env: None,
                enabled: HashMap::new(),
            })
            .collect();

        Ok(servers)
    }

    fn write_enabled(&self, enabled: &[McpServerEntry]) -> Result<(), McpError> {
        let path = paths::gemini_config();
        let content = read_file_optional(&path)?.unwrap_or_else(|| "{}".to_string());

        #[derive(serde::Deserialize, serde::Serialize)]
        struct GeminiConfig {
            #[serde(default, skip_serializing_if = "Option::is_none", rename = "mcpServers")]
            mcp_servers: Option<HashMap<String, GeminiMcpServer>>,
        }

        #[derive(serde::Deserialize, serde::Serialize)]
        struct GeminiMcpServer {
            command: String,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            args: Option<Vec<String>>,
        }

        let mut config: GeminiConfig = serde_json::from_str(&content)?;
        let mut servers = HashMap::new();
        for entry in enabled {
            servers.insert(
                entry.name.clone(),
                GeminiMcpServer {
                    command: entry.command.clone(),
                    args: entry.args.clone(),
                },
            );
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
