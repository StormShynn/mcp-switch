use crate::atomic::read_file_optional;
use crate::paths;
use crate::types::{McpError, McpServerEntry};
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
            mcpServers: Option<Vec<CodexMcpServer>>,
        }

        #[derive(serde::Deserialize)]
        struct CodexMcpServer {
            name: String,
            command: String,
            #[serde(default)]
            args: Option<Vec<String>>,
        }

        let config: CodexConfig = toml::from_str(&content)?;
        let servers = config
            .mcpServers
            .unwrap_or_default()
            .into_iter()
            .map(|s| McpServerEntry {
                name: s.name,
                command: s.command,
                args: s.args,
                env: None,
                enabled: HashMap::new(),
            })
            .collect();

        Ok(servers)
    }

    fn write_enabled(&self, enabled: &[McpServerEntry]) -> Result<(), McpError> {
        let path = paths::codex_config();
        // Codex config is TOML - we rewrite the mcpServers array entirely
        let content = read_file_optional(&path)?.unwrap_or_else(|| String::new());

        #[derive(serde::Deserialize, serde::Serialize)]
        struct CodexConfig {
            #[serde(default)]
            mcpServers: Option<Vec<CodexMcpServer>>,
        }

        #[derive(serde::Serialize, serde::Deserialize)]
        struct CodexMcpServer {
            name: String,
            command: String,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            args: Option<Vec<String>>,
        }

        // Parse existing to preserve other fields
        let mut config: CodexConfig = match toml::from_str(&content) {
            Ok(c) => c,
            Err(_) => CodexConfig { mcpServers: None },
        };

        let servers: Vec<CodexMcpServer> = enabled
            .iter()
            .map(|e| CodexMcpServer {
                name: e.name.clone(),
                command: e.command.clone(),
                args: e.args.clone(),
            })
            .collect();

        config.mcpServers = if servers.is_empty() {
            None
        } else {
            Some(servers)
        };

        let output = toml::to_string_pretty(&config).map_err(|e| {
            McpError::InvalidConfig(format!("TOML serialization error: {e}"))
        })?;
        crate::atomic::atomic_write(&path, &output)
    }
}
