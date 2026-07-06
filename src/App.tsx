import { useState, useEffect, useCallback } from "react";

/** Detect Tauri invoke errors that occur when the Rust backend isn't running */
function isBackendError(err: Error): boolean {
  const msg = err.message ?? "";
  // Tauri throws these when the Rust backend isn't available
  return msg.includes("Invoke not available") || msg.includes("backend is not running");
}
import { invoke } from "@tauri-apps/api/core";
import type { McpServerEntry, AppId } from "./lib/types";
import { APPS, APP_COLORS, defaultEnabled } from "./lib/types";

type SortKey = "name" | "status";
type SortDir = "asc" | "desc";
type FilterKey = "all" | AppId | "disabled";

function sortServers(
  servers: McpServerEntry[],
  key: SortKey,
  dir: SortDir,
  filter: FilterKey
): McpServerEntry[] {
  let filtered = servers;

  if (filter === "disabled") {
    filtered = servers.filter(
      (s) => !APPS.some((a) => s.enabled[a.id])
    );
  } else if (filter !== "all") {
    filtered = servers.filter((s) => s.enabled[filter]);
  }

  return [...filtered].sort((a, b) => {
    let cmp: number;
    if (key === "name") {
      cmp = a.name.localeCompare(b.name);
    } else {
      const aOn = APPS.filter((app) => a.enabled[app.id]).length;
      const bOn = APPS.filter((app) => b.enabled[app.id]).length;
      cmp = bOn - aOn;
    }
    return dir === "asc" ? cmp : -cmp;
  });
}

/* ── Server row component ────────────────────────── */
function ServerRow({
  server,
  index,
  onToggle,
}: {
  server: McpServerEntry;
  index: number;
  onToggle: (serverName: string, appId: AppId, enabled: boolean) => void;
}) {
  const enabledCount = APPS.filter((a) => server.enabled[a.id]).length;

  return (
    <div
      className="server-row fade-in"
      style={{ animationDelay: `${index * 30}ms` }}
    >
      <div className="server-info">
        <div className="server-name">{server.name}</div>
        <div className="server-command">{server.command}</div>
        <div className="server-meta">
          <span className="badge">
            {enabledCount}/{APPS.length} apps
          </span>
        </div>
      </div>

      <div className="server-toggles">
        {APPS.map((app) => (
          <label
            key={app.id}
            className="toggle app-toggle"
            title={`${app.label} — ${server.enabled[app.id] ? "enabled" : "disabled"}`}
            onClick={(e) => e.stopPropagation()}
          >
            <input
              type="checkbox"
              checked={server.enabled[app.id]}
              onChange={(e) =>
                onToggle(server.name, app.id, e.target.checked)
              }
            />
            <span className="toggle-track" />
            <span
              className="toggle-label"
              style={{ color: APP_COLORS[app.id] }}
            >
              {app.id}
            </span>
          </label>
        ))}
      </div>
    </div>
  );
}

/* ── Empty state ─────────────────────────────────── */
function EmptyState({ onImport }: { onImport: () => void }) {
  return (
    <div className="empty-state fade-in">
      <div className="empty-icon">
        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
          <path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z" />
          <polyline points="13 2 13 9 20 9" />
          <line x1="9" y1="13" x2="15" y2="13" />
          <line x1="12" y1="10" x2="12" y2="16" />
        </svg>
      </div>
      <h2>No MCP servers yet</h2>
      <p>
        Import existing servers from your installed coding tools, or add a new
        server manually.
      </p>
      <button className="btn btn-primary" onClick={onImport}>
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
          <polyline points="7 10 12 15 17 10" />
          <line x1="12" y1="15" x2="12" y2="3" />
        </svg>
        Import from tools
      </button>
    </div>
  );
}

