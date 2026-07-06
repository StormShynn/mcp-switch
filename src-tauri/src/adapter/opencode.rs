use crate::atomic::read_file_optional;
use crate::paths;
use crate::types::{McpError, McpServerEntry};
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
            #[serde(default)]
            mcpServers: Option<HashMap<String, OpenCodeMcpServer>>,
        }

        #[derive(serde::Deserialize)]
        struct OpenCodeMcpServer {
            command: String,
            #[serde(default)]
            args: Option<Vec<String>>,
            #[serde(default)]
            env: Option<HashMap<String, String>>,
        }

        let config: OpenCodeConfig = serde_json::from_str(&content)?;
        let servers = config
            .mcpServers
            .unwrap_or_default()
            .into_iter()
            .map(|(name, s)| McpServerEntry {
                name,
                command: s.command,
                args: s.args,
                env: s.env,
                enabled: HashMap::new(),
            })
            .collect();

        Ok(servers)
    }

    fn write_enabled(&self, enabled: &[McpServerEntry]) -> Result<(), McpError> {
        let path = paths::opencode_config();
        let content = read_file_optional(&path)?.unwrap_or_else(|| "{}".to_string());

        #[derive(serde::Deserialize, serde::Serialize)]
        struct OpenCodeConfig {
            #[serde(default, skip_serializing_if = "Option::is_none")]
            mcpServers: Option<HashMap<String, OpenCodeMcpServer>>,
        }

        #[derive(serde::Serialize)]
        struct OpenCodeMcpServer {
            command: String,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            args: Option<Vec<String>>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            env: Option<HashMap<String, String>>,
        }

        let mut config: OpenCodeConfig = serde_json::from_str(&content)?;
        let mut servers = HashMap::new();
        for entry in enabled {
            servers.insert(
                entry.name.clone(),
                OpenCodeMcpServer {
                    command: entry.command.clone(),
                    args: entry.args.clone(),
                    env: entry.env.clone(),
                },
            );
        }
        config.mcpServers = if servers.is_empty() {
            None
        } else {
            Some(servers)
        };

        let output = serde_json::to_string_pretty(&config)?;
        crate::atomic::atomic_write(&path, &output)
    }
}
