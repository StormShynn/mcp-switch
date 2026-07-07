use serde::{Deserialize, Serialize};
use std::collections::HashMap;

fn default_transport() -> String {
    "stdio".to_string()
}

/// A single MCP server entry in the store, scoped to exactly one app.
///
/// Identity is the `(name, app)` pair, not `name` alone: the same server
/// name can exist independently for several apps (e.g. "fs" for both
/// "claude" and "codex"), each with its own command/args/env/url/headers.
/// Real-world MCP servers often genuinely need different config per tool
/// (different cookie/token file paths, different transports), so entries
/// are never shared or fanned out across apps — editing one never touches
/// another app's entry, even if they happen to share a name.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerEntry {
    pub name: String,
    /// The one app this definition belongs to (e.g. "claude", "codex").
    pub app: String,
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
    /// Whether this entry is currently written into `app`'s live config.
    pub enabled: bool,
    /// Soft-trash flag: set automatically when a sync finds this server has
    /// disappeared from its app's live config while it was enabled. Never
    /// set by a manual toggle. Trashed entries keep their data and are
    /// hidden from the main list until restored or permanently deleted, so
    /// an external removal can never silently lose a working
    /// command/args/env the user might want back.
    #[serde(default)]
    pub deleted: bool,
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

    pub fn find_server_mut(&mut self, name: &str, app: &str) -> Option<&mut McpServerEntry> {
        self.servers.iter_mut().find(|s| s.name == name && s.app == app)
    }

    pub fn upsert_server(&mut self, entry: McpServerEntry) {
        if let Some(existing) = self.find_server_mut(&entry.name, &entry.app) {
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
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("Server not found: {0}")]
    ServerNotFound(String),
    #[error("Unknown app: {0}")]
    UnknownApp(String),
    #[error("Invalid config format: {0}")]
    InvalidConfig(String),
    #[error("Restart failed: {0}")]
    RestartFailed(String),
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
