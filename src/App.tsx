import { useState, useEffect, useCallback } from "react";

/** Detect Tauri invoke errors that occur when the Rust backend isn't running */
function isBackendError(err: Error): boolean {
  const msg = err.message ?? "";
  // Tauri throws these when the Rust backend isn't available
  return msg.includes("Invoke not available") || msg.includes("backend is not running");
}
import { invoke } from "@tauri-apps/api/core";
import { getVersion } from "@tauri-apps/api/app";
import { openUrl } from "@tauri-apps/plugin-opener";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import type { McpServerEntry, AppId } from "./lib/types";
import { APPS, APP_COLORS, defaultEnabled } from "./lib/types";

type UpdateStatus =
  | "idle"
  | "checking"
  | "up-to-date"
  | "available"
  | "downloading"
  | "installing"
  | "error";

const REPO_URL = "https://github.com/StormShynn/mcp-switch";

type SortKey = "name" | "status";
type SortDir = "asc" | "desc";
type FilterKey = "all" | AppId;

/** Whether `appId` is a relevant scope for this server: it's one of the
 * apps whose real config defines it (or, for legacy entries imported
 * before `sources` existed, until they're re-imported), or it's already
 * enabled there. This is independent of on/off state on purpose — toggling
 * a server must never make it appear/disappear from its own app's tab. */
function isRelevantApp(server: McpServerEntry, appId: AppId): boolean {
  return (
    server.sources.length === 0 ||
    server.sources.includes(appId) ||
    server.enabled[appId]
  );
}

