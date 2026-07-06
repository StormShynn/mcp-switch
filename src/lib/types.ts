export type AppId = "claude" | "codex" | "gemini" | "hermes" | "opencode";

export interface McpServerEntry {
  name: string;
  command: string;
  args?: string[];
  env?: Record<string, string>;
  enabled: Record<AppId, boolean>;
}

export interface Store {
  servers: McpServerEntry[];
}

export interface AppInfo {
  id: AppId;
  label: string;
  configFile: string;
}

export const APPS: AppInfo[] = [
  { id: "claude", label: "Claude Code", configFile: "~/.claude.json" },
  { id: "codex", label: "Codex CLI", configFile: "~/.codex/config.toml" },
  { id: "gemini", label: "Gemini CLI", configFile: "~/.gemini/settings.json" },
  { id: "hermes", label: "Hermes", configFile: "~/.hermes/config.{toml,json}" },
  { id: "opencode", label: "OpenCode", configFile: "~/.config/opencode/config.json" },
];

export const APP_COLORS: Record<AppId, string> = {
  claude: "#c977b3",
  codex: "#58a6ff",
  gemini: "#7c5cfc",
  hermes: "#34d399",
  opencode: "#fbbf24",
};

export function defaultEnabled(): Record<AppId, boolean> {
  return {
    claude: false,
    codex: false,
    gemini: false,
    hermes: false,
    opencode: false,
  };
}
