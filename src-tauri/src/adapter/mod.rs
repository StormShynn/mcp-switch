use crate::types::{McpError, McpServerEntry};

/// An adapter reads MCP server definitions from a specific tool's config
/// and can write a single server back into it.
pub trait Adapter: Send + Sync {
    /// The unique identifier for this tool (e.g. "claude", "codex").
    fn id(&self) -> &'static str;

    /// Read MCP servers from this tool's config file.
    fn read_servers(&self) -> Result<Vec<McpServerEntry>, McpError>;

    /// Upsert (`Some`) or remove (`None`) a single named server in this
    /// tool's live config. Every other entry already on disk — including
    /// ones MCP Switch doesn't track, e.g. added or edited outside MCP
    /// Switch since the last import — is read fresh and left untouched, so
    /// toggling one server can never clobber or delete another.
    fn write_server(&self, name: &str, entry: Option<&McpServerEntry>) -> Result<(), McpError>;
}

mod antigravity;
mod claude;
mod claude_desktop;
mod codex;
mod gemini;
mod hermes;
mod opencode;

/// Returns all registered adapters.
pub fn all_adapters() -> Vec<Box<dyn Adapter>> {
    vec![
        Box::new(claude::ClaudeAdapter),
        Box::new(claude_desktop::ClaudeDesktopAdapter),
        Box::new(codex::CodexAdapter),
        Box::new(gemini::GeminiAdapter),
        Box::new(hermes::HermesAdapter),
        Box::new(opencode::OpenCodeAdapter),
        Box::new(antigravity::AntigravityAdapter),
    ]
}

/// Find an adapter by app id.
pub fn adapter_for(app_id: &str) -> Option<Box<dyn Adapter>> {
    all_adapters().into_iter().find(|a| a.id() == app_id)
}
