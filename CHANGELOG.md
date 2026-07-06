# Changelog

## 0.1.0 (initial scaffold)

- Tauri 2 + Rust + React + TypeScript desktop app.
- SSOT store at `~/.mcp-switch/store.json` with per-app enabled flags.
- Adapters for Claude Code, Codex, Gemini CLI, Hermes, OpenCode.
- Atomic file writes (tmp + rename + fsync) to avoid corrupting native configs.
- `tauri-plugin-updater` wired to GitHub Releases (tag-driven workflow).
- UI: per-server row with 5 toggle switches, search, app filter, sync-all.
  "Import from ..." buttons discover servers from installed tools.