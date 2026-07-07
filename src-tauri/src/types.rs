use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_transport() -> String {
    "stdio".to_string()
}

/// A single MCP server entry in the store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerEntry {
    pub name: String,
    /// Transport kind: "stdio" (default, launches `command`), "http", or
    /// "sse" (both connect to `url`). Absent in older store files, which
    /// predate remote MCP servers and are always stdio.
    #[serde(default = "default_transport")]
    pub transport: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    /// Remote endpoint, set when `transport` is "http" or "sse".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
    /// Map of app_id -> enabled state
    pub enabled: HashMap<String, bool>,
    /// App ids whose real config actually defines this server (as of the last
    /// import). Drives which per-app toggle(s) the UI shows for this entry.
    #[serde(default)]
    pub sources: Vec<String>,
}

/// The single source of truth store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Store {
    pub servers: Vec<McpServerEntry>,
}

impl Store {
    pub fn empty() -> Self {
        Store {
            servers: Vec::new(),
        }
    }

    pub fn find_server_mut(&mut self, name: &str) -> Option<&mut McpServerEntry> {
        self.servers.iter_mut().find(|s| s.name == name)
    }

    pub fn upsert_server(&mut self, entry: McpServerEntry) {
        if let Some(existing) = self.find_server_mut(&entry.name) {
            *existing = entry;
        } else {
            self.servers.push(entry);
        }
    }

}

/// Identifiers for supported coding tools.
pub const APPS: &[&str] = &[
    "claude",
    "claude-desktop",
    "codex",
    "gemini",
    "hermes",
    "opencode",
    "antigravity",
];

/// Error type for MCP Switch operations.
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("TOML error: {0}")]
    Toml(#[from] toml::de::Error),
    #[error("Server not found: {0}")]
    ServerNotFound(String),
    #[error("Unknown app: {0}")]
    UnknownApp(String),
    #[error("Invalid config format: {0}")]
    InvalidConfig(String),
}

impl From<McpError> for String {
    fn from(e: McpError) -> Self {
        e.to_string()
    }
}

impl serde::Serialize for McpError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
