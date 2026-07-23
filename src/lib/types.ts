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

/** Live child process spawned by MCP Switch's runner (backend `runner.rs`). */
export interface RunningServer {
  name: string;
  app: AppId;
  pid: number;
  command: string;
  args: string[];
  /** Unix seconds (UTC) when the child was spawned. */
  startedAt: number;
}

/** Canonical map key for a running server: `${app}::${name}`. */
export const runningKey = (app: AppId, name: string): string => `${app}::${name}`;

/** One entry in the persisted auto-run list (server spawns when MCP Switch starts). */
export interface AutoRunKey {
  name: string;
  app: AppId;
}

/** Payload of the `mcp-server-exited` event emitted when a runner child terminates on its own. */
export interface ServerExitEvent {
  name: string;
  app: AppId;
  pid: number;
  code: number;
}

/** PM2-style restart policy for a runner-launched MCP server. */
export type RestartPolicy =
  | { mode: "never" }
  | { mode: "onFailure"; maxRetries: number; backoffMs: number }
  | { mode: "always"; maxRetries: number; backoffMs: number };

/** A Foreman-style profile: a named group of `(app, name)` members that
 *  can be run/stopped together. */
export interface ProfileDto {
  id: string;
  label: string;
  members: { name: string; app: AppId }[];
}

/** Payload of the `mcp-server-exited` event emitted when a runner child terminates on its own. */
export interface ServerExitEvent {
  name: string;
  app: AppId;
  pid: number;
  code: number;
  /** True if a follow-up child will be spawned automatically (per restart policy). */
  willRestart: boolean;
}

/** Payload of the `mcp-server-updated` event emitted after a successful start
 *  or auto-restart so the UI's running list can refresh without polling. */
export interface ServerStartedEvent {
  name: string;
  app: AppId;
  pid: number;
  restartCount: number;
}
