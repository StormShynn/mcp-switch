import type { Transport } from "./types";

export interface ServerTemplate {
  label: string;
  description: string;
  transport: Transport;
  command: string;
  args: string;
  envNotes?: string;
}

export const SERVER_TEMPLATES: ServerTemplate[] = [
  { label: "Filesystem", description: "Read, write, search files", transport: "stdio", command: "npx", args: "-y\n@modelcontextprotocol/server-filesystem\n<path>" },
  { label: "GitHub", description: "Issues, PRs, repos", transport: "stdio", command: "npx", args: "-y\n@modelcontextprotocol/server-github", envNotes: "Set GITHUB_TOKEN" },
  { label: "PostgreSQL", description: "Read, query PostgreSQL", transport: "stdio", command: "npx", args: "-y\n@modelcontextprotocol/server-postgres\npostgresql://localhost/mydb" },
  { label: "SQLite", description: "Read, query SQLite DB", transport: "stdio", command: "npx", args: "-y\n@modelcontextprotocol/server-sqlite\n<path>" },
  { label: "Memory", description: "Persistent memory", transport: "stdio", command: "npx", args: "-y\n@modelcontextprotocol/server-memory" },
  { label: "Puppeteer", description: "Browser automation", transport: "stdio", command: "npx", args: "-y\n@modelcontextprotocol/server-puppeteer" },
  { label: "Fetch", description: "HTTP requests", transport: "stdio", command: "npx", args: "-y\n@modelcontextprotocol/server-fetch" },
  { label: "Sequential Thinking", description: "Step-by-step reasoning", transport: "stdio", command: "npx", args: "-y\n@modelcontextprotocol/server-sequential-thinking" },
  { label: "Brave Search", description: "Web search", transport: "stdio", command: "npx", args: "-y\n@modelcontextprotocol/server-brave-search", envNotes: "Set BRAVE_API_KEY" },
];
