# [0.10.0](https://github.com/StormShynn/mcp-switch/compare/v0.9.0...v0.10.0) (2026-07-23)


### Features

* use mcp_json for config parsing - Enhances config parsing resilience by switching from serde_json to mcp_json's lenient parser across multiple adapters, improving error handling for malformed configurations while maintaining backward compatibility. ([0a47acc](https://github.com/StormShynn/mcp-switch/commit/0a47acc4a3d9e6fe7567834c665ef0fdf8e7e976))

# [0.9.0](https://github.com/StormShynn/mcp-switch/compare/v0.8.0...v0.9.0) (2026-07-23)


### Features

* **runner,commands:** add restart policy and profile management for auto-run ([bcfb4f7](https://github.com/StormShynn/mcp-switch/commit/bcfb4f7ebb5aaa112d83eaaebbdc47c07ffc03fc))

# [0.8.0](https://github.com/StormShynn/mcp-switch/compare/v0.7.0...v0.8.0) (2026-07-23)


### Features

* add MCP server management commands and tray-icon support ([83b3581](https://github.com/StormShynn/mcp-switch/commit/83b35815aa1bcaea66a12dff2226db6266dc6a0b))

# [0.7.0](https://github.com/StormShynn/mcp-switch/compare/v0.6.0...v0.7.0) (2026-07-14)


### Features

* add Export/Import JSON, server templates, Clone button ([cdb8557](https://github.com/StormShynn/mcp-switch/commit/cdb85577991633edbada6762ed54638d46291184))

# [0.6.0](https://github.com/StormShynn/mcp-switch/compare/v0.5.0...v0.6.0) (2026-07-14)


### Features

* fix updater signing keys, add search bar, keyboard shortcuts, server details ([1a82912](https://github.com/StormShynn/mcp-switch/commit/1a82912259eed7e089e9b65b02ff06bd1c687cb6))

# [0.5.0](https://github.com/StormShynn/mcp-switch/compare/v0.4.3...v0.5.0) (2026-07-14)


### Features

* add MCP server connection testing with Test button ([859775e](https://github.com/StormShynn/mcp-switch/commit/859775efb1583a203b9195c9d1ac304d58261068))

## [0.4.3](https://github.com/StormShynn/mcp-switch/compare/v0.4.2...v0.4.3) (2026-07-08)


### Bug Fixes

* **release:** drop unused bundle formats and scope artifact upload path ([ecbd2d8](https://github.com/StormShynn/mcp-switch/commit/ecbd2d83d979736a98121d0e4e906f38736634e3))

## [0.4.2](https://github.com/StormShynn/mcp-switch/compare/v0.4.1...v0.4.2) (2026-07-08)


### Bug Fixes

* **ci:** dispatch release.yml explicitly after semantic-release tags a version ([a370c9f](https://github.com/StormShynn/mcp-switch/commit/a370c9fbb271732ab97c5076bdda9f0322ed84f3))

## [0.4.2](https://github.com/StormShynn/mcp-switch/compare/v0.4.1...v0.4.2) (2026-07-08)


### Bug Fixes

* **ci:** dispatch release.yml explicitly after semantic-release tags a version ([a370c9f](https://github.com/StormShynn/mcp-switch/commit/a370c9fbb271732ab97c5076bdda9f0322ed84f3))

## [0.4.1](https://github.com/StormShynn/mcp-switch/compare/v0.4.0...v0.4.1) (2026-07-07)


### Bug Fixes

* preserve unknown live-config fields across every adapter ([855725d](https://github.com/StormShynn/mcp-switch/commit/855725d10acdbf277ac799fee71f52d39f052eed))

# [0.4.0](https://github.com/StormShynn/mcp-switch/compare/v0.3.0...v0.4.0) (2026-07-07)


### Features

* per-(name,app) server model, Trash/Restart actions, and Hermes YAML fix ([017f9fc](https://github.com/StormShynn/mcp-switch/commit/017f9fc68f3fa6c488c8e70707823ad5f2c59c58))

# [0.3.0](https://github.com/StormShynn/mcp-switch/compare/v0.2.0...v0.3.0) (2026-07-07)


### Features

* surgical per-server config writes, live-config sync with soft-trash, and manual add/edit ([f8ecae1](https://github.com/StormShynn/mcp-switch/commit/f8ecae159bde4f06dbcd6bc96a877186e26c2c26))

# [0.2.0](https://github.com/StormShynn/mcp-switch/compare/v0.1.0...v0.2.0) (2026-07-07)


### Features

* add Antigravity and Claude Desktop adapters and expand platform support ([09a0bd1](https://github.com/StormShynn/mcp-switch/commit/09a0bd12228edb8442b0bc2fa740e2edd23ba1d1))
* add Antigravity and Claude Desktop adapters, expand platform support → 4 file mới (antigravity.rs, claude_desktop.rs, mcp_json.rs, winshim.rs) + toàn bộ adapter/core/UI/READM ([da25c34](https://github.com/StormShynn/mcp-switch/commit/da25c34818f1901106d01f6289fed0c6c2afba80))
* add Antigravity and Claude Desktop adapters, expand platform support → 4 file mới (antigravity.rs, claude_desktop.rs, mcp_json.rs, winshim.rs) + toàn bộ adapter/core/UI/READM ([63c9eae](https://github.com/StormShynn/mcp-switch/commit/63c9eaeebab42ab4d120c970628f3e7b0ce340d2))
* add semantic-release for auto-tagging ([7a9c5b5](https://github.com/StormShynn/mcp-switch/commit/7a9c5b507fb9caa1cf22825a593ff9a644cd0b77))

# Changelog

## 0.1.0 (initial scaffold)

- Tauri 2 + Rust + React + TypeScript desktop app.
- SSOT store at `~/.mcp-switch/store.json` with per-app enabled flags.
- Adapters for Claude Code, Codex, Gemini CLI, Hermes, OpenCode.
- Atomic file writes (tmp + rename + fsync) to avoid corrupting native configs.
- `tauri-plugin-updater` wired to GitHub Releases (tag-driven workflow).
- UI: per-server row with 5 toggle switches, app filter chips, name/status sorting, Import button.
- CI: multi-platform release workflow (macOS, Linux, Windows) with auto-generated update.json.
