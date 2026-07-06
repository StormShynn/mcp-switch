use std::path::PathBuf;

/// Returns the MCP Switch store path: ~/.mcp-switch/store.json
pub fn store_path() -> PathBuf {
    let home = dirs::home_dir().expect("could not find home directory");
    home.join(".mcp-switch").join("store.json")
}

/// Returns the config file path for Claude Code: ~/.claude.json
pub fn claude_config() -> PathBuf {
    let home = dirs::home_dir().expect("could not find home directory");
    home.join(".claude.json")
}

/// Returns the config file path for Codex CLI: ~/.codex/config.toml
pub fn codex_config() -> PathBuf {
    let home = dirs::home_dir().expect("could not find home directory");
    home.join(".codex").join("config.toml")
}

/// Returns the config file path for Gemini CLI: ~/.gemini/settings.json
pub fn gemini_config() -> PathBuf {
    let home = dirs::home_dir().expect("could not find home directory");
    home.join(".gemini").join("settings.json")
}

/// Returns the config file path for Hermes.
/// Hermes supports both .toml and .json; we try TOML first.
pub fn hermes_config() -> PathBuf {
    let home = dirs::home_dir().expect("could not find home directory");
    home.join(".hermes").join("config.toml")
}

pub fn hermes_config_json() -> PathBuf {
    let home = dirs::home_dir().expect("could not find home directory");
    home.join(".hermes").join("config.json")
}

/// Returns the config file path for OpenCode: ~/.config/opencode/config.json
pub fn opencode_config() -> PathBuf {
    let home = dirs::home_dir().expect("could not find home directory");
    home.join(".config").join("opencode").join("config.json")
}

/// Returns the store directory to ensure it exists.
pub fn store_dir() -> PathBuf {
    let home = dirs::home_dir().expect("could not find home directory");
    home.join(".mcp-switch")
}
