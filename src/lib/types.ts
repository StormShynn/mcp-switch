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

export interface McpServerEntry {
  name: string;
  transport: Transport;
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  url?: string;
  headers?: Record<string, string>;
  enabled: Record<AppId, boolean>;
  /** App ids whose real config actually defines this server. */
  sources: AppId[];
  /** Soft-trashed: vanished from every app it used to be enabled in. */
  deleted: boolean;
}

export interface Store {
  servers: McpServerEntry[];
}

/** Payload for the add/edit server form (backend command `save_server`). */
export interface ServerInput {
  name: string;
  transport: Transport;
  command?: string;
  args?: string[];
  env?: Record<string, string>;
  url?: string;
  headers?: Record<string, string>;
  enabledApps: AppId[];
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
  { id: "hermes", label: "Hermes", configFile: "~/.hermes/config.{toml,json}" },
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

export function defaultEnabled(): Record<AppId, boolean> {
  return {
    claude: false,
    "claude-desktop": false,
    codex: false,
    gemini: false,
    hermes: false,
    opencode: false,
    antigravity: false,
  };
}