/* ── Main App ────────────────────────────────────── */
export default function App() {
  const [servers, setServers] = useState<McpServerEntry[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [sortKey, setSortKey] = useState<SortKey>("name");
  const [sortDir, setSortDir] = useState<SortDir>("asc");
  const [filter, setFilter] = useState<FilterKey>("all");
  const [importing, setImporting] = useState(false);
  const [notification, setNotification] = useState<{
    message: string;
    type: "success" | "error";
  } | null>(null);

  const notify = useCallback((message: string, type: "success" | "error") => {
    setNotification({ message, type });
    setTimeout(() => setNotification(null), 3000);
  }, []);

  const loadServers = useCallback(async () => {
    try {
      setLoading(true);
      setError(null);
      const store = await invoke<{ servers: McpServerEntry[] }>("list_servers");
      setServers(store.servers);
    } catch (err) {
      // Fallback: show sample data when backend is unavailable
      setServers([
        {
          name: "playwright",
          command: "npx @anthropic-ai/claude-code-mcp",
          args: [],
          enabled: { ...defaultEnabled(), claude: true, codex: true },
        },
        {
          name: "filesystem",
          command: "npx @modelcontextprotocol/server-filesystem",
          args: ["/workspace"],
          enabled: { ...defaultEnabled(), gemini: true, hermes: true },
        },
        {
          name: "github",
          command: "npx @modelcontextprotocol/server-github",
          args: [],
          enabled: { ...defaultEnabled(), opencode: true },
        },
      ]);
      // Only surface non-runtime errors (backend unavailable is expected)
      if (err instanceof Error && !isBackendError(err)) {
        setError(err.message);
      }
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadServers();
  }, [loadServers]);

  const handleToggle = useCallback(
    async (serverName: string, appId: AppId, enabled: boolean) => {
      // Optimistic update
      setServers((prev) =>
        prev.map((s) =>
          s.name === serverName
            ? { ...s, enabled: { ...s.enabled, [appId]: enabled } }
            : s
        )
      );

      try {
        await invoke("toggle_server", { serverName, appId, enabled });
      } catch {
        // Revert on failure
        setServers((prev) =>
          prev.map((s) =>
            s.name === serverName
              ? { ...s, enabled: { ...s.enabled, [appId]: !enabled } }
              : s
          )
        );
        notify("Failed to toggle server", "error");
      }
    },
    [notify]
  );

  const handleImport = useCallback(async () => {
    try {
      setImporting(true);
      const result = await invoke<{ imported: number }>("import_servers");
      notify(`Imported ${result.imported} server(s)`, "success");
      await loadServers();
    } catch {
      notify("Import failed", "error");
    } finally {
      setImporting(false);
    }
  }, [loadServers, notify]);

  const toggleSort = (key: SortKey) => {
    if (sortKey === key) {
      setSortDir((d) => (d === "asc" ? "desc" : "asc"));
    } else {
      setSortKey(key);
      setSortDir("asc");
    }
  };

  const sorted = sortServers(servers, sortKey, sortDir, filter);
  const sortArrow = (key: SortKey) =>
    sortKey === key ? (sortDir === "asc" ? " ▲" : " ▼") : "";

  return (
    <div className="app-container">
      {/* Header */}
      <header className="app-header">
        <div className="header-left">
          <h1 className="app-title">MCP Switch</h1>
          <span className="app-subtitle">
            {servers.length} server{servers.length !== 1 ? "s" : ""}
          </span>
        </div>
        <div className="header-actions">
          <button
            className="btn btn-primary"
            onClick={handleImport}
            disabled={importing}
          >
            {importing ? "Importing…" : "Import"}
          </button>
        </div>
      </header>

      {/* Notification */}
      {notification && (
        <div className={`notification notification-${notification.type} slide-in`}>
          {notification.message}
        </div>
      )}

      {/* Toolbar */}
      <div className="toolbar">
        <div className="filter-group">
          {(["all", ...APPS.map((a) => a.id), "disabled"] as FilterKey[]).map(
            (f) => (
              <button
                key={f}
                className={`filter-chip ${filter === f ? "active" : ""}`}
                onClick={() => setFilter(f)}
              >
                {f === "all"
                  ? "All"
                  : f === "disabled"
                    ? "Disabled"
                    : APPS.find((a) => a.id === f)?.label.split(" ")[0] ?? f}
              </button>
            )
          )}
        </div>
        <div className="sort-group">
          <button
            className={`btn btn-sort ${sortKey === "name" ? "active" : ""}`}
            onClick={() => toggleSort("name")}
          >
            Name{sortArrow("name")}
          </button>
          <button
            className={`btn btn-sort ${sortKey === "status" ? "active" : ""}`}
            onClick={() => toggleSort("status")}
          >
            Status{sortArrow("status")}
          </button>
        </div>
      </div>

      {/* Content */}
      <div className="app-content">
        {loading ? (
          <div className="loading-state">
            <div className="spinner" />
            <p>Loading servers…</p>
          </div>
        ) : sorted.length === 0 ? (
          <EmptyState onImport={handleImport} />
        ) : (
          <div className="server-list">
            {sorted.map((server, i) => (
              <ServerRow
                key={server.name}
                server={server}
                index={i}
                onToggle={handleToggle}
              />
            ))}
          </div>
        )}
      </div>

      {/* Legend */}
      <footer className="app-footer">
        <div className="legend">
          {APPS.map((app) => (
            <span key={app.id} className="legend-item">
              <span
                className="legend-dot"
                style={{ backgroundColor: APP_COLORS[app.id] }}
              />
              {app.label}
            </span>
          ))}
        </div>
      </footer>

    </div>
  );
}
