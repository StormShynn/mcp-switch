use crate::types::{McpError, McpServerEntry};

/// An adapter reads MCP server definitions from a specific tool's config
/// and can write back the enabled servers.
pub trait Adapter: Send + Sync {
    /// The unique identifier for this tool (e.g. "claude", "codex").
    fn id(&self) -> &'static str;

    /// Read MCP servers from this tool's config file.
    fn read_servers(&self) -> Result<Vec<McpServerEntry>, McpError>;

    /// Write only the enabled servers back to this tool's config.
    fn write_enabled(&self, enabled: &[McpServerEntry]) -> Result<(), McpError>;
}

mod claude;
mod codex;
mod gemini;
mod hermes;
mod opencode;

/// Returns all registered adapters.
pub fn all_adapters() -> Vec<Box<dyn Adapter>> {
    vec![
        Box::new(claude::ClaudeAdapter),
        Box::new(codex::CodexAdapter),
        Box::new(gemini::GeminiAdapter),
        Box::new(hermes::HermesAdapter),
        Box::new(opencode::OpenCodeAdapter),
    ]
}

/// Find an adapter by app id.
pub fn adapter_for(app_id: &str) -> Option<Box<dyn Adapter>> {
    all_adapters().into_iter().find(|a| a.id() == app_id)
}
