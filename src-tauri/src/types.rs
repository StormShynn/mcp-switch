use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single MCP server entry in the store.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerEntry {
    pub name: String,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,
    /// Map of app_id -> enabled state
    pub enabled: HashMap<String, bool>,
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

    pub fn find_server(&self, name: &str) -> Option<&McpServerEntry> {
        self.servers.iter().find(|s| s.name == name)
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

    pub fn remove_server(&mut self, name: &str) {
        self.servers.retain(|s| s.name != name);
    }
}

/// Identifiers for supported coding tools.
pub const APPS: &[&str] = &["claude", "codex", "gemini", "hermes", "opencode"];

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
