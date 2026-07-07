# MCP Switch

Cross-platform desktop switcher for **MCP (Model Context Protocol) servers** across
the major coding agents:

- **Claude Code** (`~/.claude.json`)
- **Claude Desktop** (`claude_desktop_config.json`)
- **Codex CLI** (`~/.codex/config.toml`)
- **Gemini CLI** (`~/.gemini/settings.json`)
- **Hermes** (`~/.hermes/config.{toml,json}`)
- **OpenCode** (`~/.config/opencode/config.json`)

Built with **Tauri 2 + Rust + React + TypeScript**.

## What it does

- **One canonical store** at `~/.mcp-switch/store.json` listing every MCP server.
  Each server has a per-app `enabled` flag.
- **Toggle a switch in the UI** -> the app's native config is rewritten
  (atomically) to include only the servers you enabled there. Other tools
  are left untouched.
- **Import** existing servers from any installed tool into the store without
  losing their config.
- **Auto-update** via GitHub Releases using `tauri-plugin-updater`.

## Why an SSOT?

Each tool stores MCP servers in its own format (TOML/JSON, different keys,
different conventions). Editing five files by hand to keep them in sync is
error-prone. The store gives you **one place to decide which MCP servers are
active in which tool**, and the rest is mechanical projection.

## Build

```bash
# 1. Install Rust: https://rustup.rs
# 2. Install Node.js >= 20
# 3. Install system deps for your OS (Tauri prerequisites)
#    - macOS: Xcode Command Line Tools
#    - Ubuntu/Debian: libwebkit2gtk-4.1-dev, libappindicator3-dev, librsvg2-dev, patchelf
#    - Windows: Microsoft Visual Studio C++ Build Tools + WebView2

npm install
npm run tauri:dev        # development
npm run tauri:build      # production bundle in src-tauri/target/release/bundle/
```

## Project layout

```
mcp-switch/
├── src/                  React + TypeScript frontend
│   ├── App.tsx           Main UI: server list, per-app switches
│   ├── lib/types.ts      Shared TypeScript types
│   └── styles/global.css
├── src-tauri/            Tauri 2 backend (Rust)
│   ├── src/
│   │   ├── lib.rs        App entry, plugins, command registration
│   │   ├── commands.rs   Tauri commands (list/toggle/sync/import)
│   │   ├── store.rs      SSOT manager (load/upsert/toggle/persist)
│   │   ├── paths.rs      Cross-platform config paths
│   │   ├── atomic.rs     Atomic file writes (tmp + rename)
│   │   ├── adapter/
│   │   │   ├── mod.rs    Adapter trait + registry
│   │   │   ├── claude.rs
│   │   │   ├── claude_desktop.rs
│   │   │   ├── codex.rs
│   │   │   ├── gemini.rs
│   │   │   ├── hermes.rs
│   │   │   └── opencode.rs
│   │   └── types.rs      Shared Rust types (Store, McpServerEntry, ...)
│   ├── capabilities/main.json   Tauri ACL permissions
│   ├── tauri.conf.json
│   └── Cargo.toml
└── .github/workflows/release.yml   Build + publish on tag push
```

## Adding a new app

1. Create `src-tauri/src/adapter/<name>.rs` implementing the `Adapter` trait.
2. Register it in `src-tauri/src/adapter/mod.rs` (`all_adapters`, `adapter_for`).
3. Add its config path helper in `src-tauri/src/paths.rs`.
4. Add the app id to the frontend's `APP_ORDER` and `APP_LABELS` in `src/App.tsx`.
5. Add an icon placeholder and (optional) brand color in CSS.

## License

MIT.
