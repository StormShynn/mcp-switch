export type AppId =
  | "claude"
  | "claude-desktop"
  | "codex"
  | "gemini"
  | "hermes"
  | "opencode"
  | "antigravity";

/** "stdio" launches `command`; "http"/"sse" connect to `url` instead. */
export type Transport = "stdio" | "http" | "sse";

/**
 * A single MCP server entry, scoped to exactly one app. The same name can
 * exist independently for several apps (e.g. "fs" for both "claude" and
 * "codex"), each with its own command/args/env/url/headers — real-world MCP
 * servers often genuinely need different config per tool (different
 * cookie/token file paths, different transports), so entries are never
 * shared or fanned out across apps.
 */
export interface McpServerEntry {
  name: string;
  app: AppId;
  transport: Transport;
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  url?: string;
  headers?: Record<string, string>;
  /** Whether this entry is currently written into `app`'s live config. */
  enabled: boolean;
  /** Soft-trashed: vanished from its app's live config while enabled. */
  deleted: boolean;
  /** Live-config fields outside the shape above (Codex's `cwd`, Gemini's
   * `timeout`, ...) — round-tripped so editing a server never drops them. */
  extra?: Record<string, unknown>;
}

export interface Store {
  servers: McpServerEntry[];
}

/** Payload for the add/edit server form (backend command `save_server`). */
export interface ServerInput {
  name: string;
  app: AppId;
  transport: Transport;
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  url?: string;
  headers?: Record<string, string>;
  enabled: boolean;
}

export interface ConnectionTestResult {
  success: boolean;
  message: string;
  serverInfo: string | null;
}

export interface SyncSummary {
  added: number;
  flaggedDeleted: number;
}

export interface AppInfo {
  id: AppId;
  label: string;
  configFile: string;
}

export const APPS: AppInfo[] = [
  { id: "claude", label: "Claude Code", configFile: "~/.claude.json" },
  { id: "claude-desktop", label: "Claude Desktop", configFile: "claude_desktop_config.json" },
  { id: "codex", label: "Codex CLI", configFile: "~/.codex/config.toml" },
  { id: "gemini", label: "Gemini CLI", configFile: "~/.gemini/settings.json" },
  { id: "hermes", label: "Hermes", configFile: "~/.hermes/config.yaml" },
  { id: "opencode", label: "OpenCode", configFile: "~/.config/opencode/config.json" },
  { id: "antigravity", label: "Antigravity", configFile: "~/.gemini/config/mcp_config.json" },
];

export const APP_COLORS: Record<AppId, string> = {
  claude: "#c977b3",
  "claude-desktop": "#d08770",
  codex: "#58a6ff",
  gemini: "#7c5cfc",
  hermes: "#34d399",
  opencode: "#fbbf24",
  antigravity: "#ef4444",
};
