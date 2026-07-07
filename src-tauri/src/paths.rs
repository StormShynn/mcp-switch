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

/// Returns candidate config paths for the Claude Desktop app, in priority order.
///
/// Distribution channel changes where this file actually lives:
/// - Windows Store (MSIX) builds: AppData is virtualized per package, so the file
///   ends up under `%LOCALAPPDATA%\Packages\Claude_<PackageFamilyName>\LocalCache\Roaming\Claude\...`.
///   The `<PackageFamilyName>` suffix isn't hardcoded here since it isn't guaranteed
///   constant across installs; instead we scan `Packages/` for a `Claude_*` entry.
/// - Regular installer: `<config_dir>/Claude/claude_desktop_config.json`
///   (`%APPDATA%\Claude` on Windows, `~/Library/Application Support/Claude` on macOS).
///
/// The Store candidate is listed first: when a `Claude_*` package exists, Windows'
/// per-package AppData virtualization means that's the file the packaged app
/// actually reads and writes. A plain `%APPDATA%\Claude\claude_desktop_config.json`
/// can still exist alongside it (e.g. a stale leftover from an earlier non-Store
/// install) without ever being touched by the Store app, so it must not outrank
/// the Store path just because it happens to exist too.
pub fn claude_desktop_config_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    #[cfg(target_os = "windows")]
    if let Some(local_data) = dirs::data_local_dir() {
        let packages_dir = local_data.join("Packages");
        if let Ok(entries) = std::fs::read_dir(&packages_dir) {
            for entry in entries.flatten() {
                let is_claude_pkg = entry
                    .file_name()
                    .to_str()
                    .is_some_and(|n| n.starts_with("Claude_"));
                if is_claude_pkg {
                    candidates.push(
                        entry
                            .path()
                            .join("LocalCache")
                            .join("Roaming")
                            .join("Claude")
                            .join("claude_desktop_config.json"),
                    );
                }
            }
        }
    }

    if let Some(config_dir) = dirs::config_dir() {
        candidates.push(config_dir.join("Claude").join("claude_desktop_config.json"));
    }

    candidates
}

/// Returns the Claude Desktop config path to use: the first candidate that already
/// exists, or the standard (non-Store) location if none do yet.
pub fn claude_desktop_config() -> PathBuf {
    let candidates = claude_desktop_config_candidates();
    candidates
        .iter()
        .find(|p| p.exists())
        .cloned()
        .or_else(|| candidates.into_iter().next())
        .expect("could not resolve a Claude Desktop config path")
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

/// Returns the config file path for Hermes Agent's real, documented format:
/// `~/.hermes/config.yaml` (confirmed against NousResearch's own docs —
/// `mcp_servers` there is a map keyed by server name, the same shape
/// Claude/Codex/Gemini use, not a list).
pub fn hermes_config_yaml() -> PathBuf {
    let home = dirs::home_dir().expect("could not find home directory");
    home.join(".hermes").join("config.yaml")
}

/// Older guesses at Hermes's config format, kept as a fallback in case an
/// installation genuinely uses one of these instead — but unlike
/// `hermes_config_yaml`, neither has been confirmed against real Hermes
/// documentation or a real install. [Unverified]
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

/// Returns candidate config paths for Google Antigravity, in priority order.
///
/// Antigravity's MCP config location has moved between versions and public
/// docs disagree on the current default:
/// - Newer (Antigravity 2.0, unified across IDE/CLI, per Google's own June
///   2026 Codelabs docs): `~/.gemini/config/mcp_config.json`.
/// - Older (per GitHub's github-mcp-server install guide):
///   `~/.gemini/antigravity/mcp_config.json`.
///
/// Both are checked; whichever already exists wins, so an existing older
/// install isn't abandoned just because a newer convention was documented
/// elsewhere. [Unverified] Neither path has been confirmed against a real
/// Antigravity install as of writing.
pub fn antigravity_config_candidates() -> Vec<PathBuf> {
    let home = dirs::home_dir().expect("could not find home directory");
    let gemini_dir = home.join(".gemini");
    vec![
        gemini_dir.join("config").join("mcp_config.json"),
        gemini_dir.join("antigravity").join("mcp_config.json"),
    ]
}

/// Returns the Antigravity config path to use: the first candidate that
/// already exists, or the newer (unified) location if none do yet.
pub fn antigravity_config() -> PathBuf {
    let candidates = antigravity_config_candidates();
    candidates
        .iter()
        .find(|p| p.exists())
        .cloned()
        .or_else(|| candidates.into_iter().next())
        .expect("could not resolve an Antigravity config path")
}

