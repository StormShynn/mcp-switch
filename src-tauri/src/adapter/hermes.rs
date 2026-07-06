use std::collections::HashMap;

use crate::atomic::read_file_optional;
use crate::paths;
use crate::types::{McpError, McpServerEntry};

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

    fn write_enabled(&self, enabled: &[McpServerEntry]) -> Result<(), McpError> {
        let toml_path = paths::hermes_config();
        let json_path = paths::hermes_config_json();

        if toml_path.exists() {
            self.write_toml(&toml_path, enabled)
        } else {
            self.write_json(&json_path, enabled)
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
            mcp_servers: Option<Vec<HermesMcpServer>>,
        }

        #[derive(serde::Deserialize)]
        struct HermesMcpServer {
            name: String,
            command: String,
            #[serde(default)]
            args: Option<Vec<String>>,
        }

        let config: HermesConfig = toml::from_str(&content)?;
        let servers = config
            .mcp_servers
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

    fn read_json(&self, path: &std::path::Path) -> Result<Vec<McpServerEntry>, McpError> {
        let Some(content) = read_file_optional(path)? else {
            return Ok(Vec::new());
        };

        #[derive(serde::Deserialize)]
        struct HermesConfig {
            #[serde(default)]
            mcp_servers: Option<Vec<HermesMcpServer>>,
        }

        #[derive(serde::Deserialize)]
        struct HermesMcpServer {
            name: String,
            command: String,
            #[serde(default)]
            args: Option<Vec<String>>,
        }

        let config: HermesConfig = serde_json::from_str(&content)?;
        let servers = config
            .mcp_servers
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

    fn write_toml(&self, path: &std::path::Path, enabled: &[McpServerEntry]) -> Result<(), McpError> {
        let content = read_file_optional(path)?.unwrap_or_default();

        #[derive(serde::Deserialize, serde::Serialize)]
        struct HermesConfig {
            #[serde(default)]
            mcp_servers: Option<Vec<HermesMcpServer>>,
        }

        #[derive(serde::Deserialize, serde::Serialize)]
        struct HermesMcpServer {
            name: String,
            command: String,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            args: Option<Vec<String>>,
        }

        let mut config: HermesConfig = toml::from_str(&content).unwrap_or(HermesConfig {
            mcp_servers: None,
        });

        let servers: Vec<HermesMcpServer> = enabled
            .iter()
            .map(|e| HermesMcpServer {
                name: e.name.clone(),
                command: e.command.clone(),
                args: e.args.clone(),
            })
            .collect();

        config.mcp_servers = if servers.is_empty() {
            None
        } else {
            Some(servers)
        };

        let output = toml::to_string_pretty(&config)
            .map_err(|e| McpError::InvalidConfig(format!("TOML serialization: {e}")))?;
        crate::atomic::atomic_write(path, &output)
    }

    fn write_json(&self, path: &std::path::Path, enabled: &[McpServerEntry]) -> Result<(), McpError> {
        let content = read_file_optional(path)?.unwrap_or_else(|| "{}".to_string());

        #[derive(serde::Deserialize, serde::Serialize)]
        struct HermesConfig {
            #[serde(default, skip_serializing_if = "Option::is_none")]
            mcp_servers: Option<Vec<HermesMcpServer>>,
        }

        #[derive(serde::Serialize)]
        struct HermesMcpServer {
            name: String,
            command: String,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            args: Option<Vec<String>>,
        }

        let mut config: HermesConfig = serde_json::from_str(&content)?;
        let servers: Vec<HermesMcpServer> = enabled
            .iter()
            .map(|e| HermesMcpServer {
                name: e.name.clone(),
                command: e.command.clone(),
                args: e.args.clone(),
            })
            .collect();

        config.mcp_servers = if servers.is_empty() {
            None
        } else {
            Some(servers)
        };

        let output = serde_json::to_string_pretty(&config)?;
        crate::atomic::atomic_write(path, &output)
    }
}