function sortServers(
  servers: McpServerEntry[],
  key: SortKey,
  dir: SortDir,
  filter: FilterKey
): McpServerEntry[] {
  let filtered = servers;

  if (filter !== "all") {
    filtered = servers.filter((s) => isRelevantApp(s, filter));
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
  // Same app-relevance rule the filter tabs use, so a server never appears
  // or disappears anywhere just because it was toggled on/off.
  const visibleApps = APPS.filter((a) => isRelevantApp(server, a.id));
  const enabledCount = visibleApps.filter((a) => server.enabled[a.id]).length;

  return (
    <div
      className="server-row fade-in"
      style={{ animationDelay: `${index * 30}ms` }}
    >
      <div className="server-info">
        <div className="server-name">{server.name}</div>
        <div className="server-command">
          {server.transport === "stdio" ? server.command : server.url}
        </div>
        <div className="server-meta">
          <span className="badge">
            {enabledCount}/{visibleApps.length} apps
          </span>
        </div>
      </div>

      <div className="server-toggles">
        {visibleApps.map((app) => (
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

/* ── About modal ─────────────────────────────────── */
function AboutModal({
  version,
  storePath,
  onClose,
  updateStatus,
  updateVersion,
  updateProgress,
  updateError,
  onCheckForUpdates,
  onInstallUpdate,
}: {
  version: string;
  storePath: string;
  onClose: () => void;
  updateStatus: UpdateStatus;
  updateVersion: string;
  updateProgress: number;
  updateError: string;
  onCheckForUpdates: () => void;
  onInstallUpdate: () => void;
}) {
  return (
    <div className="modal-backdrop" onClick={onClose}>
      <div className="modal fade-in" onClick={(e) => e.stopPropagation()}>
        <h2>MCP Switch</h2>
        <div className="modal-row">
          <span>Version</span>
          <span>{version || "…"}</span>
        </div>
        <div className="modal-row">
          <span>Repository</span>
          <a
            className="modal-link"
            onClick={() => openUrl(REPO_URL)}
          >
            StormShynn/mcp-switch
          </a>
        </div>
        <div className="modal-row">
          <span>License</span>
          <span>MIT</span>
        </div>
        <div className="modal-row modal-row-path">
          <span>Store file</span>
          <span className="modal-path">{storePath || "…"}</span>
        </div>
        <div className="modal-row">
          <span>Updates</span>
          {updateStatus === "idle" && (
            <button className="btn modal-link-btn" onClick={onCheckForUpdates}>
              Check for updates
            </button>
          )}
          {updateStatus === "checking" && <span>Checking…</span>}
          {updateStatus === "up-to-date" && <span>Up to date</span>}
          {updateStatus === "available" && (
            <button className="btn btn-primary" onClick={onInstallUpdate}>
              Update to v{updateVersion}
            </button>
          )}
          {updateStatus === "downloading" && (
            <span>Downloading… {updateProgress}%</span>
          )}
          {updateStatus === "installing" && <span>Installing…</span>}
          {updateStatus === "error" && (
            <span className="modal-update-error" title={updateError}>
              Check failed
            </span>
          )}
        </div>
        <button className="btn modal-close" onClick={onClose}>
          Close
        </button>
      </div>
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
  const [showAbout, setShowAbout] = useState(false);
  const [version, setVersion] = useState("");
  const [storePath, setStorePath] = useState("");
  const [pendingRestarts, setPendingRestarts] = useState<Set<AppId>>(new Set());
  const [updateStatus, setUpdateStatus] = useState<UpdateStatus>("idle");
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);
  const [updateProgress, setUpdateProgress] = useState(0);
  const [updateError, setUpdateError] = useState("");

  const dismissPendingRestart = useCallback((appId: AppId) => {
    setPendingRestarts((prev) => {
      if (!prev.has(appId)) return prev;
      const next = new Set(prev);
      next.delete(appId);
      return next;
    });
  }, []);

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
          transport: "stdio",
          command: "npx @anthropic-ai/claude-code-mcp",
          args: [],
          enabled: { ...defaultEnabled(), claude: true, codex: true },
          sources: ["claude", "codex"],
        },
        {
          name: "filesystem",
          transport: "stdio",
          command: "npx @modelcontextprotocol/server-filesystem",
          args: ["/workspace"],
          enabled: { ...defaultEnabled(), gemini: true, hermes: true },
          sources: ["gemini", "hermes"],
        },
        {
          name: "github",
          transport: "stdio",
          command: "npx @modelcontextprotocol/server-github",
          args: [],
          enabled: { ...defaultEnabled(), opencode: true },
          sources: ["opencode"],
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
        const appLabel = APPS.find((a) => a.id === appId)?.label ?? appId;
        notify(`${enabled ? "Enabled" : "Disabled"} for ${appLabel}`, "success");
        setPendingRestarts((prev) => {
          if (prev.has(appId)) return prev;
          return new Set(prev).add(appId);
        });
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

  const handleShowAbout = useCallback(async () => {
    setShowAbout(true);
    if (!version) {
      getVersion().then(setVersion).catch(() => setVersion("unknown"));
    }
    if (!storePath) {
      invoke<string>("get_store_path").then(setStorePath).catch(() => {});
    }
  }, [version, storePath]);

  const handleCheckForUpdates = useCallback(async () => {
    setUpdateStatus("checking");
    setUpdateError("");
    try {
      const update = await check();
      if (update) {
        setPendingUpdate(update);
        setUpdateStatus("available");
      } else {
        setUpdateStatus("up-to-date");
      }
    } catch (err) {
      setUpdateError(err instanceof Error ? err.message : String(err));
      setUpdateStatus("error");
    }
  }, []);

  const handleInstallUpdate = useCallback(async () => {
    if (!pendingUpdate) return;
    setUpdateStatus("downloading");
    setUpdateProgress(0);
    try {
      let downloaded = 0;
      let total = 0;
      await pendingUpdate.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          setUpdateProgress(total > 0 ? Math.min(100, Math.round((downloaded / total) * 100)) : 0);
        } else if (event.event === "Finished") {
          setUpdateStatus("installing");
        }
      });
      await relaunch();
    } catch (err) {
      setUpdateError(err instanceof Error ? err.message : String(err));
      setUpdateStatus("error");
    }
  }, [pendingUpdate]);

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
          <button className="btn" onClick={handleShowAbout} title="About MCP Switch">
            About
          </button>
          <button
            className="btn btn-primary"
            onClick={handleImport}
            disabled={importing}
          >
            {importing ? "Importing…" : "Import"}
          </button>
        </div>
      </header>

      {showAbout && (
        <AboutModal
          version={version}
          storePath={storePath}
          onClose={() => setShowAbout(false)}
          updateStatus={updateStatus}
          updateVersion={pendingUpdate?.version ?? ""}
          updateProgress={updateProgress}
          updateError={updateError}
          onCheckForUpdates={handleCheckForUpdates}
          onInstallUpdate={handleInstallUpdate}
        />
      )}

      {/* Restart reminder */}
      {pendingRestarts.size > 0 && (
        <div className="restart-banner">
          <span className="restart-banner-label">Restart to apply:</span>
          <div className="restart-banner-chips">
            {APPS.filter((a) => pendingRestarts.has(a.id)).map((a) => (
              <button
                key={a.id}
                className="restart-chip"
                onClick={() => dismissPendingRestart(a.id)}
                title={`Dismiss — I already restarted ${a.label}`}
              >
                <span style={{ color: APP_COLORS[a.id] }}>{a.label}</span>
                <span className="restart-chip-x">×</span>
              </button>
            ))}
          </div>
        </div>
      )}

      {/* Notification */}
      {notification && (
        <div className={`notification notification-${notification.type} slide-in`}>
          {notification.message}
        </div>
      )}

      {/* Toolbar */}
      <div className="toolbar">
        <div className="filter-group">
          {(["all", ...APPS.map((a) => a.id)] as FilterKey[]).map((f) => (
            <button
              key={f}
              className={`filter-chip ${filter === f ? "active" : ""}`}
              onClick={() => setFilter(f)}
            >
              {f === "all"
                ? "All"
                : APPS.find((a) => a.id === f)?.label.split(" ")[0] ?? f}
            </button>
          ))}
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
